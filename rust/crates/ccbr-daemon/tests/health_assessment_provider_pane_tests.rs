//! Mirrors Python `test/test_ccbrd_health_assessment_provider_pane.py`.

use ccbr_daemon::services::health_assessment::models::SessionBinding;
use ccbr_daemon::services::health_assessment::provider_pane::{
    assess_provider_pane, AgentSpecResolver, AgentSpecView, ProviderRuntimeInfo,
};
use ccbr_daemon::services::health_assessment::tmux_runtime::namespace::{
    NamespaceStateInfo, NamespaceStateStore,
};
use ccbr_provider_core::session_binding::{Session, SessionBackend};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct FakeBinding {
    provider: String,
    session: Option<Session>,
}

impl SessionBinding for FakeBinding {
    fn provider(&self) -> &str {
        &self.provider
    }
    fn session_id_attr(&self) -> &str {
        "session_id"
    }
    fn session_path_attr(&self) -> &str {
        "session_path"
    }
    fn load_session(&self, _root: &Path, _instance: Option<&str>) -> Option<Session> {
        self.session.clone()
    }
    fn clone_box(&self) -> Box<dyn SessionBinding> {
        Box::new(self.clone())
    }
}

#[derive(Debug)]
struct FakeRegistry {
    provider: String,
}

impl AgentSpecView for FakeRegistry {
    fn provider(&self) -> &str {
        &self.provider
    }
}

impl AgentSpecResolver for FakeRegistry {
    fn spec_for(&self, _agent_name: &str) -> Option<&dyn AgentSpecView> {
        Some(self)
    }
}

#[derive(Debug)]
struct FakeStore {
    state: NamespaceStateInfo,
}

impl NamespaceStateStore for FakeStore {
    fn load(&self) -> Option<NamespaceStateInfo> {
        Some(self.state.clone())
    }
}

#[derive(Debug)]
struct FakeBackend {
    socket_path: String,
    owned: bool,
}

impl SessionBackend for FakeBackend {
    fn socket_name(&self) -> Option<String> {
        None
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
        let mut options = HashMap::new();
        options.insert("@ccb_project_id".to_string(), "proj-1".to_string());
        options.insert("@ccb_role".to_string(), "agent".to_string());
        options.insert("@ccb_slot".to_string(), "agent1".to_string());
        options.insert("@ccb_window".to_string(), "main".to_string());
        options.insert("@ccb_managed_by".to_string(), "ccbrd".to_string());
        if !self.owned {
            // Make the record not match by changing the project id.
            options.insert("@ccb_project_id".to_string(), "other-proj".to_string());
        }
        Some(options)
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

fn runtime() -> ProviderRuntimeInfo {
    ProviderRuntimeInfo {
        agent_name: "agent1".into(),
        runtime_ref: Some("tmux:%1".into()),
        workspace_path: Some("/tmp/workspace".into()),
        project_id: "proj-1".into(),
        slot_key: None,
        tmux_socket_path: Some("/tmp/ccbr.sock".into()),
        tmux_window_name: None,
    }
}

fn namespace_state() -> NamespaceStateInfo {
    NamespaceStateInfo {
        tmux_socket_path: Some("/tmp/ccbr.sock".into()),
        tmux_session_name: "sess-1".into(),
        workspace_window_id: None,
    }
}

#[test]
fn test_assess_provider_pane_reports_missing_session() {
    let binding = FakeBinding {
        provider: "codex".into(),
        session: None,
    };
    let mut bindings: HashMap<String, Box<dyn SessionBinding>> = HashMap::new();
    bindings.insert("codex".into(), Box::new(binding));
    let registry = FakeRegistry {
        provider: "codex".into(),
    };
    let store = FakeStore {
        state: namespace_state(),
    };

    let assessment = assess_provider_pane(&runtime(), &registry, &bindings, &store).unwrap();

    assert!(assessment.binding.is_some());
    assert!(assessment.session.is_none());
    assert_eq!(assessment.pane_state.as_deref(), Some("missing"));
    assert_eq!(assessment.health, "session-missing");
}

#[test]
fn test_assess_provider_pane_marks_foreign_tmux_pane() {
    let backend = FakeBackend {
        socket_path: "/tmp/ccbr.sock".into(),
        owned: false,
    };
    let session = Session {
        terminal: Some("tmux".into()),
        pane_id: Some("%9".into()),
        backend: Some(Arc::new(backend)),
        ..Session::default()
    };
    let binding = FakeBinding {
        provider: "codex".into(),
        session: Some(session),
    };
    let mut bindings: HashMap<String, Box<dyn SessionBinding>> = HashMap::new();
    bindings.insert("codex".into(), Box::new(binding));
    let registry = FakeRegistry {
        provider: "codex".into(),
    };
    let store = FakeStore {
        state: namespace_state(),
    };

    let assessment = assess_provider_pane(&runtime(), &registry, &bindings, &store).unwrap();

    assert!(assessment.session.is_some());
    assert_eq!(assessment.terminal.as_deref(), Some("tmux"));
    assert_eq!(assessment.pane_state.as_deref(), Some("foreign"));
    assert_eq!(assessment.health, "pane-foreign");
}
