use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ccbr_provider_core::source_home::current_provider_source_home;
use serde_json::Value;

/// Terminal success statuses from DeepCode native sessions.
pub const TERMINAL_SUCCESS_STATUSES: &[&str] = &["completed"];
/// Terminal failure statuses.
pub const TERMINAL_FAILURE_STATUSES: &[&str] = &["failed", "error"];
/// Interrupted statuses.
pub const INTERRUPTED_STATUSES: &[&str] = &["interrupted", "cancelled", "canceled"];
/// Statuses indicating the session is waiting for user input/permission.
pub const WAITING_USER_STATUSES: &[&str] = &["ask_permission", "waiting_for_user"];
/// Permission denied statuses.
pub const PERMISSION_DENIED_STATUSES: &[&str] = &["permission_denied"];

/// Observation of a DeepCode native session for a single request.
#[derive(Debug, Clone)]
pub struct DeepSeekSessionObservation {
    pub request_seen: bool,
    pub completed: bool,
    pub status: String,
    pub reply: String,
    pub session_id: Option<String>,
    pub session_path: Option<PathBuf>,
    pub provider_turn_ref: Option<String>,
    pub line_count: usize,
    pub fail_reason: Option<String>,
    pub updated_at: Option<String>,
}

/// Observe the DeepCode native session store for a request.
pub fn observe_deepseek_session(
    work_dir: &Path,
    req_id: &str,
    home_candidates: Option<&[PathBuf]>,
) -> Option<DeepSeekSessionObservation> {
    if req_id.is_empty() {
        return None;
    }
    let mut observations: Vec<DeepSeekSessionObservation> = Vec::new();
    for project_root in project_roots(work_dir, home_candidates) {
        if let Some(observed) = observe_project_root(&project_root, req_id) {
            observations.push(observed);
        }
    }
    if observations.is_empty() {
        return None;
    }
    let completed: Vec<_> = observations
        .iter()
        .filter(|o| o.completed)
        .cloned()
        .collect();
    if !completed.is_empty() {
        return Some(
            completed
                .into_iter()
                .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
                .unwrap(),
        );
    }
    Some(
        observations
            .into_iter()
            .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
            .unwrap(),
    )
}

/// Compute the DeepCode project code for a workspace path.
pub fn deepseek_project_code(work_dir: &Path) -> String {
    let normalized = match std::fs::canonicalize(work_dir) {
        Ok(p) => p,
        Err(_) => work_dir.to_path_buf(),
    };
    let normalized = normalized.to_string_lossy().to_string();
    let legacy = normalized.replace(['\\', '/'], "-").replace(':', "");
    if legacy.len() <= 64 {
        return legacy;
    }

    let digest = sha256_short(&normalized);
    let basename = sanitize_project_name(
        Path::new(&normalized)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("project"),
    );
    let max_prefix = (1usize).max(64 - digest.len() - 1);
    let prefix = basename.chars().take(max_prefix).collect::<String>();
    let prefix = prefix.trim_end_matches(['-', '.']).to_string();
    let prefix = if prefix.is_empty() {
        "project".to_string()
    } else {
        prefix
    };
    format!("{}-{}", prefix, digest)
}

/// Compute the DeepCode project root directory for a workspace.
pub fn deepseek_project_root(work_dir: &Path, home: Option<&Path>) -> PathBuf {
    deepcode_home(home)
        .join("projects")
        .join(deepseek_project_code(work_dir))
}

fn project_roots(work_dir: &Path, home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for home in candidate_homes(home_candidates) {
        let root = deepseek_project_root(work_dir, Some(&home));
        let resolved = if let Ok(r) = std::fs::canonicalize(&root) {
            r
        } else {
            root.clone()
        };
        if seen.iter().any(|s| s == &resolved) {
            continue;
        }
        seen.push(resolved);
        if root.is_dir() {
            roots.push(root);
        }
    }
    roots
}

