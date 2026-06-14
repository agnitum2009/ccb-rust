use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// State snapshot for the Gemini session log reader.
///
/// Mirrors the state payload produced by Python
/// `provider_backends.gemini.comm_runtime.session_content`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GeminiReaderState {
    pub session_path: Option<PathBuf>,
    pub msg_count: i64,
    pub mtime: u64,
    pub mtime_ns: u64,
    pub size: u64,
    pub last_gemini_id: Option<String>,
    pub last_gemini_hash: Option<String>,
    pub last_tool_call_count: i64,
    pub last_thought_count: i64,
}

/// Compute the Gemini project hash candidates for a workspace.
///
/// Mirrors Python `provider_backends.gemini.comm_runtime.project_hash_runtime.candidates`.
pub fn project_hash_candidates(work_dir: &Path, root: &Path) -> Vec<String> {
    let abs_path = resolve_work_dir(work_dir);
    let (raw_base, slug_base, sha256_hash) = candidate_bases(&abs_path);
    let discovered = discover_project_hashes(root, &raw_base, &slug_base);
    ordered_candidates(discovered, &slug_base, &raw_base, &sha256_hash)
}

/// Find the latest Gemini session file for a workspace.
///
/// Mirrors Python `provider_backends.gemini.comm_runtime.session_selection.latest_session`.
pub fn find_latest_session_path(
    root: &Path,
    work_dir: &Path,
    preferred_session: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(preferred) = preferred_session {
        if preferred.is_file() {
            return Some(preferred.to_path_buf());
        }
    }
    let project_hash = project_hash_for_work_dir(work_dir, root);
    let chats = root.join(&project_hash).join("chats");
    if !chats.is_dir() {
        return fallback_any_project_session(root);
    }
    let mut best: Option<(PathBuf, u64)> = None;
    for entry in std::fs::read_dir(&chats).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("session-") || !name.ends_with(".json") {
            continue;
        }
        let mtime = path_mtime(&path);
        if best.as_ref().map(|(_, t)| mtime > *t).unwrap_or(true) {
            best = Some((path, mtime));
        }
    }
    best.map(|(p, _)| p)
}

/// Read a Gemini session JSON file.
pub fn read_session_json(path: &Path) -> Option<Value> {
    if !path.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    if value.is_object() {
        Some(value)
    } else {
        None
    }
}

/// Extract the latest Gemini message from a session payload.
///
/// Returns `(message_id, content)` if a non-empty Gemini message exists.
pub fn extract_last_gemini(payload: &Value) -> Option<(String, String)> {
    let messages = payload.get("messages").and_then(Value::as_array)?;
    for message in messages.iter().rev() {
        let obj = message.as_object()?;
        if obj.get("type").and_then(Value::as_str) != Some("gemini") {
            continue;
        }
        let content = message_content(message);
        if content.is_empty() {
            continue;
        }
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return Some((id, content));
    }
    None
}

/// Capture the initial reader state for a workspace.
pub fn capture_reader_state(
    root: &Path,
    work_dir: &Path,
    preferred_session: Option<&Path>,
) -> GeminiReaderState {
    let session = find_latest_session_path(root, work_dir, preferred_session);
    let payload = session.as_ref().and_then(|p| read_session_json(p));
    state_from_payload(session.as_deref(), payload.as_ref())
}

