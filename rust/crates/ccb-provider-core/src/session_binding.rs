use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

/// A concrete provider session value.
///
/// Mirrors Python's dynamic session objects. Callers populate the fields they
/// have; `data` supplies fallback values for field extractors.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub runtime_dir: Option<PathBuf>,
    pub session_file: Option<PathBuf>,
    pub ccb_session_id: Option<String>,
    pub data: HashMap<String, Value>,
    pub backend: Option<Arc<dyn SessionBackend>>,
    pub user_option_lookup: Option<HashMap<String, String>>,
    pub slot_user_option_lookup: Option<HashMap<String, String>>,
}

/// Backend capable of inspecting/manipulating a session.
pub trait SessionBackend: std::fmt::Debug + Send + Sync {
    fn socket_name(&self) -> Option<String>;
    fn socket_path(&self) -> Option<String>;
    fn is_alive(&self, pane_id: &str) -> bool;
    fn is_tmux_pane_alive(&self, pane_id: &str) -> bool;
    fn pane_exists(&self, pane_id: &str) -> bool;
    fn describe_pane(
        &self,
        pane_id: &str,
        user_options: &[String],
    ) -> Option<HashMap<String, String>>;
    fn list_panes_by_user_options(&self, options: &HashMap<String, String>) -> Option<Vec<String>>;
    fn set_pane_title(&self, pane_id: &str, title: &str) -> Result<(), String>;
    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) -> Result<(), String>;
}

/// Runtime identity of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeIdentity {
    pub state: String,
    pub reason: Option<String>,
}

/// Binding information resolved for an agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentBinding {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub provider: Option<String>,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<u32>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub ccb_session_id: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub provider_identity_state: Option<String>,
    pub provider_identity_reason: Option<String>,
}

/// Classify a binding given the presence of its references.
pub fn binding_status(
    runtime_ref: Option<&str>,
    session_ref: Option<&str>,
    workspace_path: Option<&str>,
) -> &'static str {
    if runtime_ref.is_some() && session_ref.is_some() && workspace_path.is_some() {
        "bound"
    } else if runtime_ref.is_some() || session_ref.is_some() || workspace_path.is_some() {
        "partial"
    } else {
        "unbound"
    }
}

/// Resolve a runtime reference from a session.
pub fn session_runtime_ref(session: &Session, pane_id_override: Option<&str>) -> Option<String> {
    let pane_id = pane_id_override
        .map(|s| s.trim())
        .or_else(|| session.pane_id.as_deref().map(|s| s.trim()))
        .filter(|s| !s.is_empty())?;
    let terminal = session
        .terminal
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("tmux");
    Some(format!("{}:{}", terminal, pane_id))
}

/// Resolve a session reference from a session.
pub fn session_ref(
    session: &Session,
    session_id_attr: &str,
    session_path_attr: &str,
) -> Option<String> {
    if let Some(token) = session.data.get(session_id_attr).and_then(Value::as_str) {
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    if let Some(path) = session.data.get(session_path_attr).and_then(Value::as_str) {
        let path = path.trim();
        if !path.is_empty() {
            return Some(expand_home(path));
        }
    }
    session_file(session)
}

/// Extract the tmux socket name from a session.
pub fn session_tmux_socket_name(session: &Session) -> Option<String> {
    if !session_uses_tmux(session) {
        return None;
    }
    session_data_text(session, "tmux_socket_name")
        .or_else(|| session.backend.as_ref().and_then(|b| b.socket_name()))
}

/// Extract the tmux socket path from a session.
pub fn session_tmux_socket_path(session: &Session) -> Option<String> {
    if !session_uses_tmux(session) {
        return None;
    }
    session_data_text(session, "tmux_socket_path")
        .map(|s| expand_home(&s))
        .or_else(|| session.backend.as_ref().and_then(|b| b.socket_path()))
}

/// Extract the provider session id from a session.
pub fn session_id(session: &Session, session_id_attr: &str) -> Option<String> {
    session
        .data
        .get(session_id_attr)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the CCB session id from a session.
pub fn session_ccb_session_id(session: &Session) -> Option<String> {
    session
        .ccb_session_id
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| session_data_text(session, "ccb_session_id"))
}

/// Extract the bound session file path from a session.
pub fn session_file(session: &Session) -> Option<String> {
    session
        .session_file
        .as_ref()
        .map(|p| expand_home(p.to_string_lossy().as_ref()))
}

/// Extract the runtime root directory from a session.
pub fn session_runtime_root(session: &Session) -> Option<String> {
    session
        .runtime_dir
        .as_ref()
        .map(|p| expand_home(p.to_string_lossy().as_ref()))
        .or_else(|| session_data_text(session, "runtime_dir").map(|s| expand_home(&s)))
}

/// Extract the runtime PID from a session.
pub fn session_runtime_pid(session: &Session, provider: &str) -> Option<u32> {
    if let Some(pid) = session_data_pid(session) {
        return Some(pid);
    }
    let runtime_root = session_runtime_root(session)?;
    let runtime_root = PathBuf::from(runtime_root);
    for candidate in pid_file_candidates(&runtime_root, provider) {
        if let Some(pid) = read_pid_file(&candidate) {
            return Some(pid);
        }
    }
    None
}

/// Extract the terminal name from a session.
pub fn session_terminal(session: &Session) -> Option<String> {
    session
        .terminal
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Extract the pane title marker from a session.
pub fn session_pane_title_marker(session: &Session) -> Option<String> {
    session
        .pane_title_marker
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| session_data_text(session, "pane_title_marker"))
}

fn session_uses_tmux(session: &Session) -> bool {
    session
        .terminal
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "tmux".to_string())
        == "tmux"
}

