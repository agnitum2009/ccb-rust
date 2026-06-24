//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/destroy.py`.

use super::backend::{build_backend, kill_server};
use super::ensure_context::NamespaceController;
use super::records::{build_destroy_summary, build_destroyed_event, build_destroyed_state};
use crate::Result;

/// Destroy the project namespace and persist the destroyed state.
///
/// Mirrors Python `destroy_project_namespace(controller, *, reason: str)`.
pub fn destroy_project_namespace(
    controller: &mut NamespaceController,
    reason: &str,
) -> Result<super::models::ProjectNamespaceDestroySummary> {
    let normalized_reason = reason.trim().to_string();
    let normalized_reason = if normalized_reason.is_empty() {
        "destroyed".to_string()
    } else {
        normalized_reason
    };

    std::fs::create_dir_all(&controller.layout.ccbd_dir)?;

    let current = controller.state_store.load()?;
    let occurred_at = (controller.clock)();
    let tmux_socket_path = current
        .as_ref()
        .map(|s| s.tmux_socket_path.clone())
        .unwrap_or_else(|| controller.layout.ccbd_tmux_socket_path.clone());
    let tmux_session_name = current
        .as_ref()
        .map(|s| s.tmux_session_name.clone())
        .unwrap_or_else(|| controller.layout.ccbd_tmux_session_name.clone());

    let backend = build_backend(&controller.backend_factory, &tmux_socket_path)?;
    let destroyed = kill_server(&backend);

    let control_window_name = current
        .as_ref()
        .and_then(|s| s.control_window_name.clone())
        .unwrap_or_else(|| controller.layout.ccbd_tmux_control_window_name.clone());
    let workspace_window_name = current
        .as_ref()
        .and_then(|s| s.workspace_window_name.clone())
        .unwrap_or_else(|| controller.layout.ccbd_tmux_workspace_window_name.clone());

    let next_state = build_destroyed_state(
        current.as_ref(),
        controller.project_id.clone(),
        occurred_at.clone(),
        normalized_reason.clone(),
        tmux_socket_path.clone(),
        tmux_session_name.clone(),
        controller.layout_version,
        Some(control_window_name),
        Some(workspace_window_name),
    );

    controller.state_store.save(next_state.clone());
    controller.event_store.append(build_destroyed_event(
        controller.project_id.clone(),
        occurred_at,
        next_state.namespace_epoch,
        tmux_socket_path.clone(),
        tmux_session_name.clone(),
        destroyed,
        normalized_reason.clone(),
    ));

    Ok(build_destroy_summary(
        controller.project_id.clone(),
        Some(next_state.namespace_epoch),
        tmux_socket_path,
        tmux_session_name,
        destroyed,
        normalized_reason,
    ))
}
