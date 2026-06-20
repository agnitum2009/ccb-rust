//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/ensure_identity.py`.

use ccb_terminal::identity::apply_ccb_pane_identity;

use super::backend::{
    create_session, ensure_server_policy, ensure_window, session_window_target, window_root_pane,
    Backend,
};
use super::ensure_context::{NamespaceController, NamespaceEnsureContext};
use super::materialize_topology::apply_project_tmux_ui;
use crate::Result;

/// Prepare the namespace root pane when no topology plan is supplied.
///
/// Mirrors Python `prepare_namespace_root_pane`.
pub fn prepare_namespace_root_pane(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
    epoch: i64,
    terminal_size: Option<(i32, i32)>,
    timeout_s: Option<f64>,
) -> Result<()> {
    prepare_server(&context.backend, timeout_s)?;
    if !context.session_is_alive {
        create_session(
            &context.backend,
            &context.desired_session_name,
            &controller.layout.project_root,
            Some(&context.desired_control_window_name),
            terminal_size,
            timeout_s,
        )?;
    }
    ensure_server_policy(&context.backend, timeout_s)?;
    ensure_window(
        &context.backend,
        &context.desired_session_name,
        &context.desired_control_window_name,
        &controller.layout.project_root,
        false,
        timeout_s,
    )?;
    ensure_window(
        &context.backend,
        &context.desired_session_name,
        &context.desired_workspace_window_name,
        &controller.layout.project_root,
        true,
        timeout_s,
    )?;
    let root_pane = window_root_pane(
        &context.backend,
        &session_window_target(
            &context.desired_session_name,
            Some(&context.desired_workspace_window_name),
        )?,
        timeout_s,
    )?;
    apply_namespace_identity(
        controller,
        &context.backend,
        &root_pane,
        epoch,
        &context.desired_socket_path,
        &context.desired_session_name,
    );
    Ok(())
}

/// Apply identity and project UI to the namespace root pane.
///
/// Mirrors Python `apply_namespace_identity`.
pub fn apply_namespace_identity(
    controller: &NamespaceController,
    backend: &Backend,
    pane_id: &str,
    namespace_epoch: i64,
    tmux_socket_path: &str,
    tmux_session_name: &str,
) {
    apply_ccb_pane_identity(
        backend,
        pane_id,
        "cmd",
        "cmd",
        &controller.project_id,
        None,
        true,
        Some("cmd"),
        Some("cmd"),
        None,
        None,
        None,
        Some(namespace_epoch),
        Some("ccbd"),
    );
    let _ = apply_project_tmux_ui(
        backend,
        tmux_socket_path,
        Some(&controller.layout.ccbd_socket_path),
        tmux_session_name,
    );
}

// Re-export prepare_server so this module mirrors Python imports.
pub use super::backend::prepare_server;