/// Try to read a new Gemini message since the provided state.
///
/// Returns `(Some(reply), new_state)` when a new Gemini message is observed,
/// otherwise `(None, updated_state)`.
pub fn try_get_message(state: &GeminiReaderState) -> (Option<String>, GeminiReaderState) {
    let session = state.session_path.as_deref().and_then(|p| {
        if p.is_file() {
            Some(p.to_path_buf())
        } else {
            None
        }
    });
    let new_state = if let Some(session) = session {
        let payload = read_session_json(&session);
        state_from_payload(Some(&session), payload.as_ref())
    } else {
        GeminiReaderState::default()
    };

    let reply = if let Some(payload) = new_state
        .session_path
        .as_deref()
        .and_then(read_session_json)
    {
        if let Some((id, content)) = extract_last_gemini(&payload) {
            let hash = sha256_hex(&content);
            let changed = new_state.msg_count > state.msg_count
                || new_state.last_gemini_id.as_deref() != state.last_gemini_id.as_deref()
                || new_state.last_gemini_hash.as_deref() != state.last_gemini_hash.as_deref();
            if changed && !content.is_empty() {
                Some((id, content, hash))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((id, content, hash)) = reply {
        let mut updated = new_state;
        updated.last_gemini_id = Some(id);
        updated.last_gemini_hash = Some(hash);
        (Some(content), updated)
    } else {
        (None, new_state)
    }
}

fn project_hash_for_work_dir(work_dir: &Path, root: &Path) -> String {
    if let Ok(forced) = std::env::var("GEMINI_PROJECT_HASH") {
        let forced = forced.trim();
        if !forced.is_empty() {
            return forced.to_string();
        }
    }
    let candidates = project_hash_candidates(work_dir, root);
    for hash in &candidates {
        if root.join(hash).join("chats").is_dir() {
            return hash.clone();
        }
    }
    candidates.into_iter().next().unwrap_or_default()
}

fn fallback_any_project_session(root: &Path) -> Option<PathBuf> {
    if !matches!(
        std::env::var("GEMINI_ALLOW_ANY_PROJECT_SCAN")
            .unwrap_or_default()
            .as_str(),
        "1" | "true" | "yes"
    ) {
        return None;
    }
    let mut best: Option<(PathBuf, u64)> = None;
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let chats = entry.path().join("chats");
        if !chats.is_dir() {
            continue;
        }
        for log_entry in std::fs::read_dir(&chats).into_iter().flatten().flatten() {
            let path = log_entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.starts_with("session-") || !name.ends_with(".json") {
                continue;
            }
            let mtime = path_mtime(&path);
            if best.as_ref().map(|(_, t)| mtime > *t).unwrap_or(true) {
                best = Some((path, mtime));
            }
        }
    }
    best.map(|(p, _)| p)
}

fn resolve_work_dir(work_dir: &Path) -> PathBuf {
    if let Some(rest) = work_dir.to_string_lossy().strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest.trim_start_matches('/'));
        }
    }
    work_dir.to_path_buf()
}

fn candidate_bases(path: &Path) -> (String, String, String) {
    let raw_base = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let (slug_base, sha256_hash) = compute_project_hashes(path);
    (raw_base, slug_base, sha256_hash)
}

fn compute_project_hashes(path: &Path) -> (String, String) {
    let abs_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let basename_hash = slugify_project_hash(&abs_path);
    let sha256_hash = sha256_hex(&abs_path);
    (basename_hash, sha256_hash)
}

fn slugify_project_hash(name: &str) -> String {
    let text = name
        .rsplit_once('/')
        .map(|(_, base)| base)
        .unwrap_or(name)
        .trim()
        .to_lowercase();
    let re = regex::Regex::new(r"[^a-z0-9]+").unwrap();
    re.replace_all(&text, "-").trim_matches('-').to_string()
}

fn discover_project_hashes(root: &Path, raw_base: &str, slug_base: &str) -> Vec<(u64, String)> {
    if !root.is_dir() || slug_base.is_empty() {
        return Vec::new();
    }
    let suffix_re = if slug_base.is_empty() {
        None
    } else {
        Some(regex::Regex::new(&format!(r"^{}-\d+$", regex::escape(slug_base))).unwrap())
    };
    let mut discovered = Vec::new();
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("chats").is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !matches_project_name(name, raw_base, slug_base, suffix_re.as_ref()) {
            continue;
        }
        let mtime = latest_session_mtime(&path.join("chats"));
        discovered.push((mtime, name.to_string()));
    }
    discovered
}

fn matches_project_name(
    name: &str,
    raw_base: &str,
    slug_base: &str,
    suffix_re: Option<&regex::Regex>,
) -> bool {
    name == slug_base || name == raw_base || suffix_re.map(|re| re.is_match(name)).unwrap_or(false)
}

fn latest_session_mtime(chats: &Path) -> u64 {
    let mut max_mtime = chats
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    for entry in std::fs::read_dir(chats).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("session-") || !name.ends_with(".json") {
            continue;
        }
        if let Ok(meta) = path.metadata() {
            if let Ok(t) = meta.modified() {
                if let Ok(d) = t.duration_since(UNIX_EPOCH) {
                    max_mtime = max_mtime.max(d.as_secs());
                }
            }
        }
    }
    max_mtime
}

fn ordered_candidates(
    discovered: Vec<(u64, String)>,
    slug_base: &str,
    raw_base: &str,
    sha256_hash: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let mut discovered = discovered;
    discovered.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (_, name) in discovered {
        add_candidate(&mut candidates, &mut seen, &name);
    }
    add_candidate(&mut candidates, &mut seen, slug_base);
    add_candidate(&mut candidates, &mut seen, raw_base);
    add_candidate(&mut candidates, &mut seen, sha256_hash);
    candidates
}

