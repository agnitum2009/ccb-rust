//! Mirrors Python `test/test_ccbrd_tmux_namespace.py`.

use ccbr_daemon::services::health_assessment::tmux_runtime::namespace::{
    pane_outside_project_namespace, NamespaceStateInfo, NamespaceStateStore, PaneRecord,
    RuntimeInfo, TmuxNamespaceBackend,
};

struct FakeStore {
    state: NamespaceStateInfo,
}

impl NamespaceStateStore for FakeStore {
    fn load(&self) -> Option<NamespaceStateInfo> {
        Some(self.state.clone())
    }
}

#[derive(Clone)]
struct FakeRecord {
    window_id: Option<String>,
    window_name: Option<String>,
    ccbr_window: Option<String>,
    expected_match: Option<MatchRequest>,
}

#[derive(Clone)]
struct MatchRequest {
    tmux_session_name: String,
    project_id: String,
    role: String,
    slot_key: Option<String>,
    window_name: Option<String>,
    managed_by: String,
}

impl PaneRecord for FakeRecord {
    fn window_id(&self) -> Option<&str> {
        self.window_id.as_deref()
    }
    fn window_name(&self) -> Option<&str> {
        self.window_name.as_deref()
    }
    fn ccbr_window(&self) -> Option<&str> {
        self.ccbr_window.as_deref()
    }
    fn matches(
        &self,
        tmux_session_name: &str,
        project_id: &str,
        role: &str,
        slot_key: Option<&str>,
        window_name: Option<&str>,
        managed_by: &str,
    ) -> bool {
        let Some(expected) = self.expected_match.as_ref() else {
            return true;
        };
        expected.tmux_session_name == tmux_session_name
            && expected.project_id == project_id
            && expected.role == role
            && expected.slot_key.as_deref() == slot_key
            && expected.window_name.as_deref() == window_name
            && expected.managed_by == managed_by
    }
}

struct FakeBackend {
    socket_matches: bool,
    record: Option<FakeRecord>,
}

impl TmuxNamespaceBackend for FakeBackend {
    fn backend_socket_matches(&self, _tmux_socket_path: Option<&str>) -> bool {
        self.socket_matches
    }
    fn inspect_project_namespace_pane(&self, _pane_id: &str) -> Option<Box<dyn PaneRecord>> {
        self.record
            .clone()
            .map(|r| Box::new(r) as Box<dyn PaneRecord>)
    }
}

fn runtime() -> RuntimeInfo {
    RuntimeInfo {
        project_id: "proj-1".into(),
        agent_name: "agent1".into(),
        slot_key: None,
        tmux_socket_path: Some("/tmp/ccb.sock".into()),
        tmux_window_name: None,
    }
}

fn namespace_state() -> NamespaceStateInfo {
    NamespaceStateInfo {
        tmux_socket_path: Some("/tmp/ccb.sock".into()),
        tmux_session_name: "sess-1".into(),
        workspace_window_id: None,
    }
}

#[test]
fn test_pane_outside_namespace_accepts_runtime_socket_fallback() {
    let backend = FakeBackend {
        socket_matches: false,
        record: None,
    };
    let store = FakeStore {
        state: namespace_state(),
    };
    assert!(pane_outside_project_namespace(
        &runtime(),
        &store,
        Some(&backend),
        "%3"
    ));
}

#[test]
fn test_pane_outside_namespace_checks_project_namespace_record() {
    let backend = FakeBackend {
        socket_matches: true,
        record: Some(FakeRecord {
            window_id: None,
            window_name: None,
            ccbr_window: None,
            expected_match: Some(MatchRequest {
                tmux_session_name: "sess-1".into(),
                project_id: "proj-1".into(),
                role: "agent".into(),
                slot_key: Some("agent1".into()),
                window_name: None,
                managed_by: "ccbrd".into(),
            }),
        }),
    };
    let store = FakeStore {
        state: namespace_state(),
    };
    assert!(!pane_outside_project_namespace(
        &runtime(),
        &store,
        Some(&backend),
        "%3"
    ));
}

#[test]
fn test_pane_outside_namespace_rejects_old_workspace_window() {
    let backend = FakeBackend {
        socket_matches: true,
        record: Some(FakeRecord {
            window_id: Some("@1".into()),
            window_name: None,
            ccbr_window: None,
            expected_match: None,
        }),
    };
    let store = FakeStore {
        state: NamespaceStateInfo {
            workspace_window_id: Some("@2".into()),
            ..namespace_state()
        },
    };
    assert!(pane_outside_project_namespace(
        &runtime(),
        &store,
        Some(&backend),
        "%3"
    ));
}

#[test]
fn test_pane_outside_namespace_accepts_declared_secondary_window() {
    let backend = FakeBackend {
        socket_matches: true,
        record: Some(FakeRecord {
            window_id: Some("@1".into()),
            window_name: Some("review".into()),
            ccbr_window: Some("review".into()),
            expected_match: Some(MatchRequest {
                tmux_session_name: "sess-1".into(),
                project_id: "proj-1".into(),
                role: "agent".into(),
                slot_key: Some("agent2".into()),
                window_name: Some("review".into()),
                managed_by: "ccbrd".into(),
            }),
        }),
    };
    let store = FakeStore {
        state: NamespaceStateInfo {
            workspace_window_id: Some("@0".into()),
            ..namespace_state()
        },
    };
    let runtime = RuntimeInfo {
        agent_name: "agent2".into(),
        slot_key: Some("agent2".into()),
        tmux_window_name: Some("review".into()),
        ..runtime()
    };
    assert!(!pane_outside_project_namespace(
        &runtime,
        &store,
        Some(&backend),
        "%3"
    ));
}

#[test]
fn test_pane_outside_namespace_rejects_mismatched_declared_window() {
    let backend = FakeBackend {
        socket_matches: true,
        record: Some(FakeRecord {
            window_id: Some("@1".into()),
            window_name: Some("review".into()),
            ccbr_window: Some("other".into()),
            expected_match: Some(MatchRequest {
                tmux_session_name: "sess-1".into(),
                project_id: "proj-1".into(),
                role: "agent".into(),
                slot_key: Some("agent2".into()),
                window_name: Some("other".into()),
                managed_by: "ccbrd".into(),
            }),
        }),
    };
    let store = FakeStore {
        state: NamespaceStateInfo {
            workspace_window_id: Some("@0".into()),
            ..namespace_state()
        },
    };
    let runtime = RuntimeInfo {
        agent_name: "agent2".into(),
        slot_key: Some("agent2".into()),
        tmux_window_name: Some("review".into()),
        ..runtime()
    };
    assert!(pane_outside_project_namespace(
        &runtime,
        &store,
        Some(&backend),
        "%3"
    ));
}
