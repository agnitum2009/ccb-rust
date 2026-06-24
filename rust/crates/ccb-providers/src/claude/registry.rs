use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use super::session::ClaudeProjectSession;

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
        let session = load_claude_session(work_dir)?;
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

    /// Find the Claude session file for a work directory, respecting workspace bindings.
    /// Mirrors Python `ClaudeRegistrySessionMixin._find_claude_session_file`.
    pub fn _find_claude_session_file(&self, work_dir: &Path) -> Option<PathBuf> {
        find_claude_session_file(work_dir)
    }

    /// Load the Claude project session for a work directory.
    /// Mirrors Python `ClaudeRegistrySessionMixin._load_claude_session`.
    pub fn _load_claude_session(&self, work_dir: &Path) -> Option<ClaudeProjectSession> {
        load_claude_session(work_dir)
    }
}

const CLAUDE_SESSION_FILENAME: &str = ".claude-session";

pub(crate) fn load_claude_session(work_dir: &Path) -> Option<ClaudeProjectSession> {
    let session_file = find_claude_session_file(work_dir)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(ClaudeProjectSession { session_file, data })
}

pub(crate) fn find_claude_session_file(work_dir: &Path) -> Option<PathBuf> {
    if let Some(session_file) = super::session::find_project_session_file(work_dir, None) {
        return Some(session_file);
    }
    let binding = workspace_binding_for_dir(work_dir)?;
    let target_project = binding.target_project?;
    let agent_name = binding.agent_name?;
    let instance =
        ccb_provider_core::instance_resolution::named_agent_instance(&agent_name, "claude");
    let filename = ccb_provider_core::pathing::session_filename_for_instance(
        CLAUDE_SESSION_FILENAME,
        instance.as_deref(),
    );
    let ccb_dir = target_project.join(".ccbr");
    let candidate = ccb_dir.join(&filename);
    candidate.exists().then_some(candidate)
}

#[derive(Debug, Default)]
struct WorkspaceBinding {
    target_project: Option<PathBuf>,
    agent_name: Option<String>,
}

fn workspace_binding_for_dir(work_dir: &Path) -> Option<WorkspaceBinding> {
    let binding_path = work_dir.join(".ccbr-workspace.json");
    if !binding_path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(&binding_path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let obj = value.as_object()?;
    let target_project = obj
        .get("target_project")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let agent_name = obj
        .get("agent_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(WorkspaceBinding {
        target_project,
        agent_name,
    })
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

    #[test]
    fn test_registry_resolves_named_workspace_session_file() {
        let tmp = TempDir::new().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir(&project_root).unwrap();
        let workspace = tmp.path().join("workspace-agent3");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::write(
            workspace.join(".ccbr-workspace.json"),
            serde_json::json!({
                "schema_version": 2,
                "record_type": "workspace_binding",
                "target_project": project_root.to_string_lossy().to_string(),
                "project_id": "demo-project",
                "agent_name": "agent3",
                "workspace_mode": "linked",
                "workspace_path": workspace.to_string_lossy().to_string(),
            })
            .to_string(),
        )
        .unwrap();
        let ccb_dir = project_root.join(".ccbr");
        std::fs::create_dir(&ccb_dir).unwrap();
        let session_file = ccb_dir.join(".claude-agent3-session");
        std::fs::write(
            &session_file,
            serde_json::json!({
                "active": true,
                "work_dir": workspace.to_string_lossy().to_string(),
                "claude_session_id": "sid",
            })
            .to_string(),
        )
        .unwrap();

        let registry = ClaudeSessionRegistry::new();
        assert_eq!(
            registry._find_claude_session_file(&workspace),
            Some(session_file.clone())
        );
        let loaded = registry._load_claude_session(&workspace).unwrap();
        assert_eq!(loaded.session_file, session_file);
    }
}
