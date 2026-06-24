use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::contracts::ProviderSessionBinding;
use ccbr_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use serde_json::Value;

pub const PROVIDER_NAME: &str = "opencode";
pub const SESSION_FILENAME: &str = ".opencode-session";

/// Build the OpenCode session binding.
pub fn build_session_binding() -> ProviderSessionBinding {
    ProviderSessionBinding {
        provider: PROVIDER_NAME.to_string(),
        session_id_attr: "opencode_session_id".to_string(),
        session_path_attr: "session_file".to_string(),
    }
}

/// An OpenCode project session loaded from disk.
#[derive(Debug, Clone)]
pub struct OpenCodeProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl OpenCodeProjectSession {
    pub fn ccbr_session_id(&self) -> Option<&str> {
        self.data
            .get("ccbr_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn opencode_session_id(&self) -> Option<&str> {
        self.data
            .get("opencode_session_id")
            .or_else(|| self.data.get("opencode_storage_session_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn opencode_session_id_filter(&self) -> Option<String> {
        self.opencode_session_id().map(|s| s.to_string())
    }

    pub fn opencode_project_id(&self) -> Option<&str> {
        self.data
            .get("opencode_project_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn work_dir(&self) -> Option<&str> {
        self.data
            .get("work_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn pane_id(&self) -> Option<&str> {
        self.data
            .get("pane_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn terminal(&self) -> Option<&str> {
        self.data
            .get("terminal")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn start_cmd(&self) -> Option<&str> {
        self.data
            .get("start_cmd")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn runtime_dir(&self) -> Option<&str> {
        self.data
            .get("runtime_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    pub fn pane_title_marker(&self) -> Option<&str> {
        self.data
            .get("pane_title_marker")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
    }

    /// Ensure the tmux pane for this session exists and is alive.
    /// Mirrors Python `provider_backends.opencode.session_runtime.lifecycle.ensure_pane`.
    pub fn ensure_pane(&self) -> Result<String, String> {
        self.ensure_pane_with_backend(get_backend_for_session(&self.data))
    }

    /// Test seam: ensure pane using an injected backend.
    pub fn ensure_pane_with_backend(
        &self,
        backend: Option<Box<dyn OpencodeBackend>>,
    ) -> Result<String, String> {
        let mut backend = backend.ok_or_else(|| "Terminal backend not available".to_string())?;
        let pane_id = self
            .pane_id()
            .ok_or_else(|| format!("Pane not alive: {:?}", self.pane_id()))?;

        if backend.is_alive(pane_id) {
            return Ok(pane_id.to_string());
        }

        if self.terminal() == Some("tmux") {
            if backend.pane_exists(pane_id) {
                let start_cmd = self
                    .start_cmd()
                    .ok_or_else(|| format!("respawn failed: no start_cmd for pane {}", pane_id))?;
                let cwd = self
                    .runtime_dir()
                    .or_else(|| self.work_dir())
                    .map(PathBuf::from);
                backend.respawn_pane(pane_id, start_cmd, cwd.as_deref());
                if backend.is_alive(pane_id) {
                    return Ok(pane_id.to_string());
                }
                return Err(format!("respawn failed for pane {}", pane_id));
            } else {
                return Err(format!("respawn failed: pane {} no longer exists", pane_id));
            }
        }

        Err(format!("Pane not alive: {}", pane_id))
    }
}

/// Backend abstraction used by OpenCode pane lifecycle.
/// Tests provide a fake backend; production wiring can plug into a concrete
/// tmux backend via `get_backend_for_session`.
pub trait OpencodeBackend {
    fn is_alive(&self, pane_id: &str) -> bool;
    fn pane_exists(&self, pane_id: &str) -> bool;
    fn respawn_pane(&mut self, pane_id: &str, cmd: &str, cwd: Option<&Path>);
    fn save_crash_log(&mut self, _pane_id: &str, _crash_log_path: &Path, _lines: usize) {}
}

/// Resolve a terminal backend for an OpenCode session.
/// Production wiring should delegate to the terminal-runtime backend registry;
/// by default this returns `None` so the module stays provider-core agnostic.
pub fn get_backend_for_session(_data: &HashMap<String, Value>) -> Option<Box<dyn OpencodeBackend>> {
    None
}

/// Find the OpenCode session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load the OpenCode project session for a work directory.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<OpenCodeProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let data = read_json(&session_file)?;
    if data.is_empty() {
        return None;
    }
    Some(OpenCodeProjectSession { session_file, data })
}

/// Load an OpenCode project session for an agent without falling back to the
/// primary session when the agent is named.
///
/// Mirrors Python `provider_backends.opencode.execution_runtime.helpers.load_session`.
pub fn load_session<F>(
    work_dir: &Path,
    agent_name: &str,
    primary_agent: &str,
    load_project_session_fn: F,
) -> Option<OpenCodeProjectSession>
where
    F: FnOnce(&Path, Option<&str>) -> Option<OpenCodeProjectSession>,
{
    let instance =
        ccbr_provider_core::instance_resolution::named_agent_instance(agent_name, primary_agent);
    load_project_session_fn(work_dir, instance.as_deref())
}

fn read_json(path: &Path) -> Option<HashMap<String, Value>> {
    let raw = std::fs::read_to_string(path).ok()?;
    // Strip UTF-8 BOM if present.
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let value: Value = serde_json::from_str(raw).ok()?;
    value
        .as_object()
        .cloned()
        .map(|obj| obj.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_session_binding_fields() {
        let binding = build_session_binding();
        assert_eq!(binding.provider, PROVIDER_NAME);
        assert_eq!(binding.session_id_attr, "opencode_session_id");
        assert_eq!(binding.session_path_attr, "session_file");
    }

    #[test]
    fn test_load_project_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".opencode-session",
            serde_json::json!({
                "opencode_session_id": "session-1",
                "opencode_project_id": "proj1",
                "work_dir": work_dir.to_string_lossy().to_string(),
                "pane_id": "%1",
            }),
        );
        let session = load_project_session(&work_dir, None).unwrap();
        assert_eq!(session.opencode_session_id(), Some("session-1"));
        assert_eq!(session.opencode_project_id(), Some("proj1"));
        assert_eq!(session.pane_id(), Some("%1"));
    }

    struct FakeBackend {
        alive: std::collections::HashMap<String, bool>,
        exists: std::collections::HashMap<String, bool>,
        respawned: std::cell::RefCell<Vec<String>>,
        crash_logs: std::cell::RefCell<Vec<(String, PathBuf)>>,
    }

    impl Default for FakeBackend {
        fn default() -> Self {
            Self {
                alive: std::collections::HashMap::new(),
                exists: std::collections::HashMap::new(),
                respawned: std::cell::RefCell::new(Vec::new()),
                crash_logs: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl OpencodeBackend for FakeBackend {
        fn is_alive(&self, pane_id: &str) -> bool {
            self.alive.get(pane_id).copied().unwrap_or(false)
        }

        fn pane_exists(&self, pane_id: &str) -> bool {
            self.exists
                .get(pane_id)
                .copied()
                .unwrap_or_else(|| self.is_alive(pane_id))
        }

        fn respawn_pane(&mut self, pane_id: &str, _cmd: &str, _cwd: Option<&Path>) {
            self.respawned.borrow_mut().push(pane_id.to_string());
            self.alive.insert(pane_id.to_string(), true);
        }

        fn save_crash_log(&mut self, pane_id: &str, crash_log_path: &Path, _lines: usize) {
            self.crash_logs
                .borrow_mut()
                .push((pane_id.to_string(), crash_log_path.to_path_buf()));
        }
    }

    fn fake_session(work_dir: &Path, extra: serde_json::Value) -> OpenCodeProjectSession {
        let mut data = serde_json::json!({
            "ccbr_session_id": "test-session",
            "terminal": "tmux",
            "pane_id": "%1",
            "pane_title_marker": "CCBR-opencode-test",
            "runtime_dir": work_dir.to_string_lossy().to_string(),
            "work_dir": work_dir.to_string_lossy().to_string(),
            "active": true,
        });
        if let Some(obj) = extra.as_object() {
            for (k, v) in obj {
                data[k] = v.clone();
            }
        }
        let session_file = work_dir.join(".opencode-session");
        std::fs::write(&session_file, serde_json::to_string(&data).unwrap()).unwrap();
        load_project_session(work_dir, None).unwrap()
    }

    #[test]
    fn test_ensure_pane_respawns_recorded_pane_without_marker_rebind() {
        let tmp = TempDir::new().unwrap();
        let session = fake_session(tmp.path(), serde_json::json!({"start_cmd": "sleep 1"}));
        let backend = FakeBackend {
            alive: [("%1".to_string(), false), ("%2".to_string(), true)]
                .into_iter()
                .collect(),
            exists: [("%1".to_string(), true)].into_iter().collect(),
            ..Default::default()
        };
        let result = session.ensure_pane_with_backend(Some(Box::new(backend)));
        assert_eq!(result, Ok("%1".to_string()));
    }

    #[test]
    fn test_ensure_pane_already_alive() {
        let tmp = TempDir::new().unwrap();
        let session = fake_session(tmp.path(), serde_json::Value::Null);
        let backend = FakeBackend {
            alive: [("%1".to_string(), true)].into_iter().collect(),
            ..Default::default()
        };
        let result = session.ensure_pane_with_backend(Some(Box::new(backend)));
        assert_eq!(result, Ok("%1".to_string()));
    }

    #[test]
    fn test_ensure_pane_no_backend() {
        let tmp = TempDir::new().unwrap();
        let session = fake_session(tmp.path(), serde_json::json!({"terminal": "unknown"}));
        let result = session.ensure_pane_with_backend(None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("backend"));
    }

    #[test]
    fn test_ensure_pane_dead_no_marker() {
        let tmp = TempDir::new().unwrap();
        let session = fake_session(tmp.path(), serde_json::json!({"terminal": "unknown"}));
        let backend = FakeBackend {
            alive: [("%1".to_string(), false)].into_iter().collect(),
            exists: [("%1".to_string(), true)].into_iter().collect(),
            ..Default::default()
        };
        let result = session.ensure_pane_with_backend(Some(Box::new(backend)));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("not alive"));
    }

    #[test]
    fn test_ensure_pane_missing_tmux_target_skips_respawn_noise() {
        let tmp = TempDir::new().unwrap();
        let session = fake_session(tmp.path(), serde_json::json!({"start_cmd": "sleep 1"}));
        let backend = FakeBackend {
            alive: [("%1".to_string(), false)].into_iter().collect(),
            exists: [("%1".to_string(), false)].into_iter().collect(),
            ..Default::default()
        };
        let result = session.ensure_pane_with_backend(Some(Box::new(backend)));
        let err = result.unwrap_err().to_lowercase();
        assert!(err.contains("respawn failed"));
        assert!(err.contains("no longer exists"));
    }
}
