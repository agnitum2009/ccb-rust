//! Mirrors Python `lib/ccbd/reload_apply_plan.py`.
//! 1:1 file alignment stub.

use crate::reload_apply_models::AdditiveReloadApplyResult;
use std::collections::{HashMap, HashSet};

const ALLOWED_PLAN_CLASSES: &[&str] = &[
    "no_change",
    "view_only_change",
    "maintenance_change",
    "add_agent",
    "add_window",
    "remove_agent",
    "add_tool_window",
    "remove_tool_window",
];

const ALLOWED_OPERATIONS: &[&str] = &[
    "view_only_change",
    "maintenance_change",
    "add_agent",
    "add_window",
    "remove_agent",
    "add_tool_window",
    "remove_tool_window",
    "layout_change",
];

pub fn plan_blocker(plan: &HashMap<String, serde_json::Value>) -> Option<(String, String)> {
    let status = plan.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if status != "ok" {
        return Some((
            "plan_not_ok".to_string(),
            "reload apply requires a valid dry-run plan".to_string(),
        ));
    }

    let plan_class = plan
        .get("plan_class")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !ALLOWED_PLAN_CLASSES.contains(&plan_class.as_str()) {
        return Some((
            "unsupported_plan_class".to_string(),
            format!(
                "additive reload apply only accepts view_only_change, \
                 maintenance_change, no_change, add_agent, add_window, idle remove_agent, \
                 add_tool_window, and remove_tool_window"
            ),
        ));
    }

    if let Some(blocker) = operation_blocker(plan) {
        return Some(blocker);
    }

    let future_safe = plan
        .get("future_safe_to_apply")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !future_safe {
        return Some((
            "plan_not_future_safe".to_string(),
            "dry-run plan is not future-safe for additive apply".to_string(),
        ));
    }

    if matches!(
        plan_class.as_str(),
        "add_agent" | "add_window" | "remove_agent" | "add_tool_window" | "remove_tool_window"
    ) {
        if let Some(blocker) = namespace_patch_blocker(plan) {
            return Some(blocker);
        }
    }

    None
}

pub fn plan_blocked_result(
    old_graph: &Option<HashMap<String, serde_json::Value>>,
    plan: &HashMap<String, serde_json::Value>,
    blocker: &(String, String),
    namespace_diagnostics: &HashMap<String, serde_json::Value>,
) -> AdditiveReloadApplyResult {
    let (reason, message) = blocker.clone();
    let old_graph_version = old_graph
        .as_ref()
        .and_then(|g| g.get("version").and_then(|v| v.as_i64()));

    let old_config_signature = old_graph
        .as_ref()
        .and_then(|g| g.get("config_signature").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let new_config_signature = plan
        .get("new_config_signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), serde_json::json!(reason));
    diagnostics.insert("message".to_string(), serde_json::json!(message));
    diagnostics.insert(
        "namespace".to_string(),
        serde_json::json!(namespace_diagnostics),
    );

    AdditiveReloadApplyResult {
        status: "blocked".to_string(),
        stage: "plan".to_string(),
        plan_class: plan
            .get("plan_class")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        old_graph_version,
        target_graph_version: None,
        published_graph_version: None,
        old_config_signature,
        new_config_signature,
        plan: Some(clone_plan(plan)),
        namespace_patch: None,
        runtime_mount: None,
        publish_transaction: None,
        diagnostics,
    }
}

fn operation_blocker(plan: &HashMap<String, serde_json::Value>) -> Option<(String, String)> {
    let unsupported = unsupported_operations(plan);
    if !unsupported.is_empty() {
        return Some((
            "unsupported_operations".to_string(),
            format!(
                "additive reload apply rejects operations: {}",
                unsupported.join(",")
            ),
        ));
    }
    None
}

pub fn unsupported_operations(plan: &HashMap<String, serde_json::Value>) -> Vec<String> {
    let operations = plan
        .get("operations")
        .and_then(|v| v.as_array())
        .map(|arr| arr.clone())
        .unwrap_or_default();

    let mut unsupported = HashSet::new();

    for item in operations {
        if let Some(obj) = item.as_object() {
            let name = operation_name(obj);
            if name == "layout_change" {
                if let Some(change) = obj.get("change").and_then(|v| v.as_str()) {
                    if change == "remove_window" {
                        continue;
                    }
                }
            }
            if !ALLOWED_OPERATIONS.contains(&name.as_str()) {
                unsupported.insert(name);
            }
        }
    }

    let mut result: Vec<String> = unsupported.into_iter().collect();
    result.sort();
    result
}

fn operation_name(item: &serde_json::Map<String, serde_json::Value>) -> String {
    item.get("op")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn namespace_patch_blocker(plan: &HashMap<String, serde_json::Value>) -> Option<(String, String)> {
    let patch_plan = plan
        .get("namespace_patch_plan")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<String, serde_json::Value>>()
        })
        .unwrap_or_default();

    let status = patch_plan
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if status != "planned" {
        return Some((
            "namespace_patch_plan_not_planned".to_string(),
            "additive reload apply requires an unblocked namespace patch plan".to_string(),
        ));
    }

    let blocked_operations = patch_plan
        .get("blocked_operations")
        .and_then(|v| v.as_array())
        .map(|arr| !arr.is_empty())
        .unwrap_or(false);

    if blocked_operations {
        return Some((
            "namespace_patch_plan_blocked".to_string(),
            "additive reload apply requires zero blocked namespace operations".to_string(),
        ));
    }

    let scope = patch_plan
        .get("scope")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<String, serde_json::Value>>()
        })
        .unwrap_or_default();

    let verified = scope
        .get("verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !verified {
        return Some((
            "namespace_scope_unverified".to_string(),
            "additive reload apply requires verified project namespace scope".to_string(),
        ));
    }

    None
}

fn clone_plan(plan: &HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    plan.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}
