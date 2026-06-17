//! Mirrors Python `lib/ccbd/reload_apply_namespace.py`.

use crate::app::CcbdApp;
use crate::reload_apply_results::not_published_diagnostics;
use crate::services::project_namespace::ProjectNamespaceController;
use crate::services::project_namespace_runtime::models::NamespacePatchApplyResult;
use crate::services::project_namespace_runtime::topology_plan::{
    build_namespace_topology_plan, NamespaceTopologyPlan,
};
use ccb_agents::models::ProjectConfig;
use serde_json::Value;
use std::collections::HashMap;

/// Context required to apply a namespace patch.
#[derive(Debug, Clone)]
pub struct NamespacePatchContext {
    pub plan: HashMap<String, Value>,
    pub old_topology: NamespaceTopologyPlan,
    pub new_topology: NamespaceTopologyPlan,
}

/// Resolve the current namespace, preferring a provided value.
pub fn current_namespace(
    app: &CcbdApp,
    provided_namespace: Option<&crate::services::project_namespace::ProjectNamespace>,
) -> (
    Option<crate::services::project_namespace::ProjectNamespace>,
    HashMap<String, Value>,
) {
    if let Some(ns) = provided_namespace {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("status".to_string(), serde_json::json!("provided"));
        return (Some(ns.clone()), diagnostics);
    }

    match app.project_namespace.load() {
        Some(ns) => {
            let mut diagnostics = HashMap::new();
            diagnostics.insert("status".to_string(), serde_json::json!("loaded"));
            (Some(ns.clone()), diagnostics)
        }
        None => {
            let mut diagnostics = HashMap::new();
            diagnostics.insert("status".to_string(), serde_json::json!("missing"));
            (None, diagnostics)
        }
    }
}

/// Build a topology plan for a config in the context of the current app.
pub fn topology_for(app: &CcbdApp, config: &ProjectConfig) -> NamespaceTopologyPlan {
    build_namespace_topology_plan(
        config,
        app.socket_path(),
        app.project_root.to_string_lossy().to_string(),
    )
}

/// Apply the namespace patch described by the plan.
pub fn apply_namespace_patch(
    app: &mut CcbdApp,
    plan: &HashMap<String, Value>,
    old_topology: &NamespaceTopologyPlan,
    new_topology: &NamespaceTopologyPlan,
    apply_namespace_patch_fn: Option<
        &dyn Fn(
            &HashMap<String, Value>,
            &NamespaceTopologyPlan,
            &NamespaceTopologyPlan,
        ) -> NamespacePatchApplyResult,
    >,
) -> NamespacePatchApplyResult {
    let plan_class = plan
        .get("plan_class")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if matches!(plan_class, "view_only_change" | "maintenance_change") {
        return config_only_namespace_patch_result(plan);
    }

    let patch_plan = plan
        .get("namespace_patch_plan")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<String, Value>>()
        })
        .unwrap_or_default();

    if let Some(patch_fn) = apply_namespace_patch_fn {
        return custom_namespace_patch(&patch_plan, old_topology, new_topology, patch_fn);
    }

    controller_namespace_patch(app, &patch_plan, old_topology, new_topology)
}

fn custom_namespace_patch(
    patch_plan: &HashMap<String, Value>,
    old_topology: &NamespaceTopologyPlan,
    new_topology: &NamespaceTopologyPlan,
    apply_namespace_patch_fn: &dyn Fn(
        &HashMap<String, Value>,
        &NamespaceTopologyPlan,
        &NamespaceTopologyPlan,
    ) -> NamespacePatchApplyResult,
) -> NamespacePatchApplyResult {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        apply_namespace_patch_fn(patch_plan, old_topology, new_topology)
    })) {
        Ok(result) => result,
        Err(err) => {
            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "namespace patch panic".to_string()
            };
            exception_namespace_patch_result(&msg)
        }
    }
}

fn controller_namespace_patch(
    app: &mut CcbdApp,
    patch_plan: &HashMap<String, Value>,
    _old_topology: &NamespaceTopologyPlan,
    new_topology: &NamespaceTopologyPlan,
) -> NamespacePatchApplyResult {
    if let Some(controller) = project_namespace_controller(app) {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            controller_apply_reload_patch(controller, patch_plan, new_topology)
        })) {
            Ok(result) => result,
            Err(err) => {
                let msg = if let Some(s) = err.downcast_ref::<&str>() {
                    (*s).to_string()
                } else {
                    "controller namespace patch panic".to_string()
                };
                exception_namespace_patch_result(&msg)
            }
        }
    } else {
        exception_namespace_patch_result("project namespace controller not available")
    }
}

fn project_namespace_controller(app: &mut CcbdApp) -> Option<&mut ProjectNamespaceController> {
    Some(&mut app.project_namespace)
}

