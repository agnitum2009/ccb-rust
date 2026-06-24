//! Mirrors Python `lib/provider_backends/claude/registry_runtime/events_runtime/project_logs.py`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::claude::registry_runtime::session_updates::read_log_meta_with_retry;
use crate::claude::registry_runtime::state::{ClaudeRuntimeRegistry, SessionEntry};
use crate::claude::registry_support::logs_runtime::binding::should_overwrite_binding;
use crate::claude::registry_support::pathing::path_within;
use crate::claude::session::ClaudeProjectSession;

use super::common::{load_session_for_entry, safe_update_binding, watcher_keys};
use super::sessions_index::handle_sessions_index;

const LOG_RECHECK_WINDOW_S: f64 = 0.4;
const PENDING_LOG_TTL_S: f64 = 120.0;

/// Handle a newly discovered log file under a project watch scope.
pub fn handle_new_log_file<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    project_key: &str,
    path: &Path,
) {
    if _handle_sessions_index_path(registry, project_key, path) {
        return;
    }
    let Some(log_update) = _load_log_update(registry, path) else {
        return;
    };
    let keys = watcher_keys(registry, project_key);
    if keys.is_empty() {
        return;
    }
    if log_update.cwd.is_none() {
        _handle_unscoped_log(registry, keys, &log_update);
    } else {
        _handle_scoped_log(registry, keys, &log_update);
    }
}

fn _handle_sessions_index_path<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    project_key: &str,
    path: &Path,
) -> bool {
    if path.file_name().and_then(|n| n.to_str()) != Some("sessions-index.json") {
        return false;
    }
    handle_sessions_index(registry, project_key, path);
    true
}

#[derive(Debug)]
struct ProjectLogUpdate {
    path: PathBuf,
    path_key: String,
    session_id: String,
    cwd: Option<String>,
    now: f64,
}

fn _load_log_update<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    path: &Path,
) -> Option<ProjectLogUpdate> {
    if !path.exists() {
        return None;
    }
    let now = now_f64();
    if !_should_process_log_path(registry, path, now) {
        return None;
    }
    let (cwd, sid, is_sidechain) = read_log_meta_with_retry(path);
    let path_key = path.to_string_lossy().to_string();
    if is_sidechain == Some(true) {
        _clear_pending_log(registry, &path_key);
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
    Some(ProjectLogUpdate {
        path: path.to_path_buf(),
        path_key,
        session_id,
        cwd: cwd.filter(|s| !s.is_empty()),
        now,
    })
}

fn _should_process_log_path<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    path: &Path,
    now: f64,
) -> bool {
    let path_key = path.to_string_lossy().to_string();
    let mut state = registry.state.lock().unwrap();
    let last_check = state.log_last_check.get(&path_key).copied().unwrap_or(0.0);
    if now - last_check < LOG_RECHECK_WINDOW_S {
        return false;
    }
    state.log_last_check.insert(path_key, now);
    let pending: Vec<(String, f64)> = state
        .pending_logs
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    for (pending_path, ts) in pending {
        if now - ts > PENDING_LOG_TTL_S {
            state.pending_logs.remove(&pending_path);
        }
    }
    true
}

fn _handle_unscoped_log<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    keys: Vec<String>,
    log_update: &ProjectLogUpdate,
) {
    let mut updated_any = false;
    for key in keys {
        let mut state = registry.state.lock().unwrap();
        let Some(entry) = state.sessions.get_mut(&key) else {
            continue;
        };
        if !_valid_entry(entry) {
            continue;
        }
        let should_update = {
            let session = load_session_for_entry(registry, entry);
            session
                .map(|s| _should_update_unscoped_session(s, log_update))
                .unwrap_or(false)
        };
        if should_update {
            if let Some(session_mut) = entry.session.as_mut() {
                if safe_update_binding(Some(session_mut), &log_update.path, &log_update.session_id)
                {
                    updated_any = true;
                }
            }
        }
    }
    _set_pending_state(registry, &log_update.path_key, updated_any, log_update.now);
}

