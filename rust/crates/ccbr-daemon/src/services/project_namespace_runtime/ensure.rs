//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/ensure.py`.
//! 1:1 file alignment stub.

use std::collections::HashMap;

use crate::services::project_namespace_runtime::backend::{ensure_server_policy, kill_server};
#[allow(unused_imports)]
use crate::services::project_namespace_runtime::ensure_context::{
    load_namespace_context, rebuild_namespace_backend, refresh_session_liveness, Clock, EventStore,
    LayoutConfig, NamespaceController, NamespaceEnsureContext, NamespaceState, NamespaceWindowPlan,
    StateStore, TopologyPlan,
};
use crate::services::project_namespace_runtime::ensure_identity::prepare_namespace_root_pane;
use crate::services::project_namespace_runtime::materialize_topology::{
    existing_topology_agent_panes, materialize_topology, refresh_topology_ui_for_project,
    topology_active_panes, topology_recreate_reason,
};
use crate::services::project_namespace_runtime::models::ProjectNamespace;
use crate::services::project_namespace_runtime::records::{
    build_active_state, build_created_event, namespace_from_state,
};
use crate::DaemonError;
use crate::Result;

/// Ensure the project namespace exists and matches desired configuration.
///
/// Mirrors Python `ensure_project_namespace`.
pub fn ensure_project_namespace(
    controller: &mut NamespaceController,
    layout_signature: Option<&str>,
    topology_plan: Option<&TopologyPlan>,
    force_recreate: bool,
    recreate_reason: Option<&str>,
    session_probe_timeout_s: Option<f64>,
    terminal_size: Option<(i32, i32)>,
) -> Result<ProjectNamespace> {
    std::fs::create_dir_all(&controller.layout.ccbd_dir)?;

    let mut context =
        load_namespace_context(controller, layout_signature, topology_plan, recreate_reason)?;
    context = refresh_session_liveness(controller, &context, session_probe_timeout_s)?;

    if force_recreate {
        context = force_recreate_namespace(controller, &context)?;
    }
    context = recreate_for_layout_change(controller, &context)?;

    if let Some(plan) = topology_plan {
        if context.session_is_alive && context.current.is_some() {
            if let Some(reason) = topology_recreate_reason(controller, &context, plan) {
                context = force_recreate_namespace(
                    controller,
                    &context.with_updates(None, None, Some(reason)),
                )?;
            }
        }
    }

    if context.session_is_alive && context.current.is_some() {
        if let Some(plan) = topology_plan {
            let agent_panes = existing_topology_agent_panes(controller, &context, plan);
            refresh_topology_ui_for_project(controller, &context, plan, session_probe_timeout_s)?;
            controller.last_materialized_agent_panes = agent_panes;
            controller.last_topology_active_panes =
                topology_active_panes(controller, &context, plan);
        } else {
            controller.last_materialized_agent_panes = HashMap::new();
            controller.last_topology_active_panes = Vec::new();
        }
        return persist_refreshed_namespace(controller, &context, session_probe_timeout_s);
    }

    let epoch = context
        .current
        .as_ref()
        .map(|s| s.namespace_epoch + 1)
        .unwrap_or(1);
    if let Some(plan) = topology_plan {
        let agent_panes = materialize_topology(
            controller,
            &context,
            plan,
            epoch,
            terminal_size,
            session_probe_timeout_s,
        )?;
        controller.last_materialized_agent_panes = agent_panes;
        controller.last_topology_active_panes = topology_active_panes(controller, &context, plan);
    } else {
        prepare_namespace_root_pane(
            controller,
            &context,
            epoch,
            terminal_size,
            session_probe_timeout_s,
        )?;
        controller.last_materialized_agent_panes = HashMap::new();
        controller.last_topology_active_panes = Vec::new();
    }
    build_created_namespace(controller, &context, session_probe_timeout_s)
}

// Placeholder helpers mirroring Python `ensure_state.py` -----------------------------

fn force_recreate_namespace(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
) -> Result<NamespaceEnsureContext> {
    if !context.session_is_alive {
        return Ok(context.clone());
    }
    kill_server(&context.backend);
    let backend = rebuild_namespace_backend(controller, &context.desired_socket_path)?;
    let cause = context
        .recreate_cause
        .clone()
        .unwrap_or_else(|| "forced_recreate".to_string());
    Ok(context.with_updates(Some(backend), Some(false), Some(cause)))
}