fn candidate_homes(home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("DEEPCODE_HOME").or_else(|_| std::env::var("DEEPSEEK_HOME"))
    {
        let explicit = explicit.trim();
        if !explicit.is_empty() {
            candidates.push(shellexpand::tilde(explicit));
        }
    }
    if let Some(list) = home_candidates {
        for item in list {
            candidates.push(shellexpand::tilde(&item.to_string_lossy()));
        }
    }
    candidates.push(current_provider_source_home());
    candidates.push(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")));

    let mut unique: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for candidate in candidates {
        let key = candidate.to_string_lossy().to_string();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        unique.push(candidate);
    }
    unique
}

fn deepcode_home(home: Option<&Path>) -> PathBuf {
    if let Some(home) = home {
        if home.file_name().and_then(|s| s.to_str()) == Some(".deepcode") {
            home.to_path_buf()
        } else {
            home.join(".deepcode")
        }
    } else {
        current_provider_source_home().join(".deepcode")
    }
}

fn observe_project_root(project_root: &Path, req_id: &str) -> Option<DeepSeekSessionObservation> {
    let index = read_json(&project_root.join("sessions-index.json"));
    let entries = index_entries(&index);
    let mut observations: Vec<DeepSeekSessionObservation> = Vec::new();
    for entry in entries {
        let session_id = coerce_str(
            entry
                .get("id")
                .or_else(|| entry.get("sessionId"))
                .or_else(|| entry.get("session_id")),
        );
        if session_id.is_none() {
            continue;
        }
        let session_id = session_id.unwrap();
        let session_path = project_root.join(format!("{}.jsonl", session_id));
        if let Some(observed) = observe_session_file(&session_path, req_id, Some(&entry)) {
            observations.push(observed);
        }
    }
    if !observations.is_empty() {
        return Some(
            observations
                .into_iter()
                .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
                .unwrap(),
        );
    }
    for entry in std::fs::read_dir(project_root).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            if let Some(observed) = observe_session_file(&path, req_id, None) {
                observations.push(observed);
            }
        }
    }
    if observations.is_empty() {
        return None;
    }
    Some(
        observations
            .into_iter()
            .max_by(|a, b| observation_sort_key(a).cmp(&observation_sort_key(b)))
            .unwrap(),
    )
}

fn index_entries(index: &Option<Value>) -> Vec<HashMap<String, Value>> {
    if let Some(arr) = index.as_ref().and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|v| {
                v.as_object()
                    .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            })
            .collect();
    }
    let obj = match index.as_ref().and_then(|v| v.as_object()) {
        Some(o) => o,
        None => return Vec::new(),
    };
    for key in ["sessions", "items", "data"] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| {
                    v.as_object()
                        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                })
                .collect();
        }
    }
    obj.values()
        .filter_map(|v| {
            v.as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        })
        .collect()
}