fn session_data_text(session: &Session, key: &str) -> Option<String> {
    session
        .data
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn session_data_pid(session: &Session) -> Option<u32> {
    for key in ["runtime_pid", "pid"] {
        if let Some(value) = session.data.get(key) {
            if let Some(pid) = coerce_pid(value) {
                return Some(pid);
            }
        }
    }
    None
}

fn coerce_pid(value: &Value) -> Option<u32> {
    let text = match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().filter(|&p| p > 0)
}

fn pid_file_candidates(runtime_root: &Path, provider: &str) -> Vec<PathBuf> {
    let provider_name = provider.trim().to_lowercase();
    let preferred = runtime_root.join(format!("{}.pid", provider_name));
    let mut candidates = vec![preferred];
    if let Ok(entries) = std::fs::read_dir(runtime_root) {
        let mut extras: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("pid"))
            .collect();
        extras.sort();
        candidates.extend(extras);
    }
    candidates
}

fn read_pid_file(path: &Path) -> Option<u32> {
    if !path.is_file() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => coerce_pid(&Value::String(text)),
        Err(_) => None,
    }
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestBackend {
        socket_name: String,
        socket_path: String,
    }

    impl SessionBackend for TestBackend {
        fn socket_name(&self) -> Option<String> {
            Some(self.socket_name.clone())
        }
        fn socket_path(&self) -> Option<String> {
            Some(self.socket_path.clone())
        }
        fn is_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn is_tmux_pane_alive(&self, _pane_id: &str) -> bool {
            true
        }
        fn pane_exists(&self, _pane_id: &str) -> bool {
            true
        }
        fn describe_pane(
            &self,
            _pane_id: &str,
            _user_options: &[String],
        ) -> Option<HashMap<String, String>> {
            None
        }
        fn list_panes_by_user_options(
            &self,
            _options: &HashMap<String, String>,
        ) -> Option<Vec<String>> {
            None
        }
        fn set_pane_title(&self, _pane_id: &str, _title: &str) -> Result<(), String> {
            Ok(())
        }
        fn set_pane_user_option(
            &self,
            _pane_id: &str,
            _name: &str,
            _value: &str,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_binding_status() {
        assert_eq!(binding_status(Some("r"), Some("s"), Some("w")), "bound");
        assert_eq!(binding_status(Some("r"), None, None), "partial");
        assert_eq!(binding_status(None, None, None), "unbound");
    }

    #[test]
    fn test_session_tmux_socket_fields_prefer_session_data() {
        let tmp = std::env::temp_dir();
        let mut session = Session {
            terminal: Some("tmux".to_string()),
            ..Default::default()
        };
        session.data.insert(
            "tmux_socket_name".to_string(),
            Value::String("proj-sock".to_string()),
        );
        session.data.insert(
            "tmux_socket_path".to_string(),
            Value::String(tmp.join("tmux.sock").to_string_lossy().to_string()),
        );
        session.backend = Some(Arc::new(TestBackend {
            socket_name: "backend-sock".to_string(),
            socket_path: "/tmp/backend.sock".to_string(),
        }));

        assert_eq!(
            session_tmux_socket_name(&session),
            Some("proj-sock".to_string())
        );
        assert_eq!(
            session_tmux_socket_path(&session),
            Some(tmp.join("tmux.sock").to_string_lossy().to_string())
        );
    }

    #[test]
    fn test_session_ccb_session_id_prefers_attribute_then_data() {
        let mut session = Session {
            ccb_session_id: Some("direct-session".to_string()),
            ..Default::default()
        };
        assert_eq!(
            session_ccb_session_id(&session),
            Some("direct-session".to_string())
        );

        session.ccb_session_id = None;
        session.data.insert(
            "ccb_session_id".to_string(),
            Value::String("payload-session".to_string()),
        );
        assert_eq!(
            session_ccb_session_id(&session),
            Some("payload-session".to_string())
        );
    }

    #[test]
    fn test_session_runtime_pid_prefers_data_then_provider_pid_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let runtime_dir = tmp.path().join("runtime");
        std::fs::create_dir(&runtime_dir).unwrap();
        std::fs::write(runtime_dir.join("codex.pid"), "123\n").unwrap();
        std::fs::write(runtime_dir.join("other.pid"), "456\n").unwrap();

        let session = Session {
            runtime_dir: Some(runtime_dir.clone()),
            ..Default::default()
        };
        assert_eq!(session_runtime_pid(&session, "codex"), Some(123));

        let mut session_with_data = Session {
            runtime_dir: Some(runtime_dir),
            ..Default::default()
        };
        session_with_data
            .data
            .insert("runtime_pid".to_string(), Value::String("789".to_string()));
        assert_eq!(session_runtime_pid(&session_with_data, "codex"), Some(789));
    }
}
