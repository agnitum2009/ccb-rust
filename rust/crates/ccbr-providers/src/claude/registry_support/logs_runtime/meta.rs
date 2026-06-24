//! Mirrors Python `lib/provider_backends/claude/registry_support/logs_runtime/meta.py`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// Read session metadata (cwd, session_id, is_sidechain) from the head of a log file.
pub fn read_session_meta(log_path: &Path) -> (Option<String>, Option<String>, Option<bool>) {
    let raw = match std::fs::read_to_string(log_path) {
        Ok(raw) => raw,
        Err(_) => return (None, None, None),
    };
    for line in raw.lines().take(30) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(entry) = parse_meta_entry(line) {
            if let Some(meta) = session_meta_tuple(&entry) {
                return meta;
            }
        }
    }
    (None, None, None)
}

fn parse_meta_entry(line: &str) -> Option<HashMap<String, Value>> {
    let value: Value = serde_json::from_str(line).ok()?;
    value
        .as_object()
        .cloned()
        .map(|obj| obj.into_iter().collect())
}

fn session_meta_tuple(
    entry: &HashMap<String, Value>,
) -> Option<(Option<String>, Option<String>, Option<bool>)> {
    let cwd = normalized_meta_text(entry.get("cwd").or_else(|| entry.get("projectPath")));
    let sid = normalized_meta_text(entry.get("sessionId").or_else(|| entry.get("id")));
    let sidechain = sidechain_flag(entry.get("isSidechain"));
    if cwd.is_some() || sid.is_some() {
        Some((cwd, sid, sidechain))
    } else {
        None
    }
}

fn normalized_meta_text(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn sidechain_flag(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_session_meta_extracts_cwd_and_session_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("log.jsonl");
        std::fs::write(
            &path,
            "{\"other\":true}\n{\"cwd\":\"/repo\",\"sessionId\":\"sid-1\",\"isSidechain\":false}\n",
        )
        .unwrap();
        let (cwd, sid, sidechain) = read_session_meta(&path);
        assert_eq!(cwd.as_deref(), Some("/repo"));
        assert_eq!(sid.as_deref(), Some("sid-1"));
        assert_eq!(sidechain, Some(false));
    }

    #[test]
    fn test_read_session_meta_returns_none_for_missing_meta() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("log.jsonl");
        std::fs::write(&path, "{\"other\":true}\n").unwrap();
        let (cwd, sid, sidechain) = read_session_meta(&path);
        assert!(cwd.is_none());
        assert!(sid.is_none());
        assert!(sidechain.is_none());
    }
}
