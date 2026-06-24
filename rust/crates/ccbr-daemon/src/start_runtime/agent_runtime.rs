//! Mirrors Python `lib/ccbd/start_runtime/agent_runtime.py`.

use crate::start_runtime::agent_runtime_binding::resolve_runtime_binding_state;
use crate::start_runtime::agent_runtime_models::{
    AgentRuntimeResult, AgentSpec, Command, Context, EnsureAgentRuntimeFn, LaunchBindingHintFn,
    Plan, RelabelProjectNamespacePaneFn, RuntimeAttachParams, RuntimeBindingState, RuntimeState,
    SameTmuxSocketPathFn, StartAgentExecution,
};
use crate::Result;

/// Start agent runtime with the given configuration.
///
/// Mirrors Python `start_agent_runtime`.
#[allow(clippy::too_many_arguments)]
pub fn start_agent_runtime(
    context: &Context,
    command: &Command,
    runtime_service: &dyn RuntimeService,
    agent_name: &str,
    spec: &AgentSpec,
    plan: &Plan,
    binding: Option<&crate::start_runtime::agent_runtime_models::RuntimeBinding>,
    raw_binding: Option<&crate::start_runtime::agent_runtime_models::RuntimeBinding>,
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
    _workspace_window_id: Option<&str>,
    _workspace_epoch: Option<i64>,
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
    )
    .map_err(crate::DaemonError::Config)?;

    let attach_kwargs = build_attach_kwargs(agent_name, spec, plan, &binding_state, window_name);

    let existing = runtime_service.registry_get(agent_name);
    let attempt_id = existing
        .as_ref()
        .and_then(|e| e.mount_attempt_id.as_ref())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let runtime_opt = if let Some(attempt_id) = attempt_id {
        let is_starting = existing
            .as_ref()
            .map(|e| e.reconcile_state == "starting")
            .unwrap_or(false);

        if is_starting {
            let (rt, applied) = runtime_service
                .attach_mount_attempt_authority(&attempt_id, &attach_kwargs)
                .map_err(crate::DaemonError::Config)?;
            if applied {
                rt
            } else if let Some(rt) = rt {
                Some(rt)
            } else if let Some(rt) = runtime_service.registry_get(agent_name) {
                Some(rt)
            } else {
                Some(
                    runtime_service
                        .attach(&attach_kwargs)
                        .map_err(crate::DaemonError::Config)?,
                )
            }
        } else {
            Some(
                runtime_service
                    .attach(&attach_kwargs)
                    .map_err(crate::DaemonError::Config)?,
            )
        }
    } else {
        Some(
            runtime_service
                .attach(&attach_kwargs)
                .map_err(crate::DaemonError::Config)?,
        )
    };

    let runtime = runtime_opt
        .ok_or_else(|| crate::DaemonError::Config("runtime missing after attach".to_string()))?;

    let mut actions_taken: Vec<String> = binding_state.actions_taken.clone();

    if command.restore && binding_state.agent_action != "degraded" {
        runtime_service
            .restore(agent_name)
            .map_err(crate::DaemonError::Config)?;
        actions_taken.push(format!("restore_runtime:{agent_name}"));
    }

    Ok(StartAgentExecution {
        agent_result: build_agent_result(agent_name, spec, plan, &binding_state, &runtime),
        actions_taken,
        socket_name: binding_state.socket_name,
        runtime_pane_id: binding_state.runtime_pane_id,
        project_socket_active_pane_id: binding_state.project_socket_active_pane_id,
    })
}

fn build_attach_kwargs(
    agent_name: &str,
    spec: &AgentSpec,
    plan: &Plan,
    binding_state: &RuntimeBindingState,
    window_name: Option<&str>,
) -> RuntimeAttachParams {
    let b = binding_state.binding.as_ref();
    RuntimeAttachParams {
        agent_name: agent_name.to_string(),
        workspace_path: plan.workspace_path.clone(),
        backend_type: spec.runtime_mode.clone(),
        runtime_ref: binding_state.runtime_ref.clone(),
        session_ref: binding_state.session_ref.clone(),
        health: binding_state.health.clone(),
        provider: spec.provider.clone(),
        runtime_root: b.and_then(|b| b.runtime_root.clone()),
        runtime_pid: b.and_then(|b| b.runtime_pid.clone()),
        terminal_backend: b.and_then(|b| b.terminal.clone()),
        pane_id: b.and_then(|b| b.pane_id.clone()),
        active_pane_id: b.and_then(|b| b.active_pane_id.clone()),
        pane_title_marker: b.and_then(|b| b.pane_title_marker.clone()),
        pane_state: b.and_then(|b| b.pane_state.clone()),
        tmux_socket_name: b.and_then(|b| b.tmux_socket_name.clone()),
        tmux_socket_path: b.and_then(|b| b.tmux_socket_path.clone()),
        tmux_window_name: window_name
            .map(|s| s.to_string())
            .or_else(|| b.and_then(|b| b.tmux_window_name.clone()))
            .unwrap_or_default(),
        tmux_window_id: b.and_then(|b| b.tmux_window_id.clone()),
        session_file: b.and_then(|b| b.session_file.clone()),
        session_id: b.and_then(|b| b.session_id.clone()),
        slot_key: agent_name.to_string(),
        window_id: None,
        workspace_epoch: None,
        lifecycle_state: binding_state.lifecycle_state.clone(),
        managed_by: "ccbd".to_string(),
        binding_source: "provider-session".to_string(),
    }
}

fn build_agent_result(
    agent_name: &str,
    spec: &AgentSpec,
    plan: &Plan,
    binding_state: &RuntimeBindingState,
    runtime: &RuntimeState,
) -> AgentRuntimeResult {
    AgentRuntimeResult {
        agent_name: agent_name.to_string(),
        provider: spec.provider.clone(),
        action: binding_state.agent_action.clone(),
        health: binding_state.health.clone(),
        workspace_path: plan.workspace_path.clone(),
        runtime_ref: runtime.runtime_ref.clone(),
        session_ref: runtime.session_ref.clone(),
        lifecycle_state: Some(runtime.lifecycle_state.clone()),
        desired_state: Some(runtime.desired_state.clone()),
        reconcile_state: Some(runtime.reconcile_state.clone()),
        binding_source: Some(runtime.binding_source.clone()),
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
    }
}

/// Runtime registry / attach service abstraction for dependency injection.
pub trait RuntimeService {
    fn registry_get(&self, agent_name: &str) -> Option<RuntimeState>;
    fn attach_mount_attempt_authority(
        &self,
        attempt_id: &str,
        params: &RuntimeAttachParams,
    ) -> std::result::Result<(Option<RuntimeState>, bool), String>;
    fn attach(&self, params: &RuntimeAttachParams) -> std::result::Result<RuntimeState, String>;
    fn restore(&self, agent_name: &str) -> std::result::Result<(), String>;
}