fn layout_recreate_reason(
    controller: &NamespaceController,
    current: Option<&NamespaceState>,
    desired_layout_signature: Option<&str>,
) -> Option<String> {
    let current = current?;
    if current.layout_version != controller.layout_version {
        return Some("layout_version_changed".to_string());
    }
    let desired = desired_layout_signature?;
    let current_sig = current
        .layout_signature
        .as_ref()
        .map(|s| s.trim())
        .unwrap_or("");
    if current_sig != desired {
        return Some("layout_signature_changed".to_string());
    }
    None
}

fn recreate_for_layout_change(
    controller: &NamespaceController,
    context: &NamespaceEnsureContext,
) -> Result<NamespaceEnsureContext> {
    if !context.session_is_alive {
        return Ok(context.clone());
    }
    let reason = layout_recreate_reason(
        controller,
        context.current.as_ref(),
        context.desired_layout_signature.as_deref(),
    );
    let Some(reason) = reason else {
        return Ok(context.clone());
    };
    kill_server(&context.backend);
    let backend = rebuild_namespace_backend(controller, &context.desired_socket_path)?;
    Ok(context.with_updates(Some(backend), Some(false), Some(reason)))
}

fn persist_refreshed_namespace(
    controller: &mut NamespaceController,
    context: &NamespaceEnsureContext,
    _timeout_s: Option<f64>,
) -> Result<ProjectNamespace> {
    let current = context.current.as_ref().ok_or_else(|| {
        DaemonError::Config("persist_refreshed_namespace requires current state".to_string())
    })?;

    let control_window_name = current
        .control_window_name
        .clone()
        .unwrap_or_else(|| context.desired_control_window_name.clone());
    let workspace_window_name = if context.topology_plan.is_some() {
        context.desired_workspace_window_name.clone()
    } else {
        current
            .workspace_window_name
            .clone()
            .unwrap_or_else(|| context.desired_workspace_window_name.clone())
    };

    // Apply server policy on every refresh, matching Python ensure_state.py.
    ensure_server_policy(&context.backend, _timeout_s)?;

    // Placeholder: real implementation resolves windows via tmux.
    let state = build_active_state(
        controller.project_id.clone(),
        Some(current),
        current.namespace_epoch,
        context.desired_socket_path.clone(),
        context.desired_session_name.clone(),
        controller.layout_version,
        context
            .desired_layout_signature
            .clone()
            .or_else(|| current.layout_signature.clone()),
        Some(control_window_name),
        current.control_window_id.clone(),
        Some(workspace_window_name),
        current.workspace_window_id.clone(),
        current.workspace_epoch.max(1),
        true,
        current.last_started_at.clone(),
    );
    controller.state_store.save(state.clone());
    Ok(namespace_from_state(&state))
}

fn build_created_namespace(
    controller: &mut NamespaceController,
    context: &NamespaceEnsureContext,
    _timeout_s: Option<f64>,
) -> Result<ProjectNamespace> {
    let current = context.current.as_ref();
    let occurred_at = (controller.clock)();
    let epoch = next_namespace_epoch(current);

    // Placeholder: real implementation resolves windows via tmux.
    let state = build_active_state(
        controller.project_id.clone(),
        current,
        epoch,
        context.desired_socket_path.clone(),
        context.desired_session_name.clone(),
        controller.layout_version,
        context.desired_layout_signature.clone(),
        Some(context.desired_control_window_name.clone()),
        None,
        Some(context.desired_workspace_window_name.clone()),
        None,
        1,
        true,
        Some(occurred_at.clone()),
    );
    controller.state_store.save(state.clone());

    let event = build_created_event(
        controller.project_id.clone(),
        occurred_at,
        epoch,
        context.desired_socket_path.clone(),
        context.desired_session_name.clone(),
        current.is_some(),
        context.recreate_cause.clone().unwrap_or_else(|| {
            if current.is_some() {
                "missing_session".to_string()
            } else {
                "initial_create".to_string()
            }
        }),
    );
    controller.event_store.append(event);

    let mut ns = namespace_from_state(&state);
    ns.created_this_call = true;
    Ok(ns)
}

