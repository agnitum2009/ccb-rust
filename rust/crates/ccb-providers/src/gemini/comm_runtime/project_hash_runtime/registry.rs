use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::session::{load_project_session, ClaudeProjectSession};

/// An entry in the Claude session registry.
#[derive(Debug, Clone)]
pub struct ClaudeSessionEntry {
    pub work_dir: PathBuf,
    pub session_file: PathBuf,
    pub session_path: Option<PathBuf>,
    pub session_id: Option<String>,
    pub data: HashMap<String, Value>,
    pub refreshed_at: String,
}

impl ClaudeSessionEntry {
    pub fn from_session(
        work_dir: &Path,
        session: &ClaudeProjectSession,
        refreshed_at: &str,
    ) -> Self {
        Self {
            work_dir: work_dir.to_path_buf(),
            session_file: session.session_file.clone(),
            session_path: session.claude_session_path().map(PathBuf::from),
            session_id: session.claude_session_id().map(|s| s.to_string()),
            data: session.data.clone(),
            refreshed_at: refreshed_at.to_string(),
        }
    }
}

/// Session registry for Claude backend bindings.
///
/// Monitors active sessions and refreshes log bindings periodically to adapt
/// to session switches. Mirrors Python `provider_backends.claude.registry`.
#[derive(Debug, Default)]
pub struct ClaudeSessionRegistry {
    sessions: Mutex<HashMap<String, ClaudeSessionEntry>>,
}

impl ClaudeSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or refresh) the session binding for a work directory.
    pub fn register(&self, work_dir: &Path, now: &str) -> Option<ClaudeSessionEntry> {
        let session = load_project_session(work_dir, None)?;
        let key = registry_key(work_dir);
        let entry = ClaudeSessionEntry::from_session(work_dir, &session, now);
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(key, entry.clone());
        Some(entry)
    }

    /// Remove a work directory from the registry.
    pub fn unregister(&self, work_dir: &Path) -> Option<ClaudeSessionEntry> {
        let key = registry_key(work_dir);
        self.sessions.lock().unwrap().remove(&key)
    }

    /// Return the registered entry for a work directory, if any.
    pub fn get(&self, work_dir: &Path) -> Option<ClaudeSessionEntry> {
        let key = registry_key(work_dir);
        self.sessions.lock().unwrap().get(&key).cloned()
    }

    /// Refresh the binding for a single work directory.
    pub fn refresh(&self, work_dir: &Path, now: &str) -> Option<ClaudeSessionEntry> {
        self.register(work_dir, now)
    }

    /// Refresh all registered bindings.
    pub fn refresh_all(&self, now: &str) -> Vec<ClaudeSessionEntry> {
        let keys: Vec<String> = self.sessions.lock().unwrap().keys().cloned().collect();
        let mut updated = Vec::new();
        for key in keys {
            let work_dir = PathBuf::from(key);
            if let Some(entry) = self.register(&work_dir, now) {
                updated.push(entry);
            }
        }
        updated
    }

    /// List all registered entries.
    pub fn list(&self) -> Vec<ClaudeSessionEntry> {
        self.sessions.lock().unwrap().values().cloned().collect()
    }

    /// Return the current session path for a work directory, if known.
    pub fn session_path_for(&self, work_dir: &Path) -> Option<PathBuf> {
        self.get(work_dir).and_then(|e| e.session_path)
    }
}

fn registry_key(work_dir: &Path) -> String {
    work_dir.to_string_lossy().to_string()
}

/// Global Claude session registry singleton.
static GLOBAL_REGISTRY: OnceLock<ClaudeSessionRegistry> = OnceLock::new();

/// Return the global session registry.
pub fn get_session_registry() -> &'static ClaudeSessionRegistry {
    GLOBAL_REGISTRY.get_or_init(ClaudeSessionRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::TempDir;

    fn write_session(work_dir: &Path, session_path: &str) {
        let data = serde_json::json!({
            "claude_session_id": "session-1",
            "claude_session_path": session_path,
            "work_dir": work_dir.to_string_lossy().to_string(),
        });
        std::fs::write(
            work_dir.join(".claude-session"),
            serde_json::to_string(&data).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_register_and_get() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_session(&work_dir, "/home/user/.claude/projects/proj/session.jsonl");

        let registry = ClaudeSessionRegistry::new();
        let entry = registry
            .register(&work_dir, "2025-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(
            entry.session_path.as_ref().unwrap().to_string_lossy(),
            "/home/user/.claude/projects/proj/session.jsonl"
        );

        let fetched = registry.get(&work_dir).unwrap();
        assert_eq!(fetched.session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn test_unregister() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_session(&work_dir, "/path/session.jsonl");

        let registry = ClaudeSessionRegistry::new();
        registry.register(&work_dir, "2025-01-01T00:00:00Z");
        assert!(registry.unregister(&work_dir).is_some());
        assert!(registry.get(&work_dir).is_none());
    }

    #[test]
    fn test_refresh_updates_session_path() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_session(&work_dir, "/path/old.jsonl");

        let registry = ClaudeSessionRegistry::new();
        registry.register(&work_dir, "2025-01-01T00:00:00Z");

        let mut data: Value = serde_json::from_str(
            &std::fs::read_to_string(work_dir.join(".claude-session")).unwrap(),
        )
        .unwrap();
        data["claude_session_path"] = Value::String("/path/new.jsonl".to_string());
        std::fs::write(
            work_dir.join(".claude-session"),
            serde_json::to_string(&data).unwrap(),
        )
        .unwrap();

        let entry = registry.refresh(&work_dir, "2025-01-01T00:01:00Z").unwrap();
        assert_eq!(
            entry.session_path.as_ref().unwrap().to_string_lossy(),
            "/path/new.jsonl"
        );
    }
}
