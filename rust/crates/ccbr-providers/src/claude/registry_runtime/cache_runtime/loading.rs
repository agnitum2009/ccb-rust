//! Mirrors Python `lib/provider_backends/claude/registry_runtime/cache_runtime/loading.py`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::state::{ClaudeRuntimeRegistry, RegistrySession, SessionEntry};

/// Register a healthy session into the registry and start watchers.
/// Mirrors Python `register_session`.
pub fn register_session<S, W>(
    registry: &ClaudeRuntimeRegistry<S, W>,
    work_dir: &Path,
    session: S,
    ensure_watchers_for_work_dir_fn: impl FnOnce(&Path, &str),
) where
    S: RegistrySession,
{
    let key = work_dir.to_string_lossy().to_string();
    let session_file = session.session_file().map(PathBuf::from);
    let entry = SessionEntry {
        work_dir: work_dir.to_path_buf(),
        session: Some(session),
        session_file: session_file.clone(),
        file_mtime: file_mtime(session_file.as_deref()),
        last_check: now(),
        valid: true,
        next_bind_refresh: 0.0,
        bind_backoff_s: 0.0,
    };
    {
        let mut state = registry.state.lock().unwrap();
        state.sessions.insert(key.clone(), entry);
    }
    ensure_watchers_for_work_dir_fn(work_dir, &key);
}

/// Load and cache a session for a work directory.
/// Mirrors Python `load_and_cache`.
pub fn load_and_cache<S, W>(
    registry: &ClaudeRuntimeRegistry<S, W>,
    work_dir: &Path,
    load_session_fn: impl FnOnce(&Path) -> Option<S>,
    find_session_file_fn: impl FnOnce(&Path) -> Option<PathBuf>,
) -> Option<SessionEntry<S>>
where
    S: RegistrySession,
{
    let session = load_session_fn(work_dir);
    let session_file = session_file_for_loading(work_dir, session.as_ref(), find_session_file_fn);
    let valid = session_valid(session.as_ref());
    let entry = SessionEntry {
        work_dir: work_dir.to_path_buf(),
        session,
        session_file: session_file
            .as_ref()
            .and_then(|p| p.exists().then(|| p.clone())),
        file_mtime: file_mtime(session_file.as_deref()),
        last_check: now(),
        valid,
        next_bind_refresh: 0.0,
        bind_backoff_s: 0.0,
    };
    let key = work_dir.to_string_lossy().to_string();
    {
        let mut state = registry.state.lock().unwrap();
        state.sessions.insert(key, entry);
    }
    if valid {
        let mut state = registry.state.lock().unwrap();
        state
            .sessions
            .remove(&work_dir.to_string_lossy().to_string())
    } else {
        None
    }
}

fn session_file_for_loading<S>(
    work_dir: &Path,
    session: Option<&S>,
    find_session_file_fn: impl FnOnce(&Path) -> Option<PathBuf>,
) -> Option<PathBuf>
where
    S: RegistrySession,
{
    if let Some(s) = session {
        if let Some(sf) = s.session_file() {
            return Some(sf.to_path_buf());
        }
    }
    find_session_file_fn(work_dir)
}

fn session_valid<S>(session: Option<&S>) -> bool
where
    S: RegistrySession,
{
    let Some(session) = session else { return false };
    session.ensure_pane().is_ok()
}

fn file_mtime(path: Option<&Path>) -> f64 {
    let Some(path) = path else { return 0.0 };
    if !path.exists() {
        return 0.0;
    }
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{ClaudeRuntimeRegistry, RegistrySession};
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct FakeSession {
        session_file: Option<PathBuf>,
        ensure_ok: bool,
    }

    impl RegistrySession for FakeSession {
        fn session_file(&self) -> Option<&Path> {
            self.session_file.as_deref()
        }

        fn ensure_pane(&self) -> Result<String, String> {
            if self.ensure_ok {
                Ok("%1".to_string())
            } else {
                Err("pane dead".to_string())
            }
        }
    }

    #[test]
    fn test_register_session_stores_valid_entry_and_ensures_watchers() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let session_file = tmp.path().join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let registry = ClaudeRuntimeRegistry::<FakeSession>::new(tmp.path().to_path_buf());
        let session = FakeSession {
            session_file: Some(session_file.clone()),
            ensure_ok: true,
        };
        let mut watcher_calls: Vec<(PathBuf, String)> = Vec::new();

        register_session(&registry, &work_dir, session, |wd, key| {
            watcher_calls.push((wd.to_path_buf(), key.to_string()));
        });

        let state = registry.state.lock().unwrap();
        let entry = state
            .sessions
            .get(&work_dir.to_string_lossy().to_string())
            .unwrap();
        assert!(entry.valid);
        assert_eq!(entry.session_file, Some(session_file));
        assert_eq!(
            watcher_calls,
            vec![(work_dir.clone(), work_dir.to_string_lossy().to_string())]
        );
    }

    #[test]
    fn test_load_and_cache_returns_none_for_unhealthy_session_but_caches_entry() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("repo");
        std::fs::create_dir(&work_dir).unwrap();
        let session_file = tmp.path().join(".claude-session");
        std::fs::write(&session_file, "{}").unwrap();
        let registry = ClaudeRuntimeRegistry::<FakeSession>::new(tmp.path().to_path_buf());

        let result = load_and_cache(
            &registry,
            &work_dir,
            |_wd| {
                Some(FakeSession {
                    session_file: Some(session_file.clone()),
                    ensure_ok: false,
                })
            },
            |_wd| Some(session_file.clone()),
        );

        assert!(result.is_none());
        let state = registry.state.lock().unwrap();
        let entry = state
            .sessions
            .get(&work_dir.to_string_lossy().to_string())
            .unwrap();
        assert!(!entry.valid);
        assert_eq!(entry.session_file, Some(session_file));
    }
}
