//! Mirrors Python `lib/provider_backends/claude/registry_runtime/session_updates_runtime.py`.

use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::claude::registry_support::logs_runtime::meta::read_session_meta;
use crate::claude::session_runtime::pathing::{ensure_work_dir_fields, now_str};

/// Read session metadata from a log file, retrying once if no metadata is found.
pub fn read_log_meta_with_retry(log_path: &Path) -> (Option<String>, Option<String>, Option<bool>) {
    for attempt in 0..2 {
        let (work_dir, session_id, is_sidechain) = read_session_meta(log_path);
        if meta_found(&work_dir, &session_id, &is_sidechain) {
            return (work_dir, session_id, is_sidechain);
        }
        if attempt == 0 {
            thread::sleep(Duration::from_millis(200));
        }
    }
    (None, None, None)
}

fn meta_found(
    work_dir: &Option<String>,
    session_id: &Option<String>,
    is_sidechain: &Option<bool>,
) -> bool {
    work_dir.is_some() || session_id.is_some() || *is_sidechain == Some(true)
}

/// Apply a binding update directly to a session file on disk.
pub fn update_session_file_direct(
    session_file: &Path,
    log_path: &Path,
    session_id: &str,
) -> Result<(), String> {
    if !session_file.exists() {
        return Ok(());
    }
    let mut payload = load_session_payload(session_file);
    let (old_path, old_id) = current_binding(&payload);
    let work_dir_path = ensure_work_dir_fields(&mut payload, session_file);
    apply_binding_update(&mut payload, log_path, session_id, &old_path, &old_id);
    write_session_payload(session_file, &payload)?;
    maybe_extract_replaced_session(&old_path, log_path, work_dir_path.as_deref());
    Ok(())
}

fn load_session_payload(session_file: &Path) -> Map<String, Value> {
    std::fs::read_to_string(session_file)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn current_binding(payload: &Map<String, Value>) -> (String, String) {
    (
        payload
            .get("claude_session_path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
        payload
            .get("claude_session_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string(),
    )
}

fn apply_binding_update(
    payload: &mut Map<String, Value>,
    log_path: &Path,
    session_id: &str,
    old_path: &str,
    old_id: &str,
) {
    let timestamp = now_str();
    let new_path = log_path.to_string_lossy().to_string();
    let new_id = session_id.trim();

    mark_old_binding(payload, old_path, old_id, &new_path, new_id, &timestamp);
    payload.insert("claude_session_path".to_string(), Value::String(new_path));
    payload.insert(
        "claude_session_id".to_string(),
        Value::String(new_id.to_string()),
    );
    payload.insert("updated_at".to_string(), Value::String(timestamp));
    if payload.get("active") == Some(&Value::Bool(false)) {
        payload.insert("active".to_string(), Value::Bool(true));
    }
}

fn mark_old_binding(
    payload: &mut Map<String, Value>,
    old_path: &str,
    old_id: &str,
    new_path: &str,
    new_id: &str,
    timestamp: &str,
) {
    let path_changed = !old_path.is_empty() && old_path != new_path;
    let id_changed = !old_id.is_empty() && old_id != new_id;
    if id_changed {
        payload.insert(
            "old_claude_session_id".to_string(),
            Value::String(old_id.to_string()),
        );
    }
    if path_changed {
        payload.insert(
            "old_claude_session_path".to_string(),
            Value::String(old_path.to_string()),
        );
    }
    if path_changed || id_changed {
        payload.insert(
            "old_updated_at".to_string(),
            Value::String(timestamp.to_string()),
        );
    }
}

fn write_session_payload(session_file: &Path, payload: &Map<String, Value>) -> Result<(), String> {
    let content = serde_json::to_string_pretty(payload).map_err(|e| e.to_string())? + "\n";
    ccbr_provider_sessions::safe_write_session(session_file, &content)
}

fn maybe_extract_replaced_session(_old_path: &str, _new_path: &Path, _work_dir: Option<&Path>) {
    // Auto-transfer of replaced session context is handled by the daemon runtime.
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_update_session_file_direct_writes_binding() {
        let tmp = TempDir::new().unwrap();
        let session_file = tmp.path().join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let log_path = tmp.path().join("log.jsonl");

        update_session_file_direct(&session_file, &log_path, "sid-1").unwrap();

        let raw = std::fs::read_to_string(&session_file).unwrap();
        let data: Map<String, Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            data.get("claude_session_path").unwrap().as_str().unwrap(),
            log_path.to_str().unwrap()
        );
        assert_eq!(
            data.get("claude_session_id").unwrap().as_str().unwrap(),
            "sid-1"
        );
    }
}
