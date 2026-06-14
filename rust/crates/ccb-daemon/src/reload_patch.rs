//! Mirrors Python `lib/ccbd/reload_patch.py`.
//!
//! Namespace patch plan computation: how the tmux namespace should be patched
//! during a reload (additive/remove/refresh steps + blocked operations).

use std::collections::HashSet;

use ccb_agents::models::ProjectConfig;

use crate::reload_additive_agents::{
    additive_agent_steps, additive_window_steps, build_namespace_topology,
    missing_additive_agent_steps,
};
use crate::reload_patch_remove_agents::{
    missing_remove_agent_steps, missing_tool_window_steps, remove_agent_steps,
    remove_tool_window_steps,
};
use crate::reload_plan::{
    clean_text, warnings_for_status, NamespacePatchPlan, NamespacePatchScope, NamespacePatchStep,
    ReloadOperation,
};
use crate::services::project_namespace::ProjectNamespace;

pub(crate) fn build_namespace_patch_plan(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
    operations: &[ReloadOperation],
    project_id: Option<&str>,
    current_namespace: Option<&ProjectNamespace>,
) -> NamespacePatchPlan {
    let op_records: Vec<serde_json::Value> = operations.iter().map(|o| o.to_record()).collect();
    let mut blocked = blocked_unsupported_operations(&op_records);

    let old_topology = build_namespace_topology(current_config);
    let new_topology = build_namespace_topology(new_config);
    let scope = build_scope_payload(project_id, current_namespace);

    let mut steps: Vec<NamespacePatchStep> = Vec::new();

    let has_mutating = operations.iter().any(|o| is_mutating_op(&o.op));
    if has_mutating && !scope.verified {
        blocked.push(serde_json::json!({
            "op": "namespace_scope",
            "reason": "current project namespace scope is unavailable or mismatched",
        }));
    }

    if blocked.is_empty() {
        steps.extend(view_refresh_steps(operations));
        steps.extend(additive_window_steps(&old_topology, &new_topology));
        steps.extend(additive_agent_steps(
            operations,
            &old_topology,
            &new_topology,
        ));
        steps.extend(remove_agent_steps(operations, &old_topology, &new_topology));
        steps.extend(remove_tool_window_steps(
            operations,
            &old_topology,
            &new_topology,
        ));
        blocked.extend(missing_additive_agent_steps(operations, &steps));
        blocked.extend(missing_remove_agent_steps(operations, &steps));
        blocked.extend(missing_tool_window_steps(operations, &steps));
    }

    let status = if !blocked.is_empty() {
        "blocked"
    } else if steps.is_empty() {
        "no_op"
    } else {
        "planned"
    };

    NamespacePatchPlan {
        status: status.into(),
        mutation_enabled: false,
        apply_deferred: true,
        scope,
        steps,
        blocked_operations: blocked,
        warnings: warnings_for_status(status),
    }
}

pub(crate) fn is_mutating_op(op: &str) -> bool {
    matches!(
        op,
        "add_agent" | "add_window" | "remove_agent" | "add_tool_window" | "remove_tool_window"
    )
}

pub(crate) fn blocked_unsupported_operations(
    operations: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let supported: HashSet<&str> = [
        "no_change",
        "view_only_change",
        "maintenance_change",
        "add_agent",
        "add_window",
        "remove_agent",
        "add_tool_window",
        "remove_tool_window",
    ]
    .iter()
    .copied()
    .collect();
    let mut blocked: Vec<serde_json::Value> = Vec::new();
    for op in operations {
        let name = op.get("op").and_then(|v| v.as_str()).unwrap_or("no_change");
        if name == "layout_change"
            && op.get("change").and_then(|v| v.as_str()) == Some("remove_window")
        {
            continue;
        }
        if !supported.contains(name) {
            blocked.push(serde_json::json!({
                "op": name,
                "agent": op.get("agent"),
                "window": op.get("window"),
                "reason": "namespace patch planner supports config-only, additive, and idle remove_agent operations",
            }));
        }
    }
    blocked
}

pub(crate) fn build_scope_payload(
    project_id: Option<&str>,
    current_namespace: Option<&ProjectNamespace>,
) -> NamespacePatchScope {
    let expected_project_id = clean_text(project_id);
    let Some(ns) = current_namespace else {
        return NamespacePatchScope {
            verified: false,
            project_id: expected_project_id,
            tmux_socket_path: None,
            tmux_session_name: None,
            namespace_epoch: None,
            reason: Some("namespace unavailable".into()),
        };
    };

    let namespace_project_id = clean_text(Some(&ns.project_id));
    let socket_path = clean_text(Some(&ns.tmux_socket_path));
    let session_name = clean_text(Some(&ns.tmux_session_name));
    let has_namespace_epoch = ns.namespace_epoch > 0;

    let verified = namespace_project_id.is_some()
        && socket_path.is_some()
        && session_name.is_some()
        && has_namespace_epoch
        && (expected_project_id.is_none() || expected_project_id == namespace_project_id);

    let mut scope = NamespacePatchScope {
        verified,
        project_id: namespace_project_id.or_else(|| expected_project_id.clone()),
        tmux_socket_path: socket_path,
        tmux_session_name: session_name,
        namespace_epoch: Some(ns.namespace_epoch),
        reason: None,
    };
    if !verified {
        scope.reason =
            Some("namespace project/socket/session scope is incomplete or mismatched".into());
    }
    scope
}

pub(crate) fn view_refresh_steps(operations: &[ReloadOperation]) -> Vec<NamespacePatchStep> {
    let refresh_ops: HashSet<&str> = operations
        .iter()
        .filter(|o| matches!(o.op.as_str(), "view_only_change" | "maintenance_change"))
        .map(|o| o.op.as_str())
        .collect();
    if refresh_ops.is_empty() {
        return Vec::new();
    }
    let reason = if refresh_ops == ["view_only_change"].iter().copied().collect() {
        "presentation-only config changed; no tmux namespace mutation is required"
    } else if refresh_ops == ["maintenance_change"].iter().copied().collect() {
        "maintenance heartbeat policy changed; no tmux namespace mutation is required"
    } else {
        "presentation/config-only fields changed; no tmux namespace mutation is required"
    };
    vec![NamespacePatchStep {
        action: "refresh_project_view".into(),
        window: None,
        agent: None,
        role: None,
        slot_key: None,
        reason: Some(reason.into()),
    }]
}
