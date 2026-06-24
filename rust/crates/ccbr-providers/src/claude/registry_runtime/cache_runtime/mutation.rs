//! Mirrors Python `lib/provider_backends/claude/registry_runtime/cache_runtime/mutation.py`.

use std::path::Path;

use super::super::state::{ClaudeRuntimeRegistry, RegistrySession};

/// Mark a registry entry as invalid.
/// Mirrors Python `invalidate`.
pub fn invalidate<S, W>(
    registry: &ClaudeRuntimeRegistry<S, W>,
    work_dir: &Path,
    write_log_fn: impl FnOnce(&str),
    release_watchers_for_work_dir_fn: impl FnOnce(&Path, &str),
) where
    S: RegistrySession,
{
    let key = work_dir.to_string_lossy().to_string();
    {
        let mut state = registry.state.lock().unwrap();
        if let Some(entry) = state.sessions.get_mut(&key) {
            entry.valid = false;
            write_log_fn(&format!(
                "[INFO] Session invalidated: {}",
                work_dir.display()
            ));
        }
    }
    release_watchers_for_work_dir_fn(work_dir, &key);
}

/// Remove a registry entry entirely.
/// Mirrors Python `remove`.
pub fn remove<S, W>(
    registry: &ClaudeRuntimeRegistry<S, W>,
    work_dir: &Path,
    write_log_fn: impl FnOnce(&str),
    release_watchers_for_work_dir_fn: impl FnOnce(&Path, &str),
) where
    S: RegistrySession,
{
    let key = work_dir.to_string_lossy().to_string();
    {
        let mut state = registry.state.lock().unwrap();
        if state.sessions.remove(&key).is_some() {
            write_log_fn(&format!("[INFO] Session removed: {}", work_dir.display()));
        }
    }
    release_watchers_for_work_dir_fn(work_dir, &key);
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{ClaudeRuntimeRegistry, RegistrySession, SessionEntry};
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct FakeSession {
        _id: String,
    }

    impl RegistrySession for FakeSession {
        fn session_file(&self) -> Option<&Path> {
            None
        }

        fn ensure_pane(&self) -> Result<String, String> {
            Ok("%1".to_string())
        }
    }

    #[test]
    fn test_invalidate_and_remove_release_watchers() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        let registry = ClaudeRuntimeRegistry::<FakeSession>::new(tmp.path().to_path_buf());
        {
            let mut state = registry.state.lock().unwrap();
            state.sessions.insert(
                work_dir.to_string_lossy().to_string(),
                SessionEntry {
                    work_dir: work_dir.clone(),
                    session: Some(FakeSession {
                        _id: "sess".to_string(),
                    }),
                    session_file: None,
                    file_mtime: 0.0,
                    last_check: 0.0,
                    valid: true,
                    next_bind_refresh: 0.0,
                    bind_backoff_s: 0.0,
                },
            );
        }
        let mut released: Vec<(PathBuf, String)> = Vec::new();
        let mut logs: Vec<String> = Vec::new();
        let mut release = |wd: &Path, key: &str| {
            released.push((wd.to_path_buf(), key.to_string()));
        };

        invalidate(
            &registry,
            &work_dir,
            |msg| logs.push(msg.to_string()),
            &mut release,
        );
        remove(
            &registry,
            &work_dir,
            |msg| logs.push(msg.to_string()),
            &mut release,
        );

        assert_eq!(
            logs,
            vec![
                format!("[INFO] Session invalidated: {}", work_dir.display()),
                format!("[INFO] Session removed: {}", work_dir.display()),
            ]
        );
        assert_eq!(
            released,
            vec![
                (work_dir.clone(), work_dir.to_string_lossy().to_string()),
                (work_dir.clone(), work_dir.to_string_lossy().to_string()),
            ]
        );
        assert!(!registry
            .state
            .lock()
            .unwrap()
            .sessions
            .contains_key(&work_dir.to_string_lossy().to_string()));
    }
}