fn next_namespace_epoch(current: Option<&NamespaceState>) -> i64 {
    current.map(|s| s.namespace_epoch + 1).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::project_namespace_runtime::backend::{build_backend, BackendFactory};
    use std::path::PathBuf;

    fn test_clock() -> Clock {
        Clock::new(|| "2024-01-01T00:00:00Z".to_string())
    }

    fn test_layout() -> LayoutConfig {
        LayoutConfig {
            project_root: "/tmp/ccbr-ensure-test".to_string(),
            ccbd_dir: PathBuf::from("/tmp/ccbr-ensure-test/.ccbr"),
            ccbd_socket_path: "/tmp/ccbr-ensure-test/.ccbr/ccbd.sock".to_string(),
            ccbd_tmux_socket_path: "/tmp/ccbr-ensure-test/.ccbr/tmux.sock".to_string(),
            ccbd_tmux_session_name: "ccbr-ensure-test".to_string(),
            ccbd_tmux_control_window_name: "control".to_string(),
            ccbd_tmux_workspace_window_name: "workspace".to_string(),
        }
    }

    fn test_controller() -> NamespaceController {
        NamespaceController {
            project_id: "p1".to_string(),
            layout_version: 1,
            layout: test_layout(),
            backend_factory:
                crate::services::project_namespace_runtime::test_support::FakeTmuxBackend::new()
                    .backend_factory(),
            state_store: StateStore::default(),
            event_store: EventStore::default(),
            clock: test_clock(),
            last_materialized_agent_panes: HashMap::new(),
            last_topology_active_panes: Vec::new(),
            session_alive_override: None,
        }
    }

    fn test_state(layout_version: i64, layout_signature: Option<&str>) -> NamespaceState {
        NamespaceState {
            project_id: "p1".to_string(),
            namespace_epoch: 2,
            tmux_socket_path: "/tmp/ccbr-ensure-test/.ccbr/tmux.sock".to_string(),
            tmux_session_name: "ccbr-ensure-test".to_string(),
            layout_version,
            layout_signature: layout_signature.map(|s| s.to_string()),
            control_window_name: Some("control".to_string()),
            control_window_id: Some("@0".to_string()),
            workspace_window_name: Some("workspace".to_string()),
            workspace_window_id: Some("@1".to_string()),
            workspace_epoch: 1,
            ui_attachable: true,
            last_started_at: Some("2024-01-01".to_string()),
            last_destroyed_at: None,
            last_destroy_reason: None,
        }
    }

    #[test]
    fn test_next_namespace_epoch_initial() {
        assert_eq!(next_namespace_epoch(None), 1);
    }

    #[test]
    fn test_next_namespace_epoch_increment() {
        let state = test_state(1, None);
        assert_eq!(next_namespace_epoch(Some(&state)), 3);
    }

    #[test]
    fn test_layout_recreate_reason_version_mismatch() {
        let controller = test_controller();
        let state = test_state(2, None);
        assert_eq!(
            layout_recreate_reason(&controller, Some(&state), None),
            Some("layout_version_changed".to_string())
        );
    }

    #[test]
    fn test_layout_recreate_reason_signature_mismatch() {
        let controller = test_controller();
        let state = test_state(1, Some("old"));
        assert_eq!(
            layout_recreate_reason(&controller, Some(&state), Some("new")),
            Some("layout_signature_changed".to_string())
        );
    }

    #[test]
    fn test_layout_recreate_reason_no_change() {
        let controller = test_controller();
        let state = test_state(1, Some("same"));
        assert_eq!(
            layout_recreate_reason(&controller, Some(&state), Some("same")),
            None
        );
    }

    #[test]
    fn test_topology_recreate_reason_workspace_changed() {
        let controller = test_controller();
        let state = test_state(1, None);
        let mut context = NamespaceEnsureContext {
            current: Some(state),
            backend: build_backend(
                &BackendFactory::default(),
                "/tmp/ccbr-ensure-test/.ccbr/tmux.sock",
            )
            .unwrap(),
            session_is_alive: true,
            desired_socket_path: String::new(),
            desired_session_name: String::new(),
            desired_layout_signature: None,
            desired_control_window_name: String::new(),
            desired_workspace_window_name: "new-workspace".to_string(),
            topology_plan: None,
            recreate_cause: None,
        };
        let plan = TopologyPlan {
            signature: None,
            entry_window: "new-workspace".to_string(),
            windows: Vec::new(),
            sidebar_enabled: false,
        };
        assert_eq!(
            topology_recreate_reason(&controller, &context, &plan),
            Some("topology_workspace_changed".to_string())
        );

        context.desired_workspace_window_name = "workspace".to_string();
        assert_eq!(topology_recreate_reason(&controller, &context, &plan), None);
    }

    #[test]
    fn test_ensure_project_namespace_initial_create() {
        let mut controller = test_controller();
        let ns =
            ensure_project_namespace(&mut controller, None, None, false, None, None, None).unwrap();
        assert_eq!(ns.project_id, "p1");
        assert!(ns.created_this_call);
        assert_eq!(ns.namespace_epoch, 1);
        let saved = controller.state_store.namespace.unwrap();
        assert_eq!(saved.namespace_epoch, 1);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0].event_kind,
            "namespace_created"
        );
    }

    #[test]
    fn test_ensure_project_namespace_force_recreate() {
        let mut controller = test_controller();
        controller.state_store.namespace = Some(test_state(1, None));
        controller.session_alive_override = Some(false);
        let ns = ensure_project_namespace(
            &mut controller,
            None,
            None,
            true,
            Some("manual"),
            None,
            None,
        )
        .unwrap();
        assert!(ns.created_this_call);
        assert_eq!(ns.namespace_epoch, 3);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0]
                .details
                .get("reason")
                .unwrap()
                .as_str()
                .unwrap(),
            "manual"
        );
    }

    #[test]
    fn test_ensure_project_namespace_layout_change_triggers_recreate() {
        let mut controller = test_controller();
        controller.state_store.namespace = Some(test_state(1, Some("old")));
        controller.session_alive_override = Some(true);
        let ns =
            ensure_project_namespace(&mut controller, Some("new"), None, false, None, None, None)
                .unwrap();
        assert!(ns.created_this_call);
        assert_eq!(ns.namespace_epoch, 3);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0]
                .details
                .get("reason")
                .unwrap()
                .as_str()
                .unwrap(),
            "layout_signature_changed"
        );
    }

    #[test]
    fn test_ensure_project_namespace_persist_refreshed() {
        let mut controller = test_controller();
        controller.state_store.namespace = Some(test_state(1, Some("same")));
        controller.session_alive_override = Some(true);
        let ns =
            ensure_project_namespace(&mut controller, Some("same"), None, false, None, None, None)
                .unwrap();
        assert!(!ns.created_this_call);
        assert_eq!(ns.namespace_epoch, 2);
        assert_eq!(controller.event_store.events.len(), 0);
        let saved = controller.state_store.namespace.unwrap();
        assert_eq!(saved.layout_signature, Some("same".to_string()));
    }

    #[test]
    fn test_ensure_project_namespace_topology_workspace_change_triggers_recreate() {
        let mut controller = test_controller();
        // Use a matching layout signature so layout change does not mask topology change.
        controller.state_store.namespace = Some(test_state(1, Some("topo-v2")));
        controller.session_alive_override = Some(true);
        let plan = TopologyPlan {
            signature: Some("topo-v2".to_string()),
            entry_window: "new-entry".to_string(),
            windows: vec![NamespaceWindowPlan {
                name: "new-entry".to_string(),
                order: 0,
                kind: "agents".to_string(),
                label: Some("new-entry".to_string()),
                command: None,
                user_layout: "cmd".to_string(),
                agent_names: vec!["claude".to_string()],
                sidebar: None,
            }],
            sidebar_enabled: false,
        };
        let ns =
            ensure_project_namespace(&mut controller, None, Some(&plan), false, None, None, None)
                .unwrap();
        assert!(ns.created_this_call);
        assert_eq!(controller.event_store.events.len(), 1);
        assert_eq!(
            controller.event_store.events[0]
                .details
                .get("reason")
                .unwrap()
                .as_str()
                .unwrap(),
            "topology_workspace_changed"
        );
    }
}
