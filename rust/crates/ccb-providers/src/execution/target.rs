use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use ccb_terminal::backend::{TerminalBackend, TmuxBackend};
use serde_json::Value;

/// Abstract target for sending prompts to a runtime pane.
///
/// Mirrors the subset of Python `TerminalBackend` used by pane-backed
/// provider adapters: text submission and pane content capture.
pub trait PromptTarget: Send + Sync {
    /// Send `text` to the runtime target identified by `pane_id`.
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String>;

    /// Capture the last `lines` of content from the runtime target.
    fn get_pane_content(&self, pane_id: &str, lines: usize) -> Result<String, String>;
}

/// Default prompt target backed by a tmux backend.
pub struct TmuxPromptTarget {
    backend: TmuxBackend,
}

impl TmuxPromptTarget {
    /// Build a target from session data using `tmux_socket_name` /
    /// `tmux_socket_path` when present.
    pub fn from_session_data(data: &HashMap<String, Value>) -> Self {
        let socket_name = data
            .get("tmux_socket_name")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        let socket_path = data
            .get("tmux_socket_path")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        Self {
            backend: TmuxBackend::new(socket_name, socket_path),
        }
    }

    /// Build a target from explicit tmux socket configuration.
    pub fn new(socket_name: Option<String>, socket_path: Option<String>) -> Self {
        Self {
            backend: TmuxBackend::new(socket_name, socket_path),
        }
    }
}

impl PromptTarget for TmuxPromptTarget {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
        self.backend
            .send_text(pane_id, text)
            .map_err(|e| format!("send_text_failed:{e:?}"))
    }

    fn get_pane_content(&self, pane_id: &str, lines: usize) -> Result<String, String> {
        let n = lines.max(1);
        let output = self
            .backend
            .tmux_run_capture(&["capture-pane", "-t", pane_id, "-p", "-S", &format!("-{n}")])
            .map_err(|e| format!("capture_pane_failed:{e:?}"))?;
        Ok(ccb_terminal::backend::TmuxBackend::strip_ansi(&output))
    }
}

thread_local! {
    static PROMPT_TARGET_OVERRIDE: RefCell<Option<Arc<dyn PromptTarget>>> = const { RefCell::new(None) };
}

/// Set a thread-local prompt target override for the duration of `f`.
///
/// This is the test seam used by provider adapter unit tests so that
/// adapters can remain zero-sized unit structs while still exercising
/// prompt delivery.
pub fn with_prompt_target_override<F, R>(target: Arc<dyn PromptTarget>, f: F) -> R
where
    F: FnOnce() -> R,
{
    PROMPT_TARGET_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = Some(target);
    });
    let result = f();
    PROMPT_TARGET_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    result
}

/// Return the thread-local override if present, otherwise build the default
/// tmux target from the session data stored in `runtime_state`.
pub fn resolve_prompt_target(
    runtime_state: &HashMap<String, Value>,
) -> Option<Arc<dyn PromptTarget>> {
    if let Some(target) = current_prompt_target_override() {
        return Some(target);
    }
    let backend_type = runtime_str(runtime_state, "backend_type");
    if !backend_type.is_empty() && backend_type != "tmux" {
        return None;
    }
    let socket_name = runtime_state
        .get("tmux_socket_name")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let socket_path = runtime_state
        .get("tmux_socket_path")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    Some(Arc::new(TmuxPromptTarget::new(socket_name, socket_path)))
}

/// Return the thread-local override if present, otherwise build the default
/// tmux target from project session data.
pub fn resolve_prompt_target_for_session(
    session_data: &HashMap<String, Value>,
) -> Option<Arc<dyn PromptTarget>> {
    if let Some(target) = current_prompt_target_override() {
        return Some(target);
    }
    Some(Arc::new(TmuxPromptTarget::from_session_data(session_data)))
}

fn current_prompt_target_override() -> Option<Arc<dyn PromptTarget>> {
    PROMPT_TARGET_OVERRIDE.with(|cell| cell.borrow().clone())
}

fn runtime_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Backend configuration extracted from a project session.
#[derive(Debug, Clone, Default)]
pub struct BackendConfig {
    pub backend_type: String,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
}

/// Extract backend configuration from session data.
pub fn backend_config_from_session_data(data: &HashMap<String, Value>) -> BackendConfig {
    let socket_name = data
        .get("tmux_socket_name")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let socket_path = data
        .get("tmux_socket_path")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    BackendConfig {
        backend_type: "tmux".to_string(),
        tmux_socket_name: socket_name,
        tmux_socket_path: socket_path,
    }
}

/// Store backend configuration in runtime state.
pub fn store_backend_config(state: &mut HashMap<String, Value>, config: &BackendConfig) {
    state.insert(
        "backend_type".to_string(),
        Value::String(config.backend_type.clone()),
    );
    if let Some(name) = &config.tmux_socket_name {
        state.insert("tmux_socket_name".to_string(), Value::String(name.clone()));
    }
    if let Some(path) = &config.tmux_socket_path {
        state.insert("tmux_socket_path".to_string(), Value::String(path.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingTarget {
        sent: Arc<Mutex<Vec<(String, String)>>>,
        content: Arc<Mutex<String>>,
    }

    impl PromptTarget for RecordingTarget {
        fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
            self.sent
                .lock()
                .unwrap()
                .push((pane_id.to_string(), text.to_string()));
            Ok(())
        }

        fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
            Ok(self.content.lock().unwrap().clone())
        }
    }

    #[test]
    fn test_with_prompt_target_override_uses_mock() {
        let concrete = RecordingTarget::default();
        let sent = concrete.sent.clone();
        let target: Arc<dyn PromptTarget> = Arc::new(concrete);
        with_prompt_target_override(target, || {
            let resolved = resolve_prompt_target(&HashMap::new()).unwrap();
            resolved.send_text("%1", "hello").unwrap();
        });
        let guard = sent.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0], ("%1".to_string(), "hello".to_string()));
    }

    #[test]
    fn test_resolve_without_override_returns_tmux_target() {
        let mut state = HashMap::new();
        state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
        let target = resolve_prompt_target(&state).unwrap();
        // A real tmux target cannot send without tmux, but it should exist.
        let _ = target;
    }

    #[test]
    fn test_backend_config_round_trip() {
        let mut data = HashMap::new();
        data.insert(
            "tmux_socket_name".to_string(),
            Value::String("sock".to_string()),
        );
        let config = backend_config_from_session_data(&data);
        assert_eq!(config.backend_type, "tmux");
        assert_eq!(config.tmux_socket_name, Some("sock".to_string()));

        let mut state = HashMap::new();
        store_backend_config(&mut state, &config);
        assert_eq!(state.get("backend_type").unwrap(), "tmux");
        assert_eq!(state.get("tmux_socket_name").unwrap(), "sock");
    }
}