fn _handle_scoped_log<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    keys: Vec<String>,
    log_update: &ProjectLogUpdate,
) {
    let cwd = log_update.cwd.as_deref().unwrap();
    let mut updated_any = false;
    for key in keys {
        let mut state = registry.state.lock().unwrap();
        let Some(entry) = state.sessions.get_mut(&key) else {
            continue;
        };
        if !_valid_entry(entry) {
            continue;
        }
        if !path_within(cwd, &entry.work_dir) {
            continue;
        }
        let should_update = load_session_for_entry(registry, entry).is_some();
        if should_update {
            if let Some(session_mut) = entry.session.as_mut() {
                if safe_update_binding(Some(session_mut), &log_update.path, &log_update.session_id)
                {
                    updated_any = true;
                }
            }
        }
    }
    if updated_any {
        _clear_pending_log(registry, &log_update.path_key);
    }
}

fn _valid_entry(entry: &SessionEntry<ClaudeProjectSession>) -> bool {
    entry.valid
}

fn _should_update_unscoped_session(
    session: &ClaudeProjectSession,
    log_update: &ProjectLogUpdate,
) -> bool {
    let current_path = session
        .claude_session_path()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    if should_overwrite_binding(current_path.as_deref(), &log_update.path) {
        return true;
    }
    session.claude_session_id() != Some(&log_update.session_id)
}

fn _set_pending_state<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    path_key: &str,
    updated: bool,
    now: f64,
) {
    let mut state = registry.state.lock().unwrap();
    if updated {
        state.pending_logs.remove(path_key);
    } else {
        state.pending_logs.insert(path_key.to_string(), now);
    }
}

fn _clear_pending_log<W>(
    registry: &ClaudeRuntimeRegistry<ClaudeProjectSession, W>,
    path_key: &str,
) {
    let mut state = registry.state.lock().unwrap();
    state.pending_logs.remove(path_key);
}

fn now_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::registry_runtime::state::{SessionEntry, WatcherEntry};
    use tempfile::TempDir;

    fn runtime_registry(tmp: &TempDir) -> ClaudeRuntimeRegistry<ClaudeProjectSession> {
        ClaudeRuntimeRegistry::new(tmp.path().join("claude-root"))
    }

    fn fake_session(session_file: &Path) -> ClaudeProjectSession {
        let raw = std::fs::read_to_string(session_file).unwrap();
        let data = serde_json::from_str(&raw).unwrap();
        ClaudeProjectSession {
            session_file: session_file.to_path_buf(),
            data,
        }
    }

    #[test]
    fn test_handle_new_log_file_marks_pending_when_unscoped_log_does_not_update() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let session_file = work_dir.join(".ccbr").join(".claude-session");
        std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
        let current_log = tmp.path().join("current.jsonl");
        let log_path = tmp.path().join("other.jsonl");
        std::fs::write(&current_log, "").unwrap();
        let now = std::time::SystemTime::now();
        filetime::set_file_mtime(&current_log, filetime::FileTime::from_system_time(now)).unwrap();
        std::fs::write(
            &session_file,
            serde_json::json!({
                "work_dir": work_dir.to_string_lossy().to_string(),
                "claude_session_path": current_log.to_string_lossy().to_string(),
                "claude_session_id": "sid-existing",
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            &log_path,
            "{\"sessionId\":\"sid-existing\",\"isSidechain\":false}\n",
        )
        .unwrap();
        filetime::set_file_mtime(
            &log_path,
            filetime::FileTime::from_system_time(now - std::time::Duration::from_secs(20)),
        )
        .unwrap();

        let registry = runtime_registry(&tmp);
        let session = fake_session(&session_file);
        {
            let mut state = registry.state.lock().unwrap();
            state.sessions.insert(
                "repo".to_string(),
                SessionEntry {
                    work_dir: work_dir.clone(),
                    session: Some(session),
                    session_file: Some(session_file),
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

        handle_new_log_file(&registry, "project-a", &log_path);

        let state = registry.state.lock().unwrap();
        assert!(state
            .pending_logs
            .contains_key(&log_path.to_string_lossy().to_string()));
    }
}
