//! End-to-end integration tests for `start_agent_runtime` using the real
//! `EnsureAgentRuntimeImpl` orchestrator with a fake tmux backend.

use std::cell::RefCell;
use std::sync::Arc;

use ccbr_daemon::start_runtime::agent_runtime::{start_agent_runtime, RuntimeService};
use ccbr_daemon::start_runtime::agent_runtime_binding::compute_launch_binding_hint;
use ccbr_daemon::start_runtime::agent_runtime_models::EnsureAgentRuntimeFn;
use ccbr_daemon::start_runtime::agent_runtime_models::{
    AgentSpec, Command, Context, LaunchBindingHintFn, Plan, RelabelProjectNamespacePaneFn,
    RuntimeAttachParams, RuntimeBinding, RuntimeState, SameTmuxSocketPathFn,
};
use ccbr_daemon::start_runtime::test_support::{
    make_ensure_impl, make_ensure_impl_with_min_size, FakeBackend,
};

struct SameSocket;
impl SameTmuxSocketPathFn for SameSocket {
    fn call(&self, a: Option<&str>, b: Option<&str>) -> bool {
        a == b
    }
}

struct DefaultHint {
    same: SameSocket,
}
impl LaunchBindingHintFn for DefaultHint {
    fn call(
        &self,
        binding: Option<&RuntimeBinding>,
        raw_binding: Option<&RuntimeBinding>,
        stale_binding: bool,
        assigned_pane_id: Option<&str>,
        tmux_socket_path: Option<&str>,
        project_id: &str,
    ) -> Result<Option<RuntimeBinding>, String> {
        Ok(compute_launch_binding_hint(
            binding,
            raw_binding,
            stale_binding,
            assigned_pane_id,
            tmux_socket_path,
            &self.same,
            project_id,
        ))
    }
}

struct NoOpRelabel;
impl RelabelProjectNamespacePaneFn for NoOpRelabel {
    fn call(
        &self,
        _binding: &RuntimeBinding,
        _agent_name: &str,
        _project_id: &str,
        _style_index: usize,
        _tmux_socket_path: Option<&str>,
        _namespace_epoch: Option<i64>,
        _window_name: Option<&str>,
    ) -> Result<Option<String>, String> {
        Ok(None)
    }
}

struct RecordingRuntimeService {
    attach_calls: RefCell<Vec<RuntimeAttachParams>>,
    returned_runtime: RuntimeState,
}

impl RecordingRuntimeService {
    fn new(returned_runtime: RuntimeState) -> Self {
        Self {
            attach_calls: RefCell::new(Vec::new()),
            returned_runtime,
        }
    }
}

impl RuntimeService for RecordingRuntimeService {
    fn registry_get(&self, _agent_name: &str) -> Option<RuntimeState> {
        None
    }
    fn attach_mount_attempt_authority(
        &self,
        _attempt_id: &str,
        params: &RuntimeAttachParams,
    ) -> Result<(Option<RuntimeState>, bool), String> {
        self.attach_calls.borrow_mut().push(params.clone());
        Ok((Some(self.returned_runtime.clone()), true))
    }
    fn attach(&self, params: &RuntimeAttachParams) -> Result<RuntimeState, String> {
        self.attach_calls.borrow_mut().push(params.clone());
        Ok(self.returned_runtime.clone())
    }
    fn restore(&self, _agent_name: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn test_start_agent_runtime_launches_with_ensure_impl() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();

    let context = Context {
        project_id: "proj".to_string(),
        project_root: root.clone(),
        workspace_path: root.clone(),
    };
    let command = Command { restore: false };
    let spec = AgentSpec {
        name: "agent1".to_string(),
        runtime_mode: "pane".to_string(),
        provider: "codex".to_string(),
    };
    let plan = Plan {
        workspace_path: root.clone(),
    };

    let backend = Arc::new(FakeBackend::new("%99"));
    let ensure_impl = make_ensure_impl(backend.clone());

    let returned_runtime = RuntimeState {
        runtime_ref: Some("tmux:%99".to_string()),
        session_ref: None,
        lifecycle_state: "idle".to_string(),
        desired_state: "running".to_string(),
        reconcile_state: "idle".to_string(),
        binding_source: "provider-session".to_string(),
        terminal_backend: Some("tmux".to_string()),
        tmux_socket_name: None,
        tmux_socket_path: Some("/tmp/tmux.sock".to_string()),
        tmux_window_name: None,
        tmux_window_id: None,
        pane_id: Some("%99".to_string()),
        active_pane_id: Some("%99".to_string()),
        pane_state: None,
        runtime_pid: None,
        runtime_root: None,
        mount_attempt_id: None,
    };
    let runtime_service = RecordingRuntimeService::new(returned_runtime);

    let result = start_agent_runtime(
        &context,
        &command,
        &runtime_service,
        "agent1",
        &spec,
        &plan,
        None,
        None,
        false,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        &ensure_impl,
        &DefaultHint { same: SameSocket },
        &NoOpRelabel,
        &SameSocket,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(result.agent_result.action, "launched");
    assert_eq!(result.agent_result.runtime_ref.as_deref(), Some("tmux:%99"));
    assert!(result
        .actions_taken
        .contains(&"launch_runtime:agent1".to_string()));
    assert!(backend.has_call("create_pane:"));
    assert!(!runtime_service.attach_calls.borrow().is_empty());
}

#[test]
fn test_pane_too_small_triggers_detached_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let context = Context {
        project_id: "proj".into(),
        project_root: root.clone(),
        workspace_path: root.clone(),
    };

    let backend = Arc::new(
        FakeBackend::new("%1")
            .with_detached_pane("%99")
            .with_pane_size(10, 10),
    );
    backend.mark_alive("%1", true);
    let impl_ = make_ensure_impl_with_min_size(backend.clone(), 80, 24);

    let result = EnsureAgentRuntimeFn::call(
        &impl_,
        &context,
        &Command { restore: false },
        &AgentSpec {
            name: "agent1".into(),
            runtime_mode: "pane".into(),
            provider: "codex".into(),
        },
        &Plan {
            workspace_path: root,
        },
        None,
        None,
        0,
        Some("/tmp/tmux.sock"),
    )
    .unwrap();

    assert!(result.launched);
    assert_eq!(
        result.binding.unwrap().runtime_ref.as_deref(),
        Some("tmux:%99")
    );
    assert!(backend.has_call("tmux_run:kill-pane"));
    assert!(backend.has_call("tmux_run:new-session"));
}

#[test]
fn test_namespace_launch_rejects_detached_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let backend = Arc::new(FakeBackend::new("%1").with_pane_size(10, 10));
    backend.mark_alive("%1", true);
    let impl_ =
        make_ensure_impl_with_min_size(backend.clone(), 80, 24).with_allow_detached_fallback(false);

