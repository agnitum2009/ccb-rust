//! Mirrors Python `lib/provider_backends/pane_log_support/communicator_state.py`.

use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

/// Communicator state populated by `initialize_state`.
///
/// Mirrors the attributes set on the Python communicator object.
#[derive(Debug, Default)]
pub struct CommunicatorState {
    pub session_info: HashMap<String, Value>,
    pub ccb_session_id: String,
    pub terminal: String,
    pub pane_id: String,
    pub pane_title_marker: String,
    pub backend: Option<String>,
    pub timeout: u64,
    pub project_session_file: Option<String>,
    pub log_reader: Option<Box<dyn LogReader>>,
    pub log_reader_primed: bool,
}

/// Trait for a pane-log reader so that `ensure_log_reader` can remain generic.
pub trait LogReader: std::fmt::Debug + Any {
    fn new(work_dir: Option<PathBuf>, pane_log_path: Option<PathBuf>) -> Self
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
}

/// Initialize communicator state from session info.
///
/// Mirrors Python `initialize_state`.
pub fn initialize_state<F, G>(
    state: &mut CommunicatorState,
    sync_timeout_env: &str,
    missing_session_message: &str,
    load_session_info: F,
    get_pane_id: G,
    backend: Option<String>,
) where
    F: FnOnce() -> Option<HashMap<String, Value>>,
    G: FnOnce(&HashMap<String, Value>) -> Option<String>,
{
    let session_info = load_session_info().expect(missing_session_message);
    state.ccb_session_id = session_info
        .get("ccb_session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    state.terminal = session_info
        .get("terminal")
        .and_then(Value::as_str)
        .unwrap_or("tmux")
        .to_string();
    state.pane_id = get_pane_id(&session_info).unwrap_or_default();
    state.pane_title_marker = session_info
        .get("pane_title_marker")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    state.backend = backend;
    state.timeout = env_timeout(sync_timeout_env);
    state.project_session_file = session_info
        .get("_session_file")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    state.log_reader = None;
    state.log_reader_primed = false;
    state.session_info = session_info;
}

fn env_timeout(sync_timeout_env: &str) -> u64 {
    std::env::var(sync_timeout_env)
        .ok()
        .or_else(|| std::env::var("CCB_SYNC_TIMEOUT").ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600)
}

/// Ensure a log reader is attached to the communicator state.
///
/// Mirrors Python `ensure_log_reader`.
pub fn ensure_log_reader<R>(state: &mut CommunicatorState)
where
    R: LogReader + 'static,
{
    if state.log_reader.is_some() {
        return;
    }
    let work_dir = work_dir_hint(&state.session_info);
    let pane_log_path = pane_log_path(&state.session_info);
    state.log_reader = Some(Box::new(R::new(work_dir, pane_log_path)));
    state.log_reader_primed = true;
}

fn work_dir_hint(session_info: &HashMap<String, Value>) -> Option<PathBuf> {
    session_info
        .get("work_dir")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

fn pane_log_path(session_info: &HashMap<String, Value>) -> Option<PathBuf> {
    if let Some(raw) = session_info
        .get("pane_log_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(expand_home(raw));
    }
    session_info
        .get("runtime_dir")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s).join("pane.log"))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest.trim_start_matches('/'));
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestReader {
        work_dir: Option<PathBuf>,
        pane_log_path: Option<PathBuf>,
    }

    impl LogReader for TestReader {
        fn new(work_dir: Option<PathBuf>, pane_log_path: Option<PathBuf>) -> Self {
            Self {
                work_dir,
                pane_log_path,
            }
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_initialize_state_populates_runtime_fields() {
        let tmp = std::env::temp_dir();
        let session_file = tmp.join(".ccbr").join(".pane-session");
        let session_info: HashMap<String, Value> = [
            (
                "ccb_session_id".to_string(),
                Value::String("ccb-pane-1".to_string()),
            ),
            (
                "_session_file".to_string(),
                Value::String(session_file.to_string_lossy().to_string()),
            ),
            (
                "pane_title_marker".to_string(),
                Value::String("agent5".to_string()),
            ),
        ]
        .into_iter()
        .collect();
        let info_clone = session_info.clone();

        std::env::set_var("PANE_SYNC_TIMEOUT", "33");
        let mut state = CommunicatorState::default();
        initialize_state(
            &mut state,
            "PANE_SYNC_TIMEOUT",
            "missing",
            move || Some(info_clone),
            |_info| Some("%11".to_string()),
            Some("backend:tmux".to_string()),
        );

        assert_eq!(state.ccb_session_id, "ccb-pane-1");
        assert_eq!(state.terminal, "tmux");
        assert_eq!(state.pane_id, "%11");
        assert_eq!(state.backend, Some("backend:tmux".to_string()));
        assert_eq!(state.timeout, 33);
        assert_eq!(
            state.project_session_file,
            Some(session_file.to_string_lossy().to_string())
        );
        assert!(state.log_reader.is_none());
        assert!(!state.log_reader_primed);
    }

    #[test]
    fn test_ensure_log_reader_uses_explicit_or_runtime_log_path() {
        let tmp = std::env::temp_dir();
        let mut session_info: HashMap<String, Value> = HashMap::new();
        session_info.insert(
            "work_dir".to_string(),
            Value::String((tmp.join("workspace")).to_string_lossy().to_string()),
        );
        session_info.insert(
            "pane_log_path".to_string(),
            Value::String(
                (tmp.join("logs").join("pane.log"))
                    .to_string_lossy()
                    .to_string(),
            ),
        );
        session_info.insert(
            "runtime_dir".to_string(),
            Value::String((tmp.join("runtime")).to_string_lossy().to_string()),
        );

        let mut state = CommunicatorState {
            session_info,
            ..Default::default()
        };
        ensure_log_reader::<TestReader>(&mut state);

        let reader = state
            .log_reader
            .as_ref()
            .unwrap()
            .as_any()
            .downcast_ref::<TestReader>()
            .unwrap();
        assert_eq!(reader.work_dir, Some(tmp.join("workspace")));
        assert_eq!(
            reader.pane_log_path,
            Some(tmp.join("logs").join("pane.log"))
        );
        assert!(state.log_reader_primed);
    }
}
