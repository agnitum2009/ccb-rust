//! Mirrors Python `test/test_v2_runtime_launch.py` binding / orchestration assertions.

#![allow(clippy::too_many_arguments)]

use ccbr_daemon::start_runtime::agent_runtime::{start_agent_runtime, RuntimeService};
use ccbr_daemon::start_runtime::agent_runtime_binding::{
    compute_launch_binding_hint, resolve_runtime_binding_state,
};
use ccbr_daemon::start_runtime::agent_runtime_models::{
    AgentSpec, Command, Context, EnsureAgentRuntimeFn, EnsureAgentRuntimeResult,
    LaunchBindingHintFn, Plan, RelabelProjectNamespacePaneFn, RuntimeAttachParams, RuntimeBinding,
    RuntimeState, SameTmuxSocketPathFn,
};
use std::cell::RefCell;

fn ctx() -> Context {
    Context {
        project_id: "proj".to_string(),
        project_root: "/tmp/proj".to_string(),
        workspace_path: "/tmp/ws".to_string(),
    }
}

fn cmd() -> Command {
    Command { restore: false }
}

fn spec() -> AgentSpec {
    AgentSpec {
        name: "codex".to_string(),
        runtime_mode: "pane".to_string(),
        provider: "codex".to_string(),
    }
}

fn plan() -> Plan {
    Plan {
        workspace_path: "/tmp/ws".to_string(),
    }
}

fn sample_binding(runtime_ref: &str) -> RuntimeBinding {
    RuntimeBinding {
        runtime_ref: Some(runtime_ref.to_string()),
        session_ref: Some("session-1".to_string()),
        tmux_socket_path: Some("/tmp/tmux.sock".to_string()),
        pane_id: Some("%42".to_string()),
        active_pane_id: Some("%42".to_string()),
        ..Default::default()
    }
}

struct SameSocket;
impl SameTmuxSocketPathFn for SameSocket {
    fn call(&self, a: Option<&str>, b: Option<&str>) -> bool {
        a == b
    }
}

struct NeverLaunch;
impl EnsureAgentRuntimeFn for NeverLaunch {
    fn call(
        &self,
        _context: &Context,
        _command: &Command,
        _spec: &AgentSpec,
        _plan: &Plan,
        _binding_hint: Option<&RuntimeBinding>,
        _assigned_pane_id: Option<&str>,
        _style_index: usize,
        _tmux_socket_path: Option<&str>,
    ) -> Result<EnsureAgentRuntimeResult, String> {
        Ok(EnsureAgentRuntimeResult {
            launched: false,
            binding: None,
        })
    }
}

struct LaunchWithBinding {
    binding: RuntimeBinding,
    launched: bool,
}
impl EnsureAgentRuntimeFn for LaunchWithBinding {
    fn call(
        &self,
        _context: &Context,
        _command: &Command,
        _spec: &AgentSpec,
        _plan: &Plan,
        binding_hint: Option<&RuntimeBinding>,
        _assigned_pane_id: Option<&str>,
        _style_index: usize,
        _tmux_socket_path: Option<&str>,
    ) -> Result<EnsureAgentRuntimeResult, String> {
        let mut binding = self.binding.clone();
        if let Some(hint) = binding_hint {
            binding.tmux_socket_path = hint.tmux_socket_path.clone();
        }
        Ok(EnsureAgentRuntimeResult {
            launched: self.launched,
            binding: Some(binding),
        })
    }
}

