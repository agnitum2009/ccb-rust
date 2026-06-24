//! Mirrors Python `lib/provider_backends/claude/registry_runtime/events_runtime/sessions_index.py`.

use std::path::Path;

use crate::claude::registry_runtime::state::ClaudeRuntimeRegistry;
use crate::claude::registry_support::logs_runtime::indexing::parse_sessions_index;
use crate::claude::session::ClaudeProjectSession;

use super::common::{load_session_for_entry, safe_update_binding, update_session_file};

/// Refresh bindings from a sessions index file for watched entries.
pub fn handle_sessions_index<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    project_key: &str,
    index_path: &Path,
) {
    if !index_path.exists() {
        return;
    }
    let keys: Vec<String> = {
        let state = registry.state.lock().unwrap();
        state
            .watchers
            .get(project_key)
            .map(|watcher| watcher.keys.iter().cloned().collect())
            .unwrap_or_default()
    };
    let mut state = registry.state.lock().unwrap();
    for key in keys {
        let Some(entry) = state.sessions.get_mut(&key) else {
            continue;
        };
        if !entry.valid {
            continue;
        }
        let work_dir = entry.work_dir.clone();
        let Some(session_path) = parse_sessions_index(&work_dir, &registry.claude_root) else {
            continue;
        };
        if !session_path.exists() {
            continue;
        }
        let session_id = session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if session_id.is_empty() {
            continue;
        }
        // Update session file on disk.
        std::mem::drop(state);
        update_session_file(registry, &work_dir, &session_path, &session_id);
        state = registry.state.lock().unwrap();
        let entry = match state.sessions.get_mut(&key) {
            Some(e) => e,
            None => continue,
        };
        if let Some(session) = load_session_for_entry(registry, entry) {
            let mut session = session.clone();
            safe_update_binding(Some(&mut session), &session_path, &session_id);
            entry.session = Some(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::registry_runtime::state::{SessionEntry, WatcherEntry};
    use tempfile::TempDir;

    #[test]
    fn test_handle_sessions_index_updates_matching_registry_entries() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("claude-root");
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir_all(work_dir.join(".ccbr")).unwrap();
        let session_file = work_dir.join(".ccbr").join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let project_key = work_dir
            .to_string_lossy()
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>();
        let project_dir = root.join(&project_key);
        std::fs::create_dir_all(&project_dir).unwrap();
        let bound_log = project_dir.join("bound-session.jsonl");
        std::fs::write(&bound_log, "").unwrap();
        std::fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::json!({
                "entries": [
                    {"fullPath": "bound-session.jsonl", "fileMtime": 1000, "projectPath": work_dir.to_string_lossy().to_string()},
                ]
            })
            .to_string(),
        )
        .unwrap();

        let registry = ClaudeRuntimeRegistry::<ClaudeProjectSession>::new(root);
        let session = crate::claude::session::ClaudeProjectSession {
            session_file: session_file.clone(),
            data: serde_json::from_str("{}").unwrap(),
        };
        {
            let mut state = registry.state.lock().unwrap();
            state.sessions.insert(
                "repo".to_string(),
                SessionEntry {
                    work_dir: work_dir.clone(),
                    session: Some(session),
                    session_file: Some(session_file.clone()),
                    file_mtime: 0.0,
                    last_check: 0.0,
                    valid: true,
                    next_bind_refresh: 0.0,
                    bind_backoff_s: 0.0,
                },
            );
            let mut watcher = WatcherEntry::default();
            watcher.keys.insert("repo".to_string());
            state.watchers.insert("project-a".to_string(), watcher);
        }

        handle_sessions_index(
            &registry,
            "project-a",
            &project_dir.join("sessions-index.json"),
        );

        let raw = std::fs::read_to_string(&session_file).unwrap();
        let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            data.get("claude_session_path").unwrap().as_str().unwrap(),
            bound_log.to_str().unwrap()
        );
        assert_eq!(
            data.get("claude_session_id").unwrap().as_str().unwrap(),
            "bound-session"
        );
    }
}
