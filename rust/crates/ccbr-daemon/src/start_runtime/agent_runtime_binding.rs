//! Mirrors Python `lib/ccbd/start_runtime/agent_runtime_binding.py`.

#![allow(clippy::too_many_arguments)]

use crate::start_runtime::agent_runtime_models::{
    AgentSpec, Command, Context, EnsureAgentRuntimeFn, LaunchBindingHintFn, Plan,
    RelabelProjectNamespacePaneFn, RuntimeBinding, RuntimeBindingState, SameTmuxSocketPathFn,
};

/// Resolve the runtime binding state for an agent.
///
/// Mirrors Python `resolve_runtime_binding_state`.
pub fn resolve_runtime_binding_state(
    context: &Context,
    command: &Command,
    agent_name: &str,
    spec: &AgentSpec,
    plan: &Plan,
    binding: Option<&RuntimeBinding>,
    raw_binding: Option<&RuntimeBinding>,
    stale_binding: bool,
    assigned_pane_id: Option<&str>,
    style_index: usize,
    project_id: &str,
    tmux_socket_path: Option<&str>,
    namespace_epoch: Option<i64>,
    window_name: Option<&str>,
    ensure_agent_runtime_fn: &dyn EnsureAgentRuntimeFn,
    launch_binding_hint_fn: &dyn LaunchBindingHintFn,
    relabel_project_namespace_pane_fn: &dyn RelabelProjectNamespacePaneFn,
    same_tmux_socket_path_fn: &dyn SameTmuxSocketPathFn,
) -> Result<RuntimeBindingState, String> {
    let (binding, agent_action) = launch_or_reuse_binding(
        context,
        command,
        spec,
        plan,
        binding,
        raw_binding,
        stale_binding,
        assigned_pane_id,
        style_index,
        tmux_socket_path,
        ensure_agent_runtime_fn,
        launch_binding_hint_fn,
    )?;

    let mut actions_taken: Vec<String> = Vec::new();
    actions_taken.extend(relabel_runtime_pane(
        binding.as_ref(),
        agent_name,
        project_id,
        style_index,
        tmux_socket_path,
        namespace_epoch,
        window_name,
        relabel_project_namespace_pane_fn,
    )?);

    let (runtime_ref, session_ref, health, lifecycle_state, agent_action) = runtime_status(
        binding.as_ref(),
        stale_binding,
        agent_name,
        &agent_action,
        &mut actions_taken,
    );

    let (socket_name, runtime_pane_id, project_socket_active_pane_id) = runtime_pane_facts(
        binding.as_ref(),
        runtime_ref.as_deref(),
        tmux_socket_path,
        same_tmux_socket_path_fn,
    );

    Ok(RuntimeBindingState {
        binding,
        agent_action,
        actions_taken,
        runtime_ref,
        session_ref,
        health,
        lifecycle_state,
        socket_name,
        runtime_pane_id,
        project_socket_active_pane_id,
    })
}

fn launch_or_reuse_binding(
    context: &Context,
    command: &Command,
    spec: &AgentSpec,
    plan: &Plan,
    binding: Option<&RuntimeBinding>,
    raw_binding: Option<&RuntimeBinding>,
    stale_binding: bool,
    assigned_pane_id: Option<&str>,
    style_index: usize,
    tmux_socket_path: Option<&str>,
    ensure_agent_runtime_fn: &dyn EnsureAgentRuntimeFn,
    launch_binding_hint_fn: &dyn LaunchBindingHintFn,
) -> Result<(Option<RuntimeBinding>, String), String> {
    if let Some(binding) = binding {
        return Ok((Some(binding.clone()), "attached".to_string()));
    }

    let hint = launch_binding_hint_fn.call(
        binding,
        raw_binding,
        stale_binding,
        assigned_pane_id,
        tmux_socket_path,
        &context.project_id,
    )?;

    let launch = ensure_agent_runtime_fn.call(
        context,
        command,
        spec,
        plan,
        hint.as_ref(),
        assigned_pane_id,
        style_index,
        tmux_socket_path,
    )?;

    let binding = launch.binding;
    let agent_action = if stale_binding && launch.launched {
        "relaunched"
    } else if launch.launched {
        "launched"
    } else {
        "attached"
    };

    Ok((binding, agent_action.to_string()))
}