fn controller_apply_reload_patch(
    controller: &mut ProjectNamespaceController,
    patch_plan: &HashMap<String, Value>,
    new_topology: &NamespaceTopologyPlan,
) -> NamespacePatchApplyResult {
    let steps = patch_plan
        .get("steps")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut namespace = match controller.load() {
        Some(ns) => ns.clone(),
        None => {
            return NamespacePatchApplyResult {
                status: "failed".to_string(),
                diagnostics: serde_json::json!({
                    "reason": "namespace_missing",
                    "error": "no namespace loaded",
                    "steps": steps,
                }),
            }
        }
    };

    for step in &steps {
        if let Some(action) = step.get("action").and_then(|v| v.as_str()) {
            match action {
                "create_window" => {
                    if let Some(window_name) = step.get("window").and_then(|v| v.as_str()) {
                        if !namespace.windows.iter().any(|w| w.name == window_name) {
                            namespace.windows.push(
                                crate::services::project_namespace::NamespaceWindow {
                                    name: window_name.to_string(),
                                    window_id: None,
                                    agents: Vec::new(),
                                },
                            );
                        }
                    }
                }
                "create_agent_pane" => {
                    if let (Some(window_name), Some(agent_name)) = (
                        step.get("window").and_then(|v| v.as_str()),
                        step.get("agent").and_then(|v| v.as_str()),
                    ) {
                        if let Some(window) =
                            namespace.windows.iter_mut().find(|w| w.name == window_name)
                        {
                            if !window.agents.contains(&agent_name.to_string()) {
                                window.agents.push(agent_name.to_string());
                            }
                        }
                    }
                }
                "kill_tool_window" => {
                    if let Some(window_name) = step.get("window").and_then(|v| v.as_str()) {
                        namespace.windows.retain(|w| w.name != window_name);
                    }
                }
                _ => {}
            }
        }
    }

    namespace.agent_names = namespace
        .windows
        .iter()
        .flat_map(|w| w.agents.clone())
        .collect();
    namespace.project_root = new_topology.project_root.clone();

    if let Err(e) = controller.mount(namespace) {
        return NamespacePatchApplyResult {
            status: "failed".to_string(),
            diagnostics: serde_json::json!({
                "reason": "namespace_mount_failed",
                "error": e,
                "steps": steps,
            }),
        };
    }

    NamespacePatchApplyResult {
        status: "applied".to_string(),
        diagnostics: serde_json::json!({
            "reason": "controller_apply_reload_patch",
            "namespace_state_written": true,
            "steps": steps,
        }),
    }
}

/// Build a config-only namespace patch result.
pub fn config_only_namespace_patch_result(
    plan: &HashMap<String, Value>,
) -> NamespacePatchApplyResult {
    let steps = plan
        .get("namespace_patch_plan")
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("steps"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let plan_class = plan
        .get("plan_class")
        .and_then(|v| v.as_str())
        .unwrap_or("config_only_change");

    NamespacePatchApplyResult {
        status: "applied".to_string(),
        diagnostics: serde_json::json!({
            "reason": plan_class,
            "supported_operations": ["view_only_change", "maintenance_change"],
            "namespace_state_written": false,
            "graph_published": false,
            "runtime_authority_written": false,
            "lease_or_lifecycle_written": false,
            "steps": steps,
        }),
    }
}

/// Build a failed namespace patch result from an exception message.
pub fn exception_namespace_patch_result(message: &str) -> NamespacePatchApplyResult {
    let mut diagnostics = not_published_diagnostics()
        .into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect::<serde_json::Map<String, Value>>();
    diagnostics.insert(
        "reason".to_string(),
        serde_json::json!("namespace_patch_failed"),
    );
    diagnostics.insert("error_type".to_string(), serde_json::json!("Exception"));
    diagnostics.insert("error".to_string(), serde_json::json!(message));
    diagnostics.insert(
        "runtime_authority_written".to_string(),
        serde_json::json!(false),
    );

    NamespacePatchApplyResult {
        status: "failed".to_string(),
        diagnostics: Value::Object(diagnostics),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn config_only_result_is_applied() {
        let mut plan = HashMap::new();
        plan.insert("plan_class".to_string(), json!("view_only_change"));
        plan.insert(
            "namespace_patch_plan".to_string(),
            json!({"steps": [{"action": "refresh_project_view"}]}),
        );
        let result = config_only_namespace_patch_result(&plan);
        assert_eq!(result.status, "applied");
        assert_eq!(result.diagnostics["reason"], "view_only_change");
        assert_eq!(result.diagnostics["namespace_state_written"], false);
    }

    #[test]
    fn exception_result_is_failed() {
        let result = exception_namespace_patch_result("disk full");
        assert_eq!(result.status, "failed");
        assert_eq!(result.diagnostics["reason"], "namespace_patch_failed");
        assert_eq!(result.diagnostics["error"], "disk full");
        assert_eq!(result.diagnostics["graph_published"], "false");
    }
}
