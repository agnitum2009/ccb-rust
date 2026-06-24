//! Mirrors Python `lib/provider_backends/claude/registry_runtime/cache_runtime/lookup.py`.

use std::path::{Path, PathBuf};

use super::super::state::{ClaudeRuntimeRegistry, RegistrySession, SessionEntry};

/// Return the cached session for a work directory, reloading if the session file changed.
/// Mirrors Python `get_session`.
pub fn get_session<S, W>(
    registry: &ClaudeRuntimeRegistry<S, W>,
    work_dir: &Path,
    find_session_file_fn: impl FnOnce(&Path) -> Option<PathBuf>,
    mut write_log_fn: impl FnMut(&str),
    load_and_cache_fn: impl FnOnce(&Path) -> Option<SessionEntry<S>>,
) -> Option<S>
where
    S: RegistrySession,
{
    let key = work_dir.to_string_lossy().to_string();
    let should_reload = {
        let state = registry.state.lock().unwrap();
        if let Some(entry) = state.sessions.get(&key) {
            let session_file = session_file_for_entry(entry, find_session_file_fn, work_dir);
            if should_reload(entry, session_file.as_deref(), work_dir, &mut write_log_fn) {
                true
            } else if entry.valid {
                return entry.session.clone();
            } else {
                true
            }
        } else {
            true
        }
    };

    if should_reload {
        if let Some(entry) = load_and_cache_fn(work_dir) {
            return entry.session;
        }
    }
    None
}

fn session_file_for_entry<S>(
    entry: &SessionEntry<S>,
    find_session_file_fn: impl FnOnce(&Path) -> Option<PathBuf>,
    work_dir: &Path,
) -> Option<PathBuf>
where
    S: RegistrySession,
{
    entry
        .session_file
        .clone()
        .or_else(|| find_session_file_fn(work_dir))
}

fn should_reload<S>(
    entry: &SessionEntry<S>,
    session_file: Option<&Path>,
    work_dir: &Path,
    write_log_fn: &mut dyn FnMut(&str),
) -> bool
where
    S: RegistrySession,
{
    let Some(session_file) = session_file else {
        return false;
    };
    if !session_file.exists() {
        return false;
    }
    let Ok(metadata) = session_file.metadata() else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let current_mtime = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if entry.session_file.is_none()
        || entry.session_file.as_deref() != Some(session_file)
        || current_mtime != entry.file_mtime
    {
        write_log_fn(&format!(
            "[INFO] Session file changed, reloading: {}",
            work_dir.display()
        ));
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{ClaudeRuntimeRegistry, RegistrySession, SessionEntry};
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct FakeSession {
        session_file: Option<PathBuf>,
        id: String,
    }

    impl RegistrySession for FakeSession {
        fn session_file(&self) -> Option<&Path> {
            self.session_file.as_deref()
        }

        fn ensure_pane(&self) -> Result<String, String> {
            Ok("%1".to_string())
        }
    }

    #[test]
    fn test_get_session_reloads_when_session_file_mtime_changes() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let session_file = tmp.path().join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let registry = ClaudeRuntimeRegistry::<FakeSession>::new(tmp.path().to_path_buf());
        {
            let mut state = registry.state.lock().unwrap();
            state.sessions.insert(
                work_dir.to_string_lossy().to_string(),
                SessionEntry {
                    work_dir: work_dir.clone(),
                    session: Some(FakeSession {
                        session_file: Some(session_file.clone()),
                        id: "cached-session".to_string(),
                    }),
                    session_file: Some(session_file.clone()),
                    file_mtime: 0.0,
                    last_check: 0.0,
                    valid: true,
                    next_bind_refresh: 0.0,
                    bind_backoff_s: 0.0,
                },
            );
        }
        let mut logs: Vec<String> = Vec::new();

        let reloaded = get_session(
            &registry,
            &work_dir,
            |_wd| Some(session_file.clone()),
            |msg| logs.push(msg.to_string()),
            |_wd| {
                Some(SessionEntry {
                    work_dir: work_dir.clone(),
                    session: Some(FakeSession {
                        session_file: Some(session_file.clone()),
                        id: "reloaded-session".to_string(),
                    }),
                    session_file: Some(session_file.clone()),
                    file_mtime: 0.0,
                    last_check: 0.0,
                    valid: true,
                    next_bind_refresh: 0.0,
                    bind_backoff_s: 0.0,
                })
            },
        );

        assert_eq!(reloaded.unwrap().id, "reloaded-session");
        assert_eq!(
            logs,
            vec![format!(
                "[INFO] Session file changed, reloading: {}",
                work_dir.display()
            )]
        );
    }
}
