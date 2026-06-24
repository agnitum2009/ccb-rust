//! Mirrors Python `lib/ccbrd/services/project_namespace_runtime/reflow.py`.
//!
//! Reflows the workspace window of an existing namespace without killing the
//! tmux server.

use super::backend::{
    create_window, ensure_window, find_window, kill_window, rename_window, select_window,
    session_window_target, window_root_pane, TmuxWindowRecord,
};
use super::ensure::ensure_project_namespace;
use super::ensure_context::{
    load_namespace_context, refresh_session_liveness, NamespaceController,
};
use super::ensure_identity::apply_namespace_identity;
use super::models::{ProjectNamespace, ProjectNamespaceEvent};
use super::records::{build_active_state, namespace_from_state};
use crate::Result;

fn window_target(session_name: &str, window: &TmuxWindowRecord) -> Result<String> {
    session_window_target(
        session_name,
        window
            .window_id
            .as_deref()
            .or(Some(window.window_name.as_str())),
    )
}

/// Reflow the project's workspace window.
///
/// Creates a fresh workspace window, applies namespace identity, kills the old
/// workspace window, renames the new one into place, and persists the updated
/// state. The tmux server is kept alive.
pub fn reflow_project_workspace(
    controller: &mut NamespaceController,
    layout_signature: Option<&str>,
    reason: Option<&str>,
    session_probe_timeout_s: Option<f64>,
) -> Result<ProjectNamespace> {
    std::fs::create_dir_all(&controller.layout.ccbrd_dir)?;

    let context = load_namespace_context(controller, layout_signature, None, reason)?;
    let context = refresh_session_liveness(controller, &context, session_probe_timeout_s)?;

    let current = match &context.current {
        Some(current) if context.session_is_alive => current.clone(),
        _ => {
            return ensure_project_namespace(
                controller,
                layout_signature,
                None,
                false,
                reason,
                session_probe_timeout_s,
                None,
            );
        }
    };

    ensure_window(
        &context.backend,
        &context.desired_session_name,
        &context.desired_control_window_name,
        &controller.layout.project_root,
        false,
        session_probe_timeout_s,
    )?;

    let next_workspace_epoch = current.workspace_epoch.max(1) + 1;
    let desired_workspace_name = context.desired_workspace_window_name.clone();
    let temporary_workspace_name = format!(
        "{}.__reflow__.{}",
        desired_workspace_name, next_workspace_epoch
    );

    let temporary_workspace = create_window(
        &context.backend,
        &context.desired_session_name,
        &temporary_workspace_name,
        &controller.layout.project_root,
        true,
        session_probe_timeout_s,
    )?;

    let root_pane = window_root_pane(
        &context.backend,
        &window_target(&context.desired_session_name, &temporary_workspace)?,
        session_probe_timeout_s,
    )?;

    apply_namespace_identity(
        controller,
        &context.backend,
        &root_pane,
        current.namespace_epoch,
        &context.desired_socket_path,
        &context.desired_session_name,
    );

    let current_workspace_name = current
        .workspace_window_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&desired_workspace_name);

    if let Some(current_workspace) = find_window(
        &context.backend,
        &context.desired_session_name,
        current_workspace_name,
        session_probe_timeout_s,
    )? {
        kill_window(
            &context.backend,
            &window_target(&context.desired_session_name, &current_workspace)?,
            session_probe_timeout_s,
        )?;
    }

    rename_window(
        &context.backend,
        &window_target(&context.desired_session_name, &temporary_workspace)?,
        &desired_workspace_name,
        session_probe_timeout_s,
    )?;

    let control_window = find_window(
        &context.backend,
        &context.desired_session_name,
        &context.desired_control_window_name,
        session_probe_timeout_s,
    )?;
    let workspace_window = find_window(
        &context.backend,
        &context.desired_session_name,
        &desired_workspace_name,
        session_probe_timeout_s,
    )?;

    if let Some(ref workspace) = workspace_window {
        select_window(
            &context.backend,
            &window_target(&context.desired_session_name, workspace)?,
        )?;
    }

    let state = build_active_state(
        controller.project_id.clone(),
        Some(&current),
        current.namespace_epoch,
        context.desired_socket_path.clone(),
        context.desired_session_name.clone(),
        controller.layout_version,
        context
            .desired_layout_signature
            .or_else(|| current.layout_signature.clone()),
        Some(context.desired_control_window_name.clone()),
        control_window
            .as_ref()
            .and_then(|w| w.window_id.clone())
            .or_else(|| current.control_window_id.clone()),
        Some(desired_workspace_name.clone()),
        workspace_window
            .as_ref()
            .and_then(|w| w.window_id.clone())
            .or_else(|| current.workspace_window_id.clone()),
        next_workspace_epoch,
        true,
        current.last_started_at.clone(),
    );

    controller.state_store.save(state.clone());

    let event = ProjectNamespaceEvent {
        event_kind: "workspace_reflowed".to_string(),
        project_id: controller.project_id.clone(),
        occurred_at: (controller.clock)(),
        namespace_epoch: Some(current.namespace_epoch),
        tmux_socket_path: Some(context.desired_socket_path.clone()),
        tmux_session_name: Some(context.desired_session_name.clone()),
        details: {
            let mut details = serde_json::Map::new();
            details.insert(
                "reason".to_string(),
                serde_json::Value::String(
                    reason
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "workspace_reflow".to_string()),
                ),
            );
            details
        },
    };
    controller.event_store.append(event);

    let mut namespace = namespace_from_state(&state);
    namespace.workspace_recreated_this_call = true;
    Ok(namespace)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::services::project_namespace_runtime::backend::{create_session, create_window};
    use crate::services::project_namespace_runtime::ensure_context::{
        Clock, EventStore, LayoutConfig, StateStore,
    };
    use crate::services::project_namespace_runtime::models::ProjectNamespaceState;
    use crate::services::project_namespace_runtime::test_support::FakeTmuxBackend;

    fn test_clock() -> Clock {
        Clock::new(|| "2024-01-01T00:00:00Z".to_string())
    }

    fn test_layout(root: &std::path::Path) -> LayoutConfig {
        LayoutConfig {
            project_root: root.to_string_lossy().to_string(),
            ccbrd_dir: root.join(".ccbr"),
            ccbrd_socket_path: root.join(".ccbr/ccbrd.sock").to_string_lossy().to_string(),
            ccbrd_tmux_socket_path: root.join(".ccbr/tmux.sock").to_string_lossy().to_string(),
            ccbrd_tmux_session_name: "ccbr-test".to_string(),
            ccbrd_tmux_control_window_name: "control".to_string(),
            ccbrd_tmux_workspace_window_name: "workspace".to_string(),
        }
    }

    fn test_state() -> ProjectNamespaceState {
        ProjectNamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 2,
            tmux_socket_path: "/tmp".to_string(),
            tmux_session_name: "ccbr-test".to_string(),
            layout_version: 1,
            layout_signature: Some("sig".to_string()),
            control_window_name: Some("control".to_string()),
            control_window_id: Some("@0".to_string()),
            workspace_window_name: Some("workspace".to_string()),
            workspace_window_id: Some("@1".to_string()),
            workspace_epoch: 3,
            ui_attachable: true,
            last_started_at: Some("2024-01-01".to_string()),
            last_destroyed_at: None,
            last_destroy_reason: None,
        }
    }

    fn test_controller(root: &std::path::Path, fake: &FakeTmuxBackend) -> NamespaceController {
        NamespaceController {
            project_id: "p1".to_string(),
            layout_version: 1,
            layout: test_layout(root),
            backend_factory: fake.backend_factory(),
            state_store: StateStore::default(),
            event_store: EventStore::default(),
            clock: test_clock(),
            last_materialized_agent_panes: HashMap::new(),
            last_topology_active_panes: Vec::new(),
            session_alive_override: None,
        }
    }

    fn setup_alive_namespace(controller: &NamespaceController, fake: &FakeTmuxBackend) {
        let backend = fake
            .backend_factory()
            .build(&controller.layout.ccbrd_tmux_socket_path)
            .unwrap();
        create_session(
            &backend,
            &controller.layout.ccbrd_tmux_session_name,
            &controller.layout.project_root,
            Some(&controller.layout.ccbrd_tmux_control_window_name),
            None,
            None,
        )
        .unwrap();
        create_window(
            &backend,
            &controller.layout.ccbrd_tmux_session_name,
            &controller.layout.ccbrd_tmux_workspace_window_name,
            &controller.layout.project_root,
            false,
            None,
        )
        .unwrap();
    }

    #[test]
    fn test_reflow_falls_back_to_ensure_when_no_state() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake = FakeTmuxBackend::new();
        let mut controller = test_controller(tmp.path(), &fake);

        let ns = reflow_project_workspace(&mut controller, None, None, None).unwrap();

        assert!(ns.created_this_call);
        assert!(!ns.workspace_recreated_this_call);
        assert_eq!(ns.namespace_epoch, 1);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0].event_kind,
            "namespace_created"
        );
    }

    #[test]
    fn test_reflow_falls_back_to_ensure_when_session_dead() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake = FakeTmuxBackend::new();
        let mut controller = test_controller(tmp.path(), &fake);
        controller.state_store.namespace = Some(test_state());
        controller.session_alive_override = Some(false);

        let ns = reflow_project_workspace(&mut controller, None, Some("dead"), None).unwrap();

        assert!(ns.created_this_call);
        assert!(!ns.workspace_recreated_this_call);
        assert_eq!(ns.namespace_epoch, 3);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0].event_kind,
            "namespace_created"
        );
        assert_eq!(
            controller.event_store.events[0]
                .details
                .get("reason")
                .unwrap()
                .as_str()
                .unwrap(),
            "dead"
        );
    }

    #[test]
    fn test_reflow_increments_workspace_epoch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake = FakeTmuxBackend::new();
        let mut controller = test_controller(tmp.path(), &fake);
        controller.state_store.namespace = Some(test_state());
        setup_alive_namespace(&controller, &fake);

        let ns = reflow_project_workspace(&mut controller, None, Some("refresh"), None).unwrap();

        assert!(!ns.created_this_call);
        assert!(ns.workspace_recreated_this_call);
        assert_eq!(ns.workspace_epoch, 4);

        let saved = controller.state_store.namespace.as_ref().unwrap();
        assert_eq!(saved.workspace_epoch, 4);
        assert_eq!(saved.namespace_epoch, 2);
        assert_eq!(saved.control_window_name, Some("control".to_string()));
        assert_eq!(saved.workspace_window_name, Some("workspace".to_string()));

        assert_eq!(controller.event_store.events.len(), 1);
        let event = &controller.event_store.events[0];
        assert_eq!(event.event_kind, "workspace_reflowed");
        assert_eq!(event.namespace_epoch, Some(2));
        assert_eq!(
            event.details.get("reason").unwrap().as_str().unwrap(),
            "refresh"
        );

        // The old workspace window should have been replaced by the renamed temp window.
        fake.with_state(|state| {
            let windows = state.sessions.get("ccbr-test").cloned().unwrap_or_default();
            let names: Vec<_> = windows.iter().map(|w| w.name.clone()).collect();
            assert!(names.contains(&"control".to_string()));
            assert!(names.contains(&"workspace".to_string()));
            assert!(!names.iter().any(|n| n.starts_with("workspace.__reflow__")));
        });
    }

    #[test]
    fn test_reflow_uses_default_reason_when_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake = FakeTmuxBackend::new();
        let mut controller = test_controller(tmp.path(), &fake);
        controller.state_store.namespace = Some(test_state());
        setup_alive_namespace(&controller, &fake);

        reflow_project_workspace(&mut controller, None, Some("   "), None).unwrap();

        let event = &controller.event_store.events[0];
        assert_eq!(
            event.details.get("reason").unwrap().as_str().unwrap(),
            "workspace_reflow"
        );
    }
}