struct RecordingRelabel {
    returned: RefCell<Option<String>>,
}
impl RecordingRelabel {
    fn new(returned: Option<String>) -> Self {
        Self {
            returned: RefCell::new(returned),
        }
    }
}
impl RelabelProjectNamespacePaneFn for RecordingRelabel {
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
        Ok(self.returned.borrow_mut().take())
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

#[test]
fn test_resolve_runtime_binding_state_reuses_existing_binding() {
    let binding = sample_binding("tmux:%42");
    let relabel = RecordingRelabel::new(Some("%42".to_string()));
    let state = resolve_runtime_binding_state(
        &ctx(),
        &cmd(),
        "codex",
        &spec(),
        &plan(),
        Some(&binding),
        None,
        false,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        None,
        &NeverLaunch,
        &DefaultHint { same: SameSocket },
        &relabel,
        &SameSocket,
    )
    .unwrap();

    assert_eq!(state.agent_action, "attached");
    assert!(state
        .actions_taken
        .contains(&"reuse_binding:codex".to_string()));
    assert!(state
        .actions_taken
        .contains(&"relabel_runtime_pane:codex:%42".to_string()));
    assert_eq!(state.health, "healthy");
    assert_eq!(state.lifecycle_state, "idle");
    assert_eq!(state.runtime_ref.as_deref(), Some("tmux:%42"));
    assert_eq!(state.session_ref.as_deref(), Some("session-1"));
    assert_eq!(state.socket_name.as_deref(), Some("/tmp/tmux.sock"));
    assert_eq!(state.runtime_pane_id.as_deref(), Some("%42"));
    assert_eq!(state.project_socket_active_pane_id.as_deref(), Some("%42"));
}

#[test]
fn test_resolve_runtime_binding_state_launches_when_missing() {
    let launched_binding = sample_binding("tmux:%99");
    let relabel = RecordingRelabel::new(Some("%99".to_string()));
    let state = resolve_runtime_binding_state(
        &ctx(),
        &cmd(),
        "codex",
        &spec(),
        &plan(),
        None,
        None,
        false,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        None,
        &LaunchWithBinding {
            binding: launched_binding,
            launched: true,
        },
        &DefaultHint { same: SameSocket },
        &relabel,
        &SameSocket,
    )
    .unwrap();

    assert_eq!(state.agent_action, "launched");
    assert!(state
        .actions_taken
        .contains(&"launch_runtime:codex".to_string()));
    assert!(state
        .actions_taken
        .contains(&"relabel_runtime_pane:codex:%99".to_string()));
    assert_eq!(state.runtime_pane_id.as_deref(), Some("%99"));
}

#[test]
fn test_resolve_runtime_binding_state_relaunches_when_stale() {
    let raw = sample_binding("tmux:%7");
    let launched_binding = sample_binding("tmux:%8");
    let relabel = RecordingRelabel::new(Some("%8".to_string()));
    let state = resolve_runtime_binding_state(
        &ctx(),
        &cmd(),
        "codex",
        &spec(),
        &plan(),
        None,
        Some(&raw),
        true,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        None,
        &LaunchWithBinding {
            binding: launched_binding,
            launched: true,
        },
        &DefaultHint { same: SameSocket },
        &relabel,
        &SameSocket,
    )
    .unwrap();

    assert_eq!(state.agent_action, "relaunched");
    assert!(state
        .actions_taken
        .contains(&"relaunch_runtime:codex".to_string()));
}

#[test]
fn test_resolve_runtime_binding_state_degraded_when_stale_and_unresolved() {
    let state = resolve_runtime_binding_state(
        &ctx(),
        &cmd(),
        "codex",
        &spec(),
        &plan(),
        None,
        Some(&sample_binding("tmux:%7")),
        true,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        None,
        &NeverLaunch,
        &DefaultHint { same: SameSocket },
        &RecordingRelabel::new(None),
        &SameSocket,
    )
    .unwrap();

    assert_eq!(state.agent_action, "degraded");
    assert_eq!(state.health, "degraded");
    assert_eq!(state.lifecycle_state, "degraded");
    assert!(state
        .actions_taken
        .contains(&"degraded_stale_binding:codex".to_string()));
    assert!(state.runtime_ref.as_deref().is_none_or(|s| s.is_empty()));
}

#[test]
fn test_compute_launch_binding_hint_prefers_existing_binding() {
    let b = sample_binding("tmux:%1");
    let hint = compute_launch_binding_hint(
        Some(&b),
        Some(&sample_binding("tmux:%2")),
        true,
        None,
        Some("/tmp/tmux.sock"),
        &SameSocket,
        "proj",
    );
    assert_eq!(hint.as_ref().unwrap().runtime_ref, b.runtime_ref);
}

#[test]
fn test_compute_launch_binding_hint_uses_raw_when_stale_and_no_assigned_pane() {
    let raw = sample_binding("tmux:%2");
    let hint = compute_launch_binding_hint(
        None,
        Some(&raw),
        true,
        None,
        Some("/tmp/tmux.sock"),
        &SameSocket,
        "proj",
    );
    assert_eq!(hint.as_ref().unwrap().runtime_ref, raw.runtime_ref);
}

#[test]
fn test_compute_launch_binding_hint_skips_raw_when_assigned_pane_same_socket() {
    let raw = sample_binding("tmux:%2");
    let hint = compute_launch_binding_hint(
        None,
        Some(&raw),
        true,
        Some("%5"),
        Some("/tmp/tmux.sock"),
        &SameSocket,
        "proj",
    );
    assert!(hint.is_none());
}

struct RecordingRuntimeService {
    attach_calls: RefCell<Vec<RuntimeAttachParams>>,
    restore_calls: RefCell<Vec<String>>,
    registry: RefCell<Option<RuntimeState>>,
    returned_runtime: RuntimeState,
}
impl RecordingRuntimeService {
    fn new(returned_runtime: RuntimeState) -> Self {
        Self {
            attach_calls: RefCell::new(Vec::new()),
            restore_calls: RefCell::new(Vec::new()),
            registry: RefCell::new(None),
            returned_runtime,
        }
    }
}
impl RuntimeService for RecordingRuntimeService {
    fn registry_get(&self, _agent_name: &str) -> Option<RuntimeState> {
        self.registry.borrow().clone()
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
    fn restore(&self, agent_name: &str) -> Result<(), String> {
        self.restore_calls.borrow_mut().push(agent_name.to_string());
        Ok(())
    }
}

#[test]
fn test_start_agent_runtime_attaches_and_returns_result() {
    let binding = sample_binding("tmux:%42");
    let returned_runtime = RuntimeState {
        runtime_ref: Some("tmux:%42".to_string()),
        session_ref: Some("session-1".to_string()),
        lifecycle_state: "idle".to_string(),
        desired_state: "running".to_string(),
        reconcile_state: "idle".to_string(),
        binding_source: "provider-session".to_string(),
        terminal_backend: Some("tmux".to_string()),
        tmux_socket_name: None,
        tmux_socket_path: Some("/tmp/tmux.sock".to_string()),
        tmux_window_name: None,
        tmux_window_id: None,
        pane_id: Some("%42".to_string()),
        active_pane_id: Some("%42".to_string()),
        pane_state: None,
        runtime_pid: None,
        runtime_root: None,
        mount_attempt_id: None,
    };
    let service = RecordingRuntimeService::new(returned_runtime);
    let result = start_agent_runtime(
        &ctx(),
        &cmd(),
        &service,
        "codex",
        &spec(),
        &plan(),
        Some(&binding),
        None,
        false,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        &NeverLaunch,
        &DefaultHint { same: SameSocket },
        &RecordingRelabel::new(None),
        &SameSocket,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(result.agent_result.agent_name, "codex");
    assert_eq!(result.agent_result.action, "attached");
    assert_eq!(result.agent_result.health, "healthy");
    assert_eq!(service.attach_calls.borrow().len(), 1);
}

#[test]
fn test_start_agent_runtime_restore_adds_action() {
    let binding = sample_binding("tmux:%42");
    let returned_runtime = RuntimeState {
        runtime_ref: Some("tmux:%42".to_string()),
        session_ref: Some("session-1".to_string()),
        lifecycle_state: "idle".to_string(),
        desired_state: "running".to_string(),
        reconcile_state: "idle".to_string(),
        binding_source: "provider-session".to_string(),
        terminal_backend: None,
        tmux_socket_name: None,
        tmux_socket_path: None,
        tmux_window_name: None,
        tmux_window_id: None,
        pane_id: Some("%42".to_string()),
        active_pane_id: None,
        pane_state: None,
        runtime_pid: None,
        runtime_root: None,
        mount_attempt_id: None,
    };
    let service = RecordingRuntimeService::new(returned_runtime);
    let mut command = cmd();
    command.restore = true;
    let result = start_agent_runtime(
        &ctx(),
        &command,
        &service,
        "codex",
        &spec(),
        &plan(),
        Some(&binding),
        None,
        false,
        None,
        0,
        "proj",
        Some("/tmp/tmux.sock"),
        Some(1),
        &NeverLaunch,
        &DefaultHint { same: SameSocket },
        &RecordingRelabel::new(None),
        &SameSocket,
        None,
        None,
        None,
    )
    .unwrap();

    assert!(result
        .actions_taken
        .contains(&"restore_runtime:codex".to_string()));
    assert_eq!(service.restore_calls.borrow().len(), 1);
}
