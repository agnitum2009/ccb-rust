//! Mirrors Python `lib/provider_backends/opencode/runtime/communicator.py`.

use std::collections::HashMap;
use std::path::PathBuf;

use ccb_provider_core::runtime_specs::provider_marker_prefix;
use serde_json::Value;

use super::super::reader::OpenCodeLogReader;

/// Initialize runtime fields on a communicator object.
///
/// Mirrors Python `initialize_state`.  All external lookups are injected so the
/// function stays testable without real tmux/SQLite dependencies.
pub fn initialize_state<C>(
    comm: &mut C,
    get_backend_for_session_fn: impl FnOnce(&HashMap<String, Value>) -> Option<Value>,
    get_pane_id_from_session_fn: impl FnOnce(&HashMap<String, Value>) -> Option<String>,
    log_reader_factory: impl FnOnce(&HashMap<String, Value>) -> OpenCodeLogReader,
    publish_registry_fn: impl FnOnce(&HashMap<String, Value>),
) where
    C: OpenCodeCommunicatorState,
{
    let session_info = comm.load_session_info().expect(
        "No active OpenCode session found. Add opencode to ccb.config and run `ccb` first",
    );
    comm.set_session_info(session_info.clone());

    comm.set_ccb_session_id(
        session_info
            .get("ccb_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    );
    comm.set_runtime_dir(PathBuf::from(
        session_info
            .get("runtime_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("."),
    ));
    comm.set_terminal(
        session_info
            .get("terminal")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                std::env::var("OPENCODE_TERMINAL").unwrap_or_else(|_| "tmux".to_string())
            }),
    );
    comm.set_pane_id(get_pane_id_from_session_fn(&session_info).unwrap_or_default());
    comm.set_pane_title_marker(
        session_info
            .get("pane_title_marker")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    );
    comm.set_backend(get_backend_for_session_fn(&session_info).unwrap_or(Value::Null));
    comm.set_timeout(
        std::env::var("OPENCODE_SYNC_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30),
    );
    comm.set_marker_prefix(provider_marker_prefix("opencode"));
    comm.set_project_session_file(
        session_info
            .get("_session_file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    );
    comm.set_log_reader(log_reader_factory(&session_info));

    let mut registry_payload = session_info.clone();
    registry_payload.insert(
        "terminal".to_string(),
        Value::String(comm.terminal().to_string()),
    );
    if let Some(pane) = comm.pane_id().filter(|s| !s.is_empty()) {
        registry_payload.insert("pane_id".to_string(), Value::String(pane.to_string()));
    }
    if let Some(path) = comm.project_session_file() {
        registry_payload.insert("project_session_file".to_string(), Value::String(path));
    }
    publish_registry_fn(&registry_payload);
}

/// Trait exposing the mutable fields touched by `initialize_state`.
/// A production communicator struct can implement this directly; tests can use
/// a simple wrapper.
pub trait OpenCodeCommunicatorState {
    fn load_session_info(&self) -> Option<HashMap<String, Value>>;
    fn set_session_info(&mut self, info: HashMap<String, Value>);
    fn set_ccb_session_id(&mut self, id: String);
    fn set_runtime_dir(&mut self, dir: PathBuf);
    fn set_terminal(&mut self, terminal: String);
    fn set_pane_id(&mut self, pane_id: String);
    fn set_pane_title_marker(&mut self, marker: String);
    fn set_backend(&mut self, backend: Value);
    fn set_timeout(&mut self, timeout: i64);
    fn set_marker_prefix(&mut self, prefix: String);
    fn set_project_session_file(&mut self, path: Option<String>);
    fn set_log_reader(&mut self, reader: OpenCodeLogReader);

    fn terminal(&self) -> &str;
    fn pane_id(&self) -> Option<&str>;
    fn project_session_file(&self) -> Option<String>;
}

impl OpenCodeCommunicatorState for super::super::comm::OpenCodeCommunicator {
    fn load_session_info(&self) -> Option<HashMap<String, Value>> {
        self.load_session_info()
    }

    fn set_session_info(&mut self, info: HashMap<String, Value>) {
        self.session_info = info;
    }

    fn set_ccb_session_id(&mut self, id: String) {
        self.ccb_session_id = id;
    }

    fn set_runtime_dir(&mut self, dir: PathBuf) {
        self.runtime_dir = dir;
    }

    fn set_terminal(&mut self, terminal: String) {
        self.terminal = terminal;
    }

    fn set_pane_id(&mut self, pane_id: String) {
        self.pane_id = pane_id;
    }

    fn set_pane_title_marker(&mut self, marker: String) {
        self.pane_title_marker = marker;
    }

    fn set_backend(&mut self, backend: Value) {
        self.backend = backend;
    }

    fn set_timeout(&mut self, timeout: i64) {
        self.timeout = timeout;
    }

    fn set_marker_prefix(&mut self, prefix: String) {
        self.marker_prefix = prefix;
    }

    fn set_project_session_file(&mut self, path: Option<String>) {
        self.project_session_file = path;
    }

    fn set_log_reader(&mut self, reader: OpenCodeLogReader) {
        self.log_reader = Some(reader);
    }

    fn terminal(&self) -> &str {
        &self.terminal
    }

    fn pane_id(&self) -> Option<&str> {
        Some(self.pane_id.as_str()).filter(|s| !s.is_empty())
    }

    fn project_session_file(&self) -> Option<String> {
        self.project_session_file.clone()
    }
}

/// Build a log reader for a session info map.
/// Mirrors Python `_log_reader`.
pub fn build_log_reader_for_session_info(info: &HashMap<String, Value>) -> OpenCodeLogReader {
    let work_dir = info
        .get("work_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let session_id_filter = info
        .get("opencode_session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    OpenCodeLogReader::new(None, &work_dir, "global", session_id_filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_initialize_state_populates_runtime_fields() {
        let tmp = TempDir::new().unwrap();
        let runtime_dir = tmp.path().join("runtime");
        let session_file = tmp.path().join(".ccb").join(".opencode-session");
        std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
        let session_info = serde_json::json!({
            "ccb_session_id": "ccb-open-1",
            "runtime_dir": runtime_dir.to_string_lossy().to_string(),
            "_session_file": session_file.to_string_lossy().to_string(),
            "pane_title_marker": "agent5",
            "opencode_session_id": "ses-1",
            "work_dir": tmp.path().join("workspace").to_string_lossy().to_string(),
        });
        let mut comm =
            super::super::super::comm::OpenCodeCommunicator::with_session_loader(move || {
                Some(
                    session_info
                        .as_object()
                        .unwrap()
                        .clone()
                        .into_iter()
                        .collect(),
                )
            });

        std::env::set_var("OPENCODE_TERMINAL", "tmux");
        std::env::set_var("OPENCODE_SYNC_TIMEOUT", "45");

        let mut published: Vec<HashMap<String, Value>> = Vec::new();
        initialize_state(
            &mut comm,
            |_info| Some(Value::String("backend:tmux".to_string())),
            |_info| Some("%9".to_string()),
            build_log_reader_for_session_info,
            |payload| published.push(payload.clone()),
        );

        assert_eq!(comm.ccb_session_id, "ccb-open-1");
        assert_eq!(comm.runtime_dir, runtime_dir);
        assert_eq!(comm.terminal, "tmux");
        assert_eq!(comm.pane_id, "%9");
        assert_eq!(comm.backend, Value::String("backend:tmux".to_string()));
        assert_eq!(comm.timeout, 45);
        assert_eq!(
            comm.project_session_file,
            Some(session_file.to_string_lossy().to_string())
        );
        assert!(comm.log_reader.is_some());
        let reader = comm.log_reader.as_ref().unwrap();
        assert_eq!(reader.session_id_filter(), Some("ses-1"));
        assert_eq!(published.len(), 1);
        assert_eq!(
            published[0]
                .get("ccb_session_id")
                .unwrap()
                .as_str()
                .unwrap(),
            "ccb-open-1"
        );

        std::env::remove_var("OPENCODE_TERMINAL");
        std::env::remove_var("OPENCODE_SYNC_TIMEOUT");
    }

    #[test]
    fn test_initialize_state_raises_when_session_missing() {
        let mut comm =
            super::super::super::comm::OpenCodeCommunicator::with_session_loader(|| None);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize_state(
                &mut comm,
                |_info| None,
                |_info| None,
                build_log_reader_for_session_info,
                |_payload| {},
            );
        }));
        assert!(result.is_err());
    }
}