fn observe_session_file(
    path: &Path,
    req_id: &str,
    index_entry: Option<&HashMap<String, Value>>,
) -> Option<DeepSeekSessionObservation> {
    let raw = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = raw.lines().collect();

    let mut active = false;
    let mut reply_parts: Vec<String> = Vec::new();
    let mut last_assistant_id: Option<String> = None;
    let mut request_line: usize = 0;
    let mut last_line: usize = 0;

    for (index, line) in lines.iter().enumerate() {
        let message: Value = serde_json::from_str(line).ok()?;
        let message = match message.as_object() {
            Some(o) => o,
            None => continue,
        };
        let role = message
            .get("role")
            .or_else(|| message.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let content = message_text(message);
        if role == "user" && content.contains(req_id) {
            active = true;
            reply_parts.clear();
            last_assistant_id = None;
            request_line = index + 1;
            last_line = index + 1;
            continue;
        }
        if !active {
            continue;
        }
        if role == "user" && !content.contains(req_id) {
            active = false;
            continue;
        }
        if role == "assistant" {
            let cleaned = clean_native_reply(&content, req_id);
            if !cleaned.is_empty() {
                reply_parts.push(cleaned);
                last_assistant_id = coerce_str(
                    message
                        .get("id")
                        .or_else(|| message.get("messageId"))
                        .or_else(|| message.get("message_id")),
                );
                last_line = index + 1;
            }
        }
    }

    if request_line == 0 {
        return None;
    }

    let entry = index_entry.cloned().unwrap_or_default();
    let status = coerce_str(entry.get("status")).unwrap_or_default();
    let entry_reply = clean_native_reply(
        &coerce_str(
            entry
                .get("assistantReply")
                .or_else(|| entry.get("assistant_reply")),
        )
        .unwrap_or_default(),
        req_id,
    );
    let reply = if entry_reply.is_empty() {
        clean_native_reply(&reply_parts.join("\n\n"), req_id)
    } else {
        entry_reply
    };
    let fail_reason = coerce_str(
        entry
            .get("failReason")
            .or_else(|| entry.get("fail_reason"))
            .or_else(|| entry.get("error")),
    );
    let updated_at = entry
        .get("updateTime")
        .or_else(|| entry.get("updatedAt"))
        .or_else(|| entry.get("updated_at"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let completed = TERMINAL_SUCCESS_STATUSES.contains(&status.as_str());
    let session_id = coerce_str(entry.get("id").or_else(|| entry.get("sessionId"))).or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    });

    Some(DeepSeekSessionObservation {
        request_seen: true,
        completed,
        status,
        reply,
        session_id: session_id.clone(),
        session_path: Some(path.to_path_buf()),
        provider_turn_ref: last_assistant_id.clone().or(session_id),
        line_count: last_line.max(request_line),
        fail_reason,
        updated_at,
    })
}

fn message_text(message: &serde_json::Map<String, Value>) -> String {
    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
        return content.to_string();
    }
    if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
        let mut parts: Vec<String> = Vec::new();
        for item in content {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(obj) = item.as_object() {
                if let Some(value) = obj
                    .get("text")
                    .or_else(|| obj.get("content"))
                    .and_then(|v| v.as_str())
                {
                    parts.push(value.to_string());
                }
            }
        }
        return parts.join("\n");
    }
    message
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn read_json(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn sanitize_project_name(value: &str) -> String {
    let mut text: String = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    while text.contains("--") {
        text = text.replace("--", "-");
    }
    text.trim_matches(['-', '.']).to_string()
}

fn coerce_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn observation_sort_key(observation: &DeepSeekSessionObservation) -> (u64, usize) {
    let mtime = observation
        .session_path
        .as_ref()
        .map(|p| path_mtime(p))
        .unwrap_or(0);
    (mtime, observation.line_count)
}

fn path_mtime(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sha256_short(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Clean a native CLI reply by stripping banners and done markers.
///
/// Mirrors Python `provider_backends.native_cli_support.clean_native_reply`.
fn clean_native_reply(text: &str, req_id: &str) -> String {
    crate::native_cli_support::clean_native_reply(text, req_id)
}

mod shellexpand {
    use std::path::PathBuf;

    pub fn tilde(input: &str) -> PathBuf {
        if let Some(rest) = input.strip_prefix('~') {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home + rest);
            }
        }
        PathBuf::from(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_deepseek_project_code_short_path() {
        let code = deepseek_project_code(Path::new("/home/user/proj"));
        assert!(code.contains("home-user-proj") || code.len() <= 64);
    }

    #[test]
    fn test_observe_session_file_detects_request_and_reply() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("sess.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, r#"{{"role":"user","content":"CCB_REQ_ID: req-123"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"role":"assistant","content":"hello","id":"msg-1"}}"#
        )
        .unwrap();

        let obs = observe_session_file(&path, "req-123", None).unwrap();
        assert!(obs.request_seen);
        assert_eq!(obs.reply, "hello");
        assert_eq!(obs.provider_turn_ref, Some("msg-1".to_string()));
    }

    #[test]
    fn test_observe_session_file_no_match_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("sess.jsonl");
        std::fs::write(&path, r#"{"role":"user","content":"hello"}"#).unwrap();
        assert!(observe_session_file(&path, "req-123", None).is_none());
    }
}