fn add_candidate(candidates: &mut Vec<String>, seen: &mut HashSet<String>, value: &str) {
    let token = value.trim();
    if token.is_empty() || seen.contains(token) {
        return;
    }
    seen.insert(token.to_string());
    candidates.push(token.to_string());
}

fn state_from_payload(session: Option<&Path>, payload: Option<&Value>) -> GeminiReaderState {
    let (mtime, mtime_ns, size) = session_stats(session);
    let msg_count = if session.is_some() && payload.is_none() {
        -1
    } else {
        payload_messages(payload).len() as i64
    };
    let (last_id, hash, tool_count, thought_count) = last_gemini_metadata(payload);
    GeminiReaderState {
        session_path: session.map(Path::to_path_buf),
        msg_count,
        mtime,
        mtime_ns,
        size,
        last_gemini_id: last_id,
        last_gemini_hash: hash,
        last_tool_call_count: tool_count,
        last_thought_count: thought_count,
    }
}

fn payload_messages(payload: Option<&Value>) -> Vec<&Value> {
    payload
        .and_then(|p| p.get("messages"))
        .and_then(Value::as_array)
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn last_gemini_metadata(payload: Option<&Value>) -> (Option<String>, Option<String>, i64, i64) {
    if let Some((id, content)) = payload.and_then(extract_last_gemini) {
        let tool_count = payload
            .and_then(|p| p.get("toolCalls"))
            .and_then(Value::as_array)
            .map(|a| a.len() as i64)
            .unwrap_or(0);
        let thought_count = payload
            .and_then(|p| p.get("thoughts"))
            .and_then(Value::as_array)
            .map(|a| a.len() as i64)
            .unwrap_or(0);
        (
            Some(id),
            Some(sha256_hex(&content)),
            tool_count,
            thought_count,
        )
    } else {
        (None, None, 0, 0)
    }
}

fn message_content(message: &Value) -> String {
    let content = message.get("content");
    if let Some(s) = content.and_then(Value::as_str) {
        return s.trim().to_string();
    }
    content
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("text").or_else(|| obj.get("content")))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn session_stats(session: Option<&Path>) -> (u64, u64, u64) {
    let Some(session) = session else {
        return (0, 0, 0);
    };
    let Ok(meta) = session.metadata() else {
        return (0, 0, 0);
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    (mtime, mtime_ns, meta.len())
}

fn path_mtime(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sha256_hex(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_last_gemini_from_payload() {
        let payload = serde_json::json!({
            "messages": [
                {"type": "user", "content": "hello"},
                {"type": "gemini", "id": "g-1", "content": "reply text"},
            ]
        });
        let (id, content) = extract_last_gemini(&payload).unwrap();
        assert_eq!(id, "g-1");
        assert_eq!(content, "reply text");
    }

    #[test]
    fn test_capture_and_read_session() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".gemini").join("tmp");
        let chats = root.join("myproject").join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let session = chats.join("session-1.json");
        std::fs::write(
            &session,
            serde_json::json!({
                "messages": [
                    {"type": "gemini", "id": "g-1", "content": "hello gemini"}
                ]
            })
            .to_string(),
        )
        .unwrap();

        std::env::set_var("GEMINI_PROJECT_HASH", "myproject");
        let state = capture_reader_state(&root, tmp.path(), None);
        std::env::remove_var("GEMINI_PROJECT_HASH");

        assert_eq!(state.session_path, Some(session.clone()));
        assert_eq!(state.msg_count, 1);

        // Append a new Gemini message so the reader observes a change.
        std::fs::write(
            &session,
            serde_json::json!({
                "messages": [
                    {"type": "gemini", "id": "g-1", "content": "hello gemini"},
                    {"type": "gemini", "id": "g-2", "content": "second reply"}
                ]
            })
            .to_string(),
        )
        .unwrap();

        let (reply, new_state) = try_get_message(&state);
        assert_eq!(reply, Some("second reply".to_string()));
        assert_eq!(new_state.msg_count, 2);
    }

    #[test]
    fn test_try_get_message_detects_no_change() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".gemini").join("tmp");
        let chats = root.join("myproject").join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let session = chats.join("session-1.json");
        std::fs::write(
            &session,
            serde_json::json!({
                "messages": [{"type": "gemini", "content": "same"}]
            })
            .to_string(),
        )
        .unwrap();

        std::env::set_var("GEMINI_PROJECT_HASH", "myproject");
        let state = capture_reader_state(&root, tmp.path(), None);
        std::env::remove_var("GEMINI_PROJECT_HASH");

        let (_, state2) = try_get_message(&state);
        let (reply, _) = try_get_message(&state2);
        assert!(reply.is_none());
    }
}
