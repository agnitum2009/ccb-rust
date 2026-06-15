//! Mirrors Python `lib/ccbd/start_runtime/agent_runtime.py`.
//! 1:1 file alignment stub.

use crate::Result;
use crate::start_runtime::agent_runtime_models::StartAgentExecution;
use crate::start_runtime::agent_runtime_binding::resolve_runtime_binding_state;
use crate::models::CcbdStartupAgentResult;

/// Start agent runtime with the given configuration
pub fn start_agent_runtime(
    context: &Context,
    command: &Command,
    runtime_service: &dyn RuntimeService,
    agent_name: &str,
    spec: &AgentSpec,
    plan: &Plan,
    binding: &BindingState,
    raw_binding: &RawBinding,
    stale_binding: bool,
    assigned_pane_id: Option<&str>,
    style_index: usize,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    namespace_epoch: Option<i64>,
    ensure_agent_runtime_fn: &dyn EnsureAgentRuntimeFn,
    launch_binding_hint_fn: &dyn LaunchBindingHintFn,
    relabel_project_namespace_pane_fn: &dyn RelabelProjectNamespacePaneFn,
    same_tmux_socket_path_fn: &dyn SameTmuxSocketPathFn,
    workspace_window_id: Option<&str>,
    workspace_epoch: Option<i64>,
    window_name: Option<&str>,
) -> Result<StartAgentExecution> {
    let binding_state = resolve_runtime_binding_state(
        context,
        command,
        agent_name,
        spec,
        plan,
        binding,
        raw_binding,
        stale_binding,
        assigned_pane_id,
        style_index,
        project_id,
        tmux_socket_path,
        namespace_epoch,
        window_name,
        ensure_agent_runtime_fn,
        launch_binding_hint_fn,
        relabel_project_namespace_pane_fn,
        same_tmux_socket_path_fn,
    )?;

    let attach_kwargs = RuntimeAttachParams {
        agent_name: agent_name.to_string(),
        workspace_path: plan.workspace_path.clone(),
        backend_type: spec.runtime_mode.clone(),
        runtime_ref: binding_state.runtime_ref.clone(),
        session_ref: binding_state.session_ref.clone(),
        health: binding_state.health.clone(),
        provider: spec.provider.clone(),
        runtime_root: binding_state.binding.runtime_root.clone(),
        runtime_pid: binding_state.binding.runtime_pid.clone(),
        terminal_backend: binding_state.binding.terminal.clone(),
        pane_id: binding_state.binding.pane_id.clone(),
        active_pane_id: binding_state.binding.active_pane_id.clone(),
        pane_title_marker: binding_state.binding.pane_title_marker.clone(),
        pane_state: binding_state.binding.pane_state.clone(),
        tmux_socket_name: binding_state.binding.tmux_socket_name.clone(),
        tmux_socket_path: binding_state.binding.tmux_socket_path.clone(),
        tmux_window_name: window_name.unwrap_or(
            binding_state.binding.tmux_window_name.as_deref().unwrap_or("")
        ).to_string(),
        tmux_window_id: binding_state.binding.tmux_window_id.clone(),
        session_file: binding_state.binding.session_file.clone(),
        session_id: binding_state.binding.session_id.clone(),
        slot_key: agent_name.to_string(),
        window_id: workspace_window_id.map(|s| s.to_string()),
        workspace_epoch,
        lifecycle_state: binding_state.lifecycle_state.clone(),
        managed_by: "ccbd".to_string(),
        binding_source: "provider-session".to_string(),
    };

    let existing = runtime_service.registry_get(agent_name);
    let attempt_id = existing
        .as_ref()
        .and_then(|e| e.mount_attempt_id.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let runtime = if let Some(attempt_id) = attempt_id {
        let is_starting = existing
            .as_ref()
            .map(|e| e.reconcile_state.as_deref() == Some("starting"))
            .unwrap_or(false);

        if is_starting {
            let (rt, applied) = runtime_service.attach_mount_attempt_authority(
                &attempt_id,
                &attach_kwargs,
            )?;
            if applied {
                rt
            } else {
                rt.or_else(|| runtime_service.registry_get(agent_name)).unwrap_or_else(|| {
                    runtime_service.attach(&attach_kwargs)
                })?
            }
        } else {
            runtime_service.attach(&attach_kwargs)?
        }
    } else {
        runtime_service.attach(&attach_kwargs)?
    };

    let mut actions_taken: Vec<String> = binding_state.actions_taken
        .iter()
        .map(|s| s.to_string())
        .collect();

    if command.restore && binding_state.agent_action != "degraded" {
        runtime_service.restore(agent_name)?;
        actions_taken.push(format!("restore_runtime:{}", agent_name));
    }

    Ok(StartAgentExecution {
        agent_result: CcbdStartupAgentResult {
            agent_name: agent_name.to_string(),
            provider: spec.provider.clone(),
            action: binding_state.agent_action.clone(),
            health: binding_state.health.clone(),
            workspace_path: plan.workspace_path.clone(),
            runtime_ref: runtime.runtime_ref.clone(),
            session_ref: runtime.session_ref.clone(),
            lifecycle_state: runtime.lifecycle_state.clone(),
            desired_state: runtime.desired_state.clone(),
            reconcile_state: runtime.reconcile_state.clone(),
            binding_source: runtime.binding_source.clone(),
            terminal_backend: runtime.terminal_backend.clone(),
            tmux_socket_name: runtime.tmux_socket_name.clone(),
            tmux_socket_path: runtime.tmux_socket_path.clone(),
            tmux_window_name: runtime.tmux_window_name.clone(),
            tmux_window_id: runtime.tmux_window_id.clone(),
            pane_id: runtime.pane_id.clone(),
            active_pane_id: runtime.active_pane_id.clone(),
            pane_state: runtime.pane_state.clone(),
            runtime_pid: runtime.runtime_pid.clone(),
            runtime_root: runtime.runtime_root.clone(),
            failure_reason: if binding_state.agent_action == "degraded" {
                Some("stale_binding_unresolved".to_string())
            } else {
                None
            },
        },
        actions_taken: actions_taken.into_iter().collect(),
        socket_name: binding_state.socket_name.clone(),
        runtime_pane_id: binding_state.runtime_pane_id.clone(),
        project_socket_active_pane_id: binding_state.project_socket_active_pane_id.clone(),
    })
}

// Traits for dependency injection

pub trait RuntimeService {
    fn registry_get(&self, agent_name: &str) -> Option<RuntimeState>;
    fn attach_mount_attempt_authority(&self, attempt_id: &str, params: &RuntimeAttachParams) -> Result<(Option<RuntimeState>, bool)>;
    fn attach(&self, params: &RuntimeAttachParams) -> Result<RuntimeState>;
    fn restore(&self, agent_name: &str) -> Result<()>;
}

pub trait EnsureAgentRuntimeFn {
    fn call(&self, context: &Context, agent_name: &str) -> Result<()>;
}

pub trait LaunchBindingHintFn {
    fn call(&self, agent_name: &str) -> Result<Option<String>>;
}

pub trait RelabelProjectNamespacePaneFn {
    fn call(&self, agent_name: &str, pane_id: &str) -> Result<()>;
}

pub trait SameTmuxSocketPathFn {
    fn call(&self, path1: Option<&str>, path2: Option<&str>) -> bool;
}

// Type definitions

#[derive(Debug, Clone)]
pub struct Context {
    pub workspace_path: String,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub restore: bool,
}

#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub runtime_mode: String,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub workspace_path: String,
}