fn relabel_runtime_pane(
    binding: Option<&RuntimeBinding>,
    agent_name: &str,
    project_id: &str,
    style_index: usize,
    tmux_socket_path: Option<&str>,
    namespace_epoch: Option<i64>,
    window_name: Option<&str>,
    relabel_project_namespace_pane_fn: &dyn RelabelProjectNamespacePaneFn,
) -> Result<Vec<String>, String> {
    let binding = match binding {
        Some(b) => b,
        None => return Ok(Vec::new()),
    };

    let relabeled_pane = relabel_project_namespace_pane_fn.call(
        binding,
        agent_name,
        project_id,
        style_index,
        tmux_socket_path,
        namespace_epoch,
        window_name,
    )?;

    match relabeled_pane {
        Some(pane) => Ok(vec![format!("relabel_runtime_pane:{agent_name}:{pane}")]),
        None => Ok(Vec::new()),
    }
}

fn runtime_status(
    binding: Option<&RuntimeBinding>,
    stale_binding: bool,
    agent_name: &str,
    agent_action: &str,
    actions_taken: &mut Vec<String>,
) -> (Option<String>, Option<String>, String, String, String) {
    if binding.is_none() && stale_binding {
        actions_taken.push(format!("degraded_stale_binding:{agent_name}"));
        return (
            Some(String::new()),
            Some(String::new()),
            "degraded".to_string(),
            "degraded".to_string(),
            "degraded".to_string(),
        );
    }

    let runtime_ref = binding.and_then(|b| b.runtime_ref.clone());
    let session_ref = binding.and_then(|b| b.session_ref.clone());
    actions_taken.extend(runtime_action_markers(agent_name, agent_action));
    (
        runtime_ref,
        session_ref,
        "healthy".to_string(),
        "idle".to_string(),
        agent_action.to_string(),
    )
}

fn runtime_action_markers(agent_name: &str, agent_action: &str) -> Vec<String> {
    let marker = match agent_action {
        "attached" => Some(format!("reuse_binding:{agent_name}")),
        "launched" => Some(format!("launch_runtime:{agent_name}")),
        "relaunched" => Some(format!("relaunch_runtime:{agent_name}")),
        _ => None,
    };
    marker.into_iter().collect()
}

fn runtime_pane_facts(
    binding: Option<&RuntimeBinding>,
    runtime_ref: Option<&str>,
    tmux_socket_path: Option<&str>,
    same_tmux_socket_path_fn: &dyn SameTmuxSocketPathFn,
) -> (Option<String>, Option<String>, Option<String>) {
    let runtime_ref = match runtime_ref {
        Some(r) if r.starts_with("tmux:") => r,
        _ => return (None, None, None),
    };

    let binding = match binding {
        Some(b) => b,
        None => return (None, None, None),
    };

    let runtime_pane_id = Some(runtime_ref["tmux:".len()..].to_string());
    let socket_name = binding
        .tmux_socket_path
        .clone()
        .or_else(|| binding.tmux_socket_name.clone());

    let project_socket_active_pane_id =
        if same_tmux_socket_path_fn.call(binding.tmux_socket_path.as_deref(), tmux_socket_path) {
            runtime_pane_id.clone()
        } else {
            None
        };

    (socket_name, runtime_pane_id, project_socket_active_pane_id)
}

/// Default launch-binding-hint logic.
///
/// Mirrors Python `launch_binding_hint`.
pub fn compute_launch_binding_hint(
    binding: Option<&RuntimeBinding>,
    raw_binding: Option<&RuntimeBinding>,
    stale_binding: bool,
    assigned_pane_id: Option<&str>,
    tmux_socket_path: Option<&str>,
    same_tmux_socket_path_fn: &dyn SameTmuxSocketPathFn,
    project_id: &str,
) -> Option<RuntimeBinding> {
    if binding.is_some() {
        return binding.cloned();
    }
    if !stale_binding {
        return None;
    }
    if let Some(raw) = raw_binding {
        if raw.pane_state.as_deref() == Some("foreign") {
            return None;
        }
        if let Some(raw_project) = raw.ccbr_project_id.as_deref() {
            if raw_project != project_id {
                return None;
            }
        }
        if !same_tmux_socket_path_fn.call(raw.tmux_socket_path.as_deref(), tmux_socket_path) {
            return None;
        }
    }
    if assigned_pane_id.is_some()
        && same_tmux_socket_path_fn.call(
            raw_binding.and_then(|b| b.tmux_socket_path.as_deref()),
            tmux_socket_path,
        )
    {
        return None;
    }
    raw_binding.cloned()
}
