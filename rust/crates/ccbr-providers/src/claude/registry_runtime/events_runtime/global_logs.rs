//! Mirrors Python `lib/provider_backends/claude/registry_runtime/events_runtime/global_logs.py`.

use std::path::{Path, PathBuf};

use crate::claude::registry_runtime::session_updates::read_log_meta_with_retry;
use crate::claude::registry_runtime::state::ClaudeRuntimeRegistry;
use crate::claude::session::ClaudeProjectSession;

use super::common::{safe_update_binding, update_session_file};
use super::sessions_index::handle_sessions_index;

/// Handle a newly discovered log file in the global Claude logs root.
pub fn handle_new_log_file_global<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    path: &Path,
) {
    if path.file_name().and_then(|n| n.to_str()) == Some("sessions-index.json") {
        let project_key = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        handle_sessions_index(registry, &project_key, path);
        return;
    }
    if !path.exists() {
        return;
    }
    let Some((work_dir, session_id)) = _discover_log_binding(path) else {
        return;
    };

    update_session_file(registry, &work_dir, path, &session_id);
    let key = work_dir.to_string_lossy().to_string();
    let session = {
        let state = registry.state.lock().unwrap();
        state
            .sessions
            .get(&key)
            .and_then(|entry| entry.session.clone())
    };
    if let Some(mut session) = session {
        safe_update_binding(Some(&mut session), path, &session_id);
        let mut state = registry.state.lock().unwrap();
        if let Some(entry) = state.sessions.get_mut(&key) {
            entry.session = Some(session);
        }
    }
}

fn _discover_log_binding(path: &Path) -> Option<(PathBuf, String)> {
    let (cwd, sid, is_sidechain) = read_log_meta_with_retry(path);
    if is_sidechain == Some(true) || cwd.is_none() {
        return None;
    }
    let session_id = sid.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string()
    });
    if session_id.is_empty() {
        return None;
    }
    Some((PathBuf::from(cwd.unwrap()), session_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::registry_runtime::state::SessionEntry;
    use tempfile::TempDir;

    fn runtime_registry(tmp: &TempDir) -> ClaudeRuntimeRegistry<ClaudeProjectSession> {
        ClaudeRuntimeRegistry::new(tmp.path().join("claude-root"))
    }

    #[test]
    fn test_handle_new_log_file_global_updates_session_and_session_file() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir_all(work_dir.join(".ccbr")).unwrap();
        let session_file = work_dir.join(".ccbr").join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();

        let registry = runtime_registry(&tmp);
        let session = crate::claude::session::ClaudeProjectSession {
            session_file: session_file.clone(),
            data: serde_json::from_str("{}").unwrap(),
        };
        {
            let mut state = registry.state.lock().unwrap();
            state.sessions.insert(
                work_dir.to_string_lossy().to_string(),
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
        }

        let log_path = tmp.path().join("claude-log.jsonl");
        std::fs::write(
            &log_path,
            format!(
                "{{\"cwd\":\"{}\",\"sessionId\":\"sid-1\",\"isSidechain\":false}}\n",
                work_dir.display()
            ),
        )
        .unwrap();

        handle_new_log_file_global(&registry, &log_path);

        let raw = std::fs::read_to_string(&session_file).unwrap();
        let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw).unwrap();
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