    let result = EnsureAgentRuntimeFn::call(
        &impl_,
        &Context {
            project_id: "proj".into(),
            project_root: root.clone(),
            workspace_path: root.clone(),
        },
        &Command { restore: false },
        &AgentSpec {
            name: "agent1".into(),
            runtime_mode: "pane".into(),
            provider: "codex".into(),
        },
        &Plan {
            workspace_path: root,
        },
        None,
        None,
        0,
        Some("/tmp/tmux.sock"),
    );

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("could not allocate stable tmux pane"));
}

#[test]
fn test_detached_fallback_when_no_space_for_new_pane() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let backend = Arc::new(
        FakeBackend::new("%99")
            .with_create_pane_error("split-window failed: no space for new pane"),
    );
    let impl_ = make_ensure_impl(backend.clone());

    let result = EnsureAgentRuntimeFn::call(
        &impl_,
        &Context {
            project_id: "proj".into(),
            project_root: root.clone(),
            workspace_path: root.clone(),
        },
        &Command { restore: false },
        &AgentSpec {
            name: "agent1".into(),
            runtime_mode: "pane".into(),
            provider: "codex".into(),
        },
        &Plan {
            workspace_path: root,
        },
        None,
        None,
        0,
        Some("/tmp/tmux.sock"),
    )
    .unwrap();

    assert!(result.launched);
    assert_eq!(
        result.binding.unwrap().runtime_ref.as_deref(),
        Some("tmux:%99")
    );
    assert!(backend.has_call("tmux_run:new-session"));
}

#[test]
fn test_create_pane_no_space_rejects_detached_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let backend = Arc::new(
        FakeBackend::new("%99")
            .with_create_pane_error("split-window failed: no space for new pane"),
    );
    let impl_ = make_ensure_impl(backend.clone()).with_allow_detached_fallback(false);

    let result = EnsureAgentRuntimeFn::call(
        &impl_,
        &Context {
            project_id: "proj".into(),
            project_root: root.clone(),
            workspace_path: root.clone(),
        },
        &Command { restore: false },
        &AgentSpec {
            name: "agent1".into(),
            runtime_mode: "pane".into(),
            provider: "codex".into(),
        },
        &Plan {
            workspace_path: root,
        },
        None,
        None,
        0,
        Some("/tmp/tmux.sock"),
    );

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("could not allocate stable tmux pane"));
}

#[test]
fn test_foreign_binding_is_not_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let backend = Arc::new(FakeBackend::new("%99"));
    backend.mark_alive("%42", true);
    let impl_ = make_ensure_impl(backend.clone());

    let foreign_binding = RuntimeBinding {
        runtime_ref: Some("tmux:%42".into()),
        session_ref: Some("/tmp/proj/.ccbr/.codex-agent1-session".into()),
        tmux_socket_path: Some("/tmp/other-project.sock".into()),
        ccbr_project_id: Some("other-project".into()),
        pane_state: Some("foreign".into()),
        ..Default::default()
    };

    let result = EnsureAgentRuntimeFn::call(
        &impl_,
        &Context {
            project_id: "proj".into(),
            project_root: root.clone(),
            workspace_path: root.clone(),
        },
        &Command { restore: false },
        &AgentSpec {
            name: "agent1".into(),
            runtime_mode: "pane".into(),
            provider: "codex".into(),
        },
        &Plan {
            workspace_path: root,
        },
        Some(&foreign_binding),
        None,
        0,
        Some("/tmp/proj.sock"),
    )
    .unwrap();

    assert!(result.launched);
    assert_eq!(
        result.binding.unwrap().runtime_ref.as_deref(),
        Some("tmux:%99")
    );
    assert!(backend.has_call("tmux_run:kill-pane"));
}
