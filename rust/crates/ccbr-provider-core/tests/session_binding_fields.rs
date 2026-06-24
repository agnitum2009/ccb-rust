use std::collections::HashMap;
use std::sync::Arc;

use ccbr_provider_core::session_binding::{
    session_ccbr_session_id, session_runtime_pid, session_tmux_socket_name,
    session_tmux_socket_path, Session, SessionBackend,
};
use serde_json::Value;

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
fn test_session_tmux_socket_fields_prefer_session_data() {
    let tmp = tempfile::TempDir::new().unwrap();
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
        Value::String(tmp.path().join("tmux.sock").to_string_lossy().to_string()),
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
        Some(tmp.path().join("tmux.sock").to_string_lossy().to_string())
    );
}

#[test]
fn test_session_ccbr_session_id_prefers_attribute_then_data() {
    let mut session = Session {
        ccbr_session_id: Some("direct-session".to_string()),
        ..Default::default()
    };
    assert_eq!(
        session_ccbr_session_id(&session),
        Some("direct-session".to_string())
    );

    session.ccbr_session_id = None;
    session.data.insert(
        "ccbr_session_id".to_string(),
        Value::String("payload-session".to_string()),
    );
    assert_eq!(
        session_ccbr_session_id(&session),
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
