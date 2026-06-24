//! Mirrors Python `lib/provider_backends/opencode/comm.py`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

/// Re-export the SQLite-capable log reader.
pub use super::reader::OpenCodeLogReader;

type SessionLoaderFn = dyn Fn() -> Option<HashMap<String, Value>>;

/// OpenCode communicator facade.
/// Mirrors Python `provider_backends.opencode.runtime.communicator_facade.OpenCodeCommunicator`.
#[derive(Default)]
pub struct OpenCodeCommunicator {
    pub session_info: HashMap<String, Value>,
    pub ccbr_session_id: String,
    pub runtime_dir: PathBuf,
    pub terminal: String,
    pub pane_id: String,
    pub pane_title_marker: String,
    pub backend: Value,
    pub timeout: i64,
    pub marker_prefix: String,
    pub project_session_file: Option<String>,
    pub log_reader: Option<OpenCodeLogReader>,
    load_session_info_fn: Option<Box<SessionLoaderFn>>,
}

impl OpenCodeCommunicator {
    /// Create a new communicator with the default session loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Test seam: create a communicator with an injected session loader.
    pub fn with_session_loader<F>(load_fn: F) -> Self
    where
        F: Fn() -> Option<HashMap<String, Value>> + 'static,
    {
        Self {
            load_session_info_fn: Some(Box::new(load_fn)),
            ..Self::default()
        }
    }

    /// Load session info from disk, backfilling computed fields.
    /// Mirrors Python `OpenCodeCommunicator._load_session_info`.
    pub fn load_session_info(&self) -> Option<HashMap<String, Value>> {
        if let Some(ref loader) = self.load_session_info_fn {
            let mut info = loader()?;
            Self::backfill_session_info(&mut info);
            return Some(info);
        }
        let session_file = self.find_session_file()?;
        let raw = std::fs::read_to_string(&session_file).ok()?;
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
        let mut info: HashMap<String, Value> = serde_json::from_str::<Value>(raw)
            .ok()?
            .as_object()?
            .clone()
            .into_iter()
            .collect();
        info.insert(
            "_session_file".to_string(),
            Value::String(session_file.to_string_lossy().to_string()),
        );
        Self::backfill_session_info(&mut info);
        Some(info)
    }

    fn backfill_session_info(info: &mut HashMap<String, Value>) {
        if !info.contains_key("opencode_session_id") {
            if let Some(sid) = info
                .get("opencode_storage_session_id")
                .and_then(|v| v.as_str())
            {
                info.insert(
                    "opencode_session_id".to_string(),
                    Value::String(sid.to_string()),
                );
            }
        }
        if !info.contains_key("opencode_project_id") {
            if let Some(pid) = info
                .get("opencode_storage_project_id")
                .and_then(|v| v.as_str())
            {
                info.insert(
                    "opencode_project_id".to_string(),
                    Value::String(pid.to_string()),
                );
            }
        }
    }

    /// Find the OpenCode session file.
    /// Mirrors Python `OpenCodeCommunicator._find_session_file`.
    pub fn find_session_file(&self) -> Option<PathBuf> {
        if let Ok(env) = std::env::var("CCB_SESSION_FILE") {
            let trimmed = env.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                if path.exists() {
                    return Some(path);
                }
            }
        }
        // Resolve from runtime_dir or current directory.
        let base = if self.runtime_dir.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            self.runtime_dir.clone()
        };
        super::session::find_project_session_file(&base, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_load_session_info_backfills_project_fields() {
        let tmp = TempDir::new().unwrap();
        let runtime_dir = tmp.path().join("run");
        std::fs::create_dir(&runtime_dir).unwrap();
        let session_file = tmp.path().join(".opencode-session");
        write_json(
            tmp.path(),
            ".opencode-session",
            serde_json::json!({
                "active": true,
                "runtime_dir": runtime_dir.to_string_lossy().to_string(),
                "terminal": "tmux",
                "pane_id": "%1",
                "work_dir": tmp.path().to_string_lossy().to_string(),
                "ccbr_session_id": "ccbr-opencode-test",
                "opencode_session_id": "ses_123",
                "opencode_project_id": "proj_123",
                "_session_file": session_file.to_string_lossy().to_string(),
            }),
        );

        let comm = OpenCodeCommunicator::with_session_loader({
            let session_file = session_file.clone();
            move || {
                let raw = std::fs::read_to_string(&session_file).ok()?;
                let value: Value = serde_json::from_str(&raw).ok()?;
                Some(value.as_object()?.clone().into_iter().collect())
            }
        });
        let info = comm.load_session_info().unwrap();
        assert_eq!(
            info.get("_session_file").unwrap().as_str().unwrap(),
            session_file.to_string_lossy().to_string()
        );
        assert_eq!(info.get("opencode_session_id").unwrap(), "ses_123");
        assert_eq!(info.get("opencode_project_id").unwrap(), "proj_123");
    }

    #[test]
    fn test_find_session_file_prefers_ccbr_session_file() {
        let tmp = TempDir::new().unwrap();
        let session = tmp
            .path()
            .join("proj")
            .join(".ccbr")
            .join(".opencode-session");
        std::fs::create_dir_all(session.parent().unwrap()).unwrap();
        std::fs::write(&session, "{}").unwrap();
        let other = tmp.path().join("elsewhere");
        std::fs::create_dir(&other).unwrap();

        let _guard = std::env::set_current_dir(&other);
        std::env::set_var("CCB_SESSION_FILE", session.to_string_lossy().to_string());
        let comm = OpenCodeCommunicator::new();
        assert_eq!(comm.find_session_file().unwrap(), session);
        std::env::remove_var("CCB_SESSION_FILE");
    }
}
