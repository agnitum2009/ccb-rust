use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use ccb_provider_core::source_home::current_provider_source_home;
use serde_json::Value;

/// Observation of a Kimi native turn log for a single request.
#[derive(Debug, Clone)]
pub struct KimiTurnObservation {
    pub request_seen: bool,
    pub completed: bool,
    pub reply: String,
    pub session_id: Option<String>,
    pub session_path: Option<PathBuf>,
    pub provider_turn_ref: Option<String>,
    pub line_count: usize,
    pub native_started_at: Option<String>,
    pub native_completed_at: Option<String>,
}

/// Observe the Kimi native session store for a request.
pub fn observe_kimi_turn(
    work_dir: &Path,
    req_id: &str,
    home_candidates: Option<&[PathBuf]>,
) -> Option<KimiTurnObservation> {
    if req_id.is_empty() {
        return None;
    }
    let mut observations: Vec<KimiTurnObservation> = Vec::new();
    for wire_path in wire_paths(work_dir, home_candidates) {
        if let Some(observed) = observe_wire_file(&wire_path, req_id) {
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

/// Compute the Kimi project hash for a workspace path.
pub fn kimi_project_hash(work_dir: &Path) -> String {
    let normalized = work_dir.expand_home().canonical_or_self();
    let normalized = normalized.to_string_lossy().to_string();
    format!("{:x}", md5::compute(normalized.as_bytes()))
}

/// Compute the Kimi sessions root directory for a workspace.
pub fn kimi_sessions_root(work_dir: &Path, home: Option<&Path>) -> PathBuf {
    let base = kimi_home(home);
    base.join("sessions").join(kimi_project_hash(work_dir))
}

fn wire_paths(work_dir: &Path, home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for home in candidate_homes(home_candidates) {
        let root = kimi_sessions_root(work_dir, Some(&home));
        if !root.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = entry.path().join("wire.jsonl");
            if !path.is_file() {
                continue;
            }
            let resolved = path.canonical_or_self();
            if seen.iter().any(|s| s == &resolved) {
                continue;
            }
            seen.push(resolved);
            paths.push(path);
        }
    }
    paths.sort_by_key(|a| path_mtime(a));
    paths
}

fn candidate_homes(home_candidates: Option<&[PathBuf]>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(explicit) = std::env::var("KIMI_HOME") {
        let explicit = explicit.trim();
        if !explicit.is_empty() {
            candidates.push(PathBuf::from(expand_tilde(explicit)));
        }
    }
    if let Some(list) = home_candidates {
        for item in list {
            candidates.push(item.expand_home());
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

fn kimi_home(home: Option<&Path>) -> PathBuf {
    match home {
        None => current_provider_source_home().join(".kimi"),
        Some(home) => {
            if home.file_name().and_then(|s| s.to_str()) == Some(".kimi") {
                home.to_path_buf()
            } else {
                home.join(".kimi")
            }
        }
    }
}

fn observe_wire_file(path: &Path, req_id: &str) -> Option<KimiTurnObservation> {
    let raw = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&raw);
    let lines: Vec<&str> = text.lines().collect();

    let mut current: Option<HashMap<String, Value>> = None;
    let mut latest: Option<KimiTurnObservation> = None;

    for (index, line) in lines.iter().enumerate() {
        let index = index + 1;
        let event: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = match event.as_object() {
            Some(o) => o,
            None => continue,
        };
        let (event_type, payload, timestamp) = normalize_event(event);

        match event_type.as_str() {
            "TurnBegin" => {
                if payload_has_req_id(&payload, req_id) {
                    current = Some(new_state(timestamp, index));
                    latest = Some(observation_from_state(
                        path,
                        current.as_ref().unwrap(),
                        req_id,
                        false,
                        None,
                        index,
                    ));
                } else {
                    current = None;
                }
                continue;
            }
            "turn.prompt" | "turn.started" => {
                if payload_has_req_id(&payload, req_id) {
                    let message_id =
                        coerce_str(payload.get("turnId").or_else(|| payload.get("turn_id")));
                    let mut state = new_state(timestamp, index);
                    if let Some(id) = message_id {
                        state.insert("message_id".to_string(), Value::String(id));
                    }
                    current = Some(state);
                    latest = Some(observation_from_state(
                        path,
                        current.as_ref().unwrap(),
                        req_id,
                        false,
                        None,
                        index,
                    ));
                }
                continue;
            }
            "context.append_message" => {
                let message = payload.get("message").and_then(|v| v.as_object());
                if let Some(message) = message {
                    let role = message
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    let content = text_from_value(message.get("content"));
                    if role == "user" && content.contains(req_id) {
                        current = Some(new_state(timestamp, index));
                        latest = Some(observation_from_state(
                            path,
                            current.as_ref().unwrap(),
                            req_id,
                            false,
                            None,
                            index,
                        ));
                        continue;
                    }
                    if role == "user" && current.is_some() {
                        current = None;
                        continue;
                    }
                    if current.is_none() || role != "assistant" {
                        continue;
                    }
                    let cleaned = clean_native_reply(&content, req_id);
                    if !cleaned.is_empty() {
                        append_part(current.as_mut().unwrap(), cleaned, false);
                        latest = Some(observation_from_state(
                            path,
                            current.as_ref().unwrap(),
                            req_id,
                            false,
                            None,
                            index,
                        ));
                    }
                }
                continue;
            }
            _ => {}
        }

        let current_ref = match current.as_mut() {
            Some(c) => c,
            None => continue,
        };

        match event_type.as_str() {
            "ContentPart" => {
                if let Some(text) = payload.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        append_part(current_ref, text.to_string(), false);
                        latest = Some(observation_from_state(
                            path,
                            current_ref,
                            req_id,
                            false,
                            None,
                            index,
                        ));
                    }
                }
            }
            "assistant.delta" => {
                if let Some(text) = payload.get("delta").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        append_part(current_ref, text.to_string(), true);
                        latest = Some(observation_from_state(
                            path,
                            current_ref,
                            req_id,
                            false,
                            None,
                            index,
                        ));
                    }
                }
            }
            "context.append_loop_event" => {
                if let Some(nested) = payload.get("event").and_then(|v| v.as_object()) {
                    let nested_type = nested.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if nested_type == "content.part" {
                        let text = text_from_value(nested.get("part"));
                        if !text.is_empty() {
                            append_part(current_ref, text, false);
                            latest = Some(observation_from_state(
                                path,
                                current_ref,
                                req_id,
                                false,
                                None,
                                index,
                            ));
                        }
                    }
                }
            }
            "StatusUpdate" => {
                if let Some(message_id) = payload.get("message_id").and_then(|v| v.as_str()) {
                    if !message_id.is_empty() {
                        current_ref.insert(
                            "message_id".to_string(),
                            Value::String(message_id.to_string()),
                        );
                    }
                }
            }
            "TurnEnd" => {
                latest = Some(observation_from_state(
                    path,
                    current_ref,
                    req_id,
                    true,
                    timestamp,
                    index,
                ));
                current = None;
            }
            "turn.ended" => {
                let reason = payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if reason.is_empty() || reason == "completed" {
                    latest = Some(observation_from_state(
                        path,
                        current_ref,
                        req_id,
                        true,
                        timestamp,
                        index,
                    ));
                    current = None;
                }
            }
            _ => {}
        }
    }

    latest
}