#[derive(Debug, Clone)]
pub struct BindingState {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub health: String,
    pub lifecycle_state: String,
    pub binding: BindingDetails,
    pub agent_action: String,
    pub actions_taken: Vec<String>,
    pub socket_name: Option<String>,
    pub runtime_pane_id: Option<String>,
    pub project_socket_active_pane_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RawBinding {}

#[derive(Debug, Clone)]
pub struct BindingDetails {
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<String>,
    pub terminal: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeAttachParams {
    pub agent_name: String,
    pub workspace_path: String,
    pub backend_type: String,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub health: String,
    pub provider: String,
    pub runtime_root: Option<String>,
    pub runtime_pid: Option<String>,
    pub terminal_backend: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: String,
    pub tmux_window_id: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub slot_key: String,
    pub window_id: Option<String>,
    pub workspace_epoch: Option<i64>,
    pub lifecycle_state: String,
    pub managed_by: String,
    pub binding_source: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub lifecycle_state: String,
    pub desired_state: String,
    pub reconcile_state: String,
    pub binding_source: String,
    pub terminal_backend: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub tmux_window_name: Option<String>,
    pub tmux_window_id: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_state: Option<String>,
    pub runtime_pid: Option<String>,
    pub runtime_root: Option<String>,
    pub mount_attempt_id: Option<String>,
}