fn normalize_event(
    event: &serde_json::Map<String, Value>,
) -> (String, HashMap<String, Value>, Option<String>) {
    if let Some(message) = event.get("message").and_then(|v| v.as_object()) {
        let event_type = message
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let payload: HashMap<String, Value> = message
            .get("payload")
            .and_then(|v| v.as_object())
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let timestamp = event
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return (event_type, payload, timestamp);
    }
    let event_type = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload: HashMap<String, Value> = event
        .get("payload")
        .and_then(|v| v.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(|| event.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
    let timestamp = event
        .get("timestamp")
        .or_else(|| event.get("time"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (event_type, payload, timestamp)
}

fn payload_has_req_id(payload: &HashMap<String, Value>, req_id: &str) -> bool {
    let user_input = payload.get("user_input");
    let user_input = match user_input {
        Some(Value::Array(arr)) => arr,
        _ => return false,
    };
    for part in user_input {
        if let Some(obj) = part.as_object() {
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                if text.contains(req_id) {
                    return true;
                }
            }
        }
    }
    false
}

fn append_part(state: &mut HashMap<String, Value>, text: String, continuous: bool) {
    let parts = state
        .entry("parts".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(arr) = parts {
        if continuous && !arr.is_empty() {
            if let Some(Value::String(last)) = arr.last_mut() {
                last.push_str(&text);
            } else {
                arr.push(Value::String(text));
            }
        } else {
            arr.push(Value::String(text));
        }
    }
}

fn text_from_value(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|v| text_from_value(Some(v)))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::Object(obj)) => {
            for key in ["text", "content", "input", "user_input", "message"] {
                if let Some(v) = obj.get(key) {
                    let text = text_from_value(Some(v));
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn coerce_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn new_state(started_at: Option<String>, line: usize) -> HashMap<String, Value> {
    let mut state = HashMap::new();
    if let Some(t) = started_at {
        state.insert("started_at".to_string(), Value::String(t));
    }
    state.insert("line".to_string(), Value::Number(line.into()));
    state.insert("message_id".to_string(), Value::Null);
    state
}

fn observation_from_state(
    path: &Path,
    state: &HashMap<String, Value>,
    req_id: &str,
    completed: bool,
    completed_at: Option<String>,
    line_count: usize,
) -> KimiTurnObservation {
    let parts = state
        .get("parts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let reply = clean_native_reply(&parts, req_id);
    let session_id = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let message_id = state
        .get("message_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let provider_turn_ref = message_id.clone().or_else(|| session_id.clone());
    KimiTurnObservation {
        request_seen: true,
        completed,
        reply,
        session_id,
        session_path: Some(path.to_path_buf()),
        provider_turn_ref,
        line_count,
        native_started_at: state
            .get("started_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        native_completed_at: completed_at,
    }
}

fn observation_sort_key(observation: &KimiTurnObservation) -> (u64, usize) {
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

/// Clean a native CLI reply by stripping banners and done markers.
///
/// Mirrors Python `provider_backends.native_cli_support.clean_native_reply`.
pub fn clean_native_reply(text: &str, req_id: &str) -> String {
    crate::native_cli_support::clean_native_reply(text, req_id)
}

trait PathExt {
    fn expand_home(&self) -> PathBuf;
    fn canonical_or_self(&self) -> PathBuf;
}

impl PathExt for Path {
    fn expand_home(&self) -> PathBuf {
        let s = self.to_string_lossy();
        PathBuf::from(expand_tilde(&s))
    }

    fn canonical_or_self(&self) -> PathBuf {
        std::fs::canonicalize(self).unwrap_or_else(|_| self.to_path_buf())
    }
}

fn expand_tilde(input: impl AsRef<str>) -> String {
    let input = input.as_ref();
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_lines(path: &Path, lines: &[&str]) {
        let mut file = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
    }

    #[test]
    fn test_kimi_project_hash_is_md5() {
        let hash = kimi_project_hash(Path::new("/home/user/proj"));
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_observe_wire_file_detects_turn() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("wire.jsonl");
        write_lines(
            &path,
            &[
                r#"{"type":"turn.prompt","payload":{"user_input":[{"text":"CCB_REQ_ID: req-123"}],"turnId":"turn-1"}}"#,
                r#"{"type":"ContentPart","payload":{"text":"hello"}}"#,
                r#"{"type":"TurnEnd"}"#,
            ],
        );

        let obs = observe_wire_file(&path, "req-123").unwrap();
        assert!(obs.request_seen);
        assert!(obs.completed);
        assert_eq!(obs.reply, "hello");
        assert_eq!(obs.provider_turn_ref, Some("turn-1".to_string()));
    }

    #[test]
    fn test_observe_wire_file_no_match_returns_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("wire.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"turn.prompt","payload":{"user_input":[{"text":"hello"}]}}"#,
        )
        .unwrap();
        assert!(observe_wire_file(&path, "req-123").is_none());
    }

    #[test]
    fn test_observe_kimi_turn_searches_sessions_root() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        let home = tmp.path().join(".kimi");
        let sessions_root = home.join("sessions").join(kimi_project_hash(&work_dir));
        std::fs::create_dir_all(&sessions_root).unwrap();
        let wire_path = sessions_root.join("sess1").join("wire.jsonl");
        std::fs::create_dir(wire_path.parent().unwrap()).unwrap();
        write_lines(
            &wire_path,
            &[
                r#"{"type":"turn.prompt","payload":{"user_input":[{"text":"CCB_REQ_ID: req-abc"}]}}"#,
                r#"{"type":"ContentPart","payload":{"text":"reply"}}"#,
                r#"{"type":"TurnEnd"}"#,
            ],
        );

        let obs = observe_kimi_turn(&work_dir, "req-abc", Some(&[home])).unwrap();
        assert!(obs.completed);
        assert_eq!(obs.reply, "reply");
    }
}
