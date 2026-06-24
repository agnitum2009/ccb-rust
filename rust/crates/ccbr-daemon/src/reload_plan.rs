use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::reload_patch::build_namespace_patch_plan;
use ccbr_agents::models::{ProjectConfig, WindowSpec};

/// Priority ordering for plan classification. Higher values are more disruptive.
const PLAN_PRIORITY: &[(&str, i32)] = &[
    ("no_change", 0),
    ("view_only_change", 10),
    ("maintenance_change", 20),
    ("add_tool_window", 30),
    ("add_agent", 40),
    ("add_window", 50),
    ("remove_tool_window", 55),
    ("layout_change", 60),
    ("change_tool_window", 65),
    ("move_agent", 70),
    ("remove_agent", 80),
    ("replace_agent", 90),
];

/// A single reload operation detected between the current and new config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadOperation {
    pub op: String,
    #[serde(flatten)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

impl ReloadOperation {
    pub fn reason(&self) -> String {
        self.details
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn to_record(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({"op": self.op});
        if let serde_json::Value::Object(map) = &mut obj {
            map.extend(self.details.clone());
        }
        obj
    }
}

/// Scope verification for the current project namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespacePatchScope {
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_socket_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
}

/// A single step in the namespace patch plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespacePatchStep {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Plan for how the tmux namespace should be patched. Kept intentionally
/// explicit so unsafe operations surface as blocked steps rather than pane
/// kills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespacePatchPlan {
    pub status: String,
    pub mutation_enabled: bool,
    pub apply_deferred: bool,
    pub scope: NamespacePatchScope,
    pub steps: Vec<NamespacePatchStep>,
    #[serde(default)]
    pub blocked_operations: Vec<serde_json::Value>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Dry-run plan produced by diffing the current and new project configs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadPlan {
    pub status: String,
    pub dry_run: bool,
    pub mutation_enabled: bool,
    pub safe_to_apply: bool,
    pub future_safe_to_apply: bool,
    pub plan_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_config_signature: Option<String>,
    pub old_known_agents: Vec<String>,
    pub new_known_agents: Vec<String>,
    pub operations: Vec<ReloadOperation>,
    #[serde(default)]
    pub drain_intents: Vec<serde_json::Value>,
    pub namespace_patch_plan: NamespacePatchPlan,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl ReloadPlan {
    pub fn to_record(&self) -> serde_json::Value {
        let added: Vec<String> = self
            .operations
            .iter()
            .filter(|o| o.op == "add_agent")
            .filter_map(|o| {
                o.details
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        let removed: Vec<String> = self
            .operations
            .iter()
            .filter(|o| o.op == "remove_agent")
            .filter_map(|o| {
                o.details
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        let modified: Vec<String> = self
            .operations
            .iter()
            .filter(|o| o.op == "replace_agent")
            .filter_map(|o| {
                o.details
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        let unchanged: Vec<String> = self
            .old_known_agents
            .iter()
            .filter(|a| self.new_known_agents.contains(*a) && !modified.contains(*a))
            .cloned()
            .collect();
        serde_json::json!({
            "status": self.status,
            "dry_run": self.dry_run,
            "mutation_enabled": self.mutation_enabled,
            "safe_to_apply": self.safe_to_apply,
            "future_safe_to_apply": self.future_safe_to_apply,
            "plan_class": self.plan_class,
            "old_config_signature": self.old_config_signature,
            "new_config_signature": self.new_config_signature,
            "old_known_agents": self.old_known_agents,
            "new_known_agents": self.new_known_agents,
            "operations": self.operations.iter().map(|o| o.to_record()).collect::<Vec<_>>(),
            "drain_intents": self.drain_intents,
            "namespace_patch_plan": self.namespace_patch_plan,
            "reasons": self.reasons,
            "warnings": self.warnings,
            "errors": self.errors,
            "added_agents": added,
            "removed_agents": removed,
            "modified_agents": modified,
            "unchanged_agents": unchanged,
        })
    }

    pub fn is_noop(&self) -> bool {
        self.plan_class == "no_change" || self.operations.is_empty()
    }

    /// Returns true when the plan's own classification and operation set are
    /// safe for an additive reload apply. Mirrors Python's `_future_safe_to_apply`.
    pub fn is_future_safe_to_apply(&self) -> bool {
        future_safe_to_apply(&self.plan_class, &self.operations)
    }
}

/// Build a dry-run plan from the current and new project configs.
///
/// `current_config_identity` is the previously-computed identity payload for
/// the current graph (used to detect no-change fast paths). If `None`, the
/// identity is recomputed from `current_config`.
pub fn build_reload_dry_run_plan(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
    current_config_identity: Option<&serde_json::Value>,
    project_id: Option<&str>,
    current_namespace: Option<&crate::services::project_namespace::ProjectNamespace>,
) -> ReloadPlan {
    let old_identity = current_config_identity
        .cloned()
        .unwrap_or_else(|| project_config_identity_payload(current_config));
    let new_identity = project_config_identity_payload(new_config);
    let mut warnings: Vec<String> = Vec::new();

    let old_sig = old_identity
        .get("config_signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let new_sig = new_identity
        .get("config_signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if old_sig.is_some() && old_sig == new_sig {
        return identity_preserving_plan(
            current_config,
            new_config,
            old_identity,
            new_identity,
            warnings,
            project_id,
            current_namespace,
        );
    }

    let mut operations = build_operations(current_config, new_config);
    if operations.is_empty() {
        operations.push(ReloadOperation {
            op: "layout_change".into(),
            details: serde_json::from_value(serde_json::json!({
                "change": "unclassified_identity_change",
                "reason": "config identity changed but no narrower operation was detected",
            }))
            .unwrap(),
        });
        warnings.push("Config identity changed; diff degraded to layout_change.".into());
    }

    if operations.iter().any(|o| o.op == "replace_agent") {
        warnings.push(
            "Existing agent spec changes are conservatively classified as replace_agent.".into(),
        );
    }

    let plan_class = select_plan_class(&operations);
    let namespace_patch_plan = build_namespace_patch_plan(
        current_config,
        new_config,
        &operations,
        project_id,
        current_namespace,
    );

    let reasons = operation_reasons(&operations);
    ReloadPlan {
        status: "ok".into(),
        dry_run: true,
        mutation_enabled: false,
        safe_to_apply: false,
        future_safe_to_apply: future_safe_to_apply(&plan_class, &operations),
        plan_class,
        old_config_signature: old_sig,
        new_config_signature: new_sig,
        old_known_agents: agents_from_identity(&old_identity),
        new_known_agents: agents_from_identity(&new_identity),
        operations,
        drain_intents: Vec::new(),
        namespace_patch_plan,
        reasons,
        warnings,
        errors: Vec::new(),
    }
}

/// Build a plan when the new config could not be loaded or validated.
pub fn build_invalid_reload_dry_run_plan(
    current_config: &ProjectConfig,
    error: &str,
    current_config_identity: Option<&serde_json::Value>,
) -> ReloadPlan {
    let old_identity = current_config_identity
        .cloned()
        .unwrap_or_else(|| project_config_identity_payload(current_config));

    ReloadPlan {
        status: "invalid_config".into(),
        dry_run: true,
        mutation_enabled: false,
        safe_to_apply: false,
        future_safe_to_apply: false,
        plan_class: "invalid_config".into(),
        old_config_signature: old_identity
            .get("config_signature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        new_config_signature: None,
        old_known_agents: agents_from_identity(&old_identity),
        new_known_agents: Vec::new(),
        operations: Vec::new(),
        drain_intents: Vec::new(),
        namespace_patch_plan: NamespacePatchPlan {
            status: "not_planned".into(),
            mutation_enabled: false,
            apply_deferred: true,
            scope: NamespacePatchScope {
                verified: false,
                project_id: None,
                tmux_socket_path: None,
                tmux_session_name: None,
                namespace_epoch: None,
                reason: Some("namespace patch planning requires a valid new config".into()),
            },
            steps: Vec::new(),
            blocked_operations: vec![serde_json::json!({
                "op": "invalid_config",
                "reason": error,
            })],
            warnings: vec!["namespace patch planning requires a valid new config".into()],
        },
        reasons: vec!["new config could not be loaded or validated".into()],
        warnings: Vec::new(),
        errors: vec![error.into()],
    }
}

fn identity_preserving_plan(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
    old_identity: serde_json::Value,
    new_identity: serde_json::Value,
    mut warnings: Vec<String>,
    project_id: Option<&str>,
    current_namespace: Option<&crate::services::project_namespace::ProjectNamespace>,
) -> ReloadPlan {
    let old_full = canonical_config_record(current_config, true);
    let new_full = canonical_config_record(new_config, true);

    let (operations, plan_class) = if old_full == new_full {
        (Vec::new(), "no_change".to_string())
    } else {
        let old_without_view = canonical_config_record(current_config, false);
        let new_without_view = canonical_config_record(new_config, false);
        if old_without_view != new_without_view {
            warnings.push(
                "Config identity is unchanged but non-sidebar presentation fields could not be split more narrowly.".into(),
            );
        }
        let ops = vec![ReloadOperation {
            op: "view_only_change".into(),
            details: serde_json::from_value(serde_json::json!({
                "field": "sidebar_view",
                "reason": "config identity is unchanged; only presentation fields affect the diff",
            }))
            .unwrap(),
        }];
        (ops, "view_only_change".to_string())
    };

    let namespace_patch_plan = build_namespace_patch_plan(
        current_config,
        new_config,
        &operations,
        project_id,
        current_namespace,
    );

    let reasons = operation_reasons(&operations);
    ReloadPlan {
        status: "ok".into(),
        dry_run: true,
        mutation_enabled: false,
        safe_to_apply: false,
        future_safe_to_apply: true,
        plan_class,
        old_config_signature: old_identity
            .get("config_signature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        new_config_signature: new_identity
            .get("config_signature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        old_known_agents: agents_from_identity(&old_identity),
        new_known_agents: agents_from_identity(&new_identity),
        operations,
        drain_intents: Vec::new(),
        namespace_patch_plan,
        reasons,
        warnings,
        errors: Vec::new(),
    }
}

fn build_operations(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
) -> Vec<ReloadOperation> {
    let mut operations: Vec<ReloadOperation> = Vec::new();

    let old_agents: HashSet<String> = current_config.agents.keys().cloned().collect();
    let new_agents: HashSet<String> = new_config.agents.keys().cloned().collect();

    let old_window_by_agent = agent_window_map(current_config);
    let new_window_by_agent = agent_window_map(new_config);
    let old_windows = window_record_map(current_config);
    let new_windows = window_record_map(new_config);
    let old_tools = tool_window_record_map(current_config);
    let new_tools = tool_window_record_map(new_config);

    let added_windows: HashSet<String> = set_difference(
        &new_windows.keys().cloned().collect(),
        &old_windows.keys().cloned().collect(),
    );
    for window_name in ordered_windows(new_config, &added_windows) {
        let record = &new_windows[&window_name];
        operations.push(ReloadOperation {
            op: "add_window".into(),
            details: serde_json::from_value(serde_json::json!({
                "window": window_name,
                "agents": record.agent_names,
                "reason": "window exists only in new config",
            }))
            .unwrap(),
        });
    }

    for agent_name in sorted_set_diff(&new_agents, &old_agents) {
        operations.push(ReloadOperation {
            op: "add_agent".into(),
            details: serde_json::from_value(serde_json::json!({
                "agent": agent_name,
                "window": new_window_by_agent.get(&agent_name),
                "reason": "agent exists only in new config",
            }))
            .unwrap(),
        });
    }

    for agent_name in sorted_set_diff(&old_agents, &new_agents) {
        operations.push(ReloadOperation {
            op: "remove_agent".into(),
            details: serde_json::from_value(serde_json::json!({
                "agent": agent_name,
                "window": old_window_by_agent.get(&agent_name),
                "reason": "agent exists only in current published config",
            }))
            .unwrap(),
        });
    }

    for agent_name in sorted_set_intersection(&old_agents, &new_agents) {
        let old_window = old_window_by_agent.get(&agent_name).cloned();
        let new_window = new_window_by_agent.get(&agent_name).cloned();
        if old_window != new_window {
            operations.push(ReloadOperation {
                op: "move_agent".into(),
                details: serde_json::from_value(serde_json::json!({
                    "agent": agent_name,
                    "from_window": old_window,
                    "to_window": new_window,
                    "reason": "existing agent window membership changed",
                }))
                .unwrap(),
            });
        }
    }

    for agent_name in sorted_set_intersection(&old_agents, &new_agents) {
        let old_record = current_config.agents.get(&agent_name).unwrap().to_record();
        let new_record = new_config.agents.get(&agent_name).unwrap().to_record();
        if old_record != new_record {
            operations.push(ReloadOperation {
                op: "replace_agent".into(),
                details: serde_json::from_value(serde_json::json!({
                    "agent": agent_name,
                    "fields": changed_fields(&old_record, &new_record),
                    "reason": "existing agent spec changed",
                }))
                .unwrap(),
            });
        }
    }

    operations.extend(topology_operations(
        current_config,
        new_config,
        &old_windows,
        &new_windows,
    ));
    operations.extend(tool_window_operations(
        current_config,
        new_config,
        &old_tools,
        &new_tools,
    ));
    operations.extend(maintenance_operations(current_config, new_config));
    operations
}

fn topology_operations(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
    old_windows: &HashMap<String, WindowSpec>,
    new_windows: &HashMap<String, WindowSpec>,
) -> Vec<ReloadOperation> {
    let mut operations: Vec<ReloadOperation> = Vec::new();

    let removed_windows: HashSet<String> = set_difference(
        &old_windows.keys().cloned().collect(),
        &new_windows.keys().cloned().collect(),
    );
    for window_name in ordered_windows(current_config, &removed_windows) {
        operations.push(ReloadOperation {
            op: "layout_change".into(),
            details: serde_json::from_value(serde_json::json!({
                "window": window_name,
                "change": "remove_window",
                "reason": "window exists only in current published config",
            }))
            .unwrap(),
        });
    }

    let common_windows: HashSet<String> = set_intersection(
        &old_windows.keys().cloned().collect(),
        &new_windows.keys().cloned().collect(),
    );
    for window_name in ordered_windows(new_config, &common_windows) {
        let old_record = &old_windows[&window_name];
        let new_record = &new_windows[&window_name];
        let old_set: HashSet<String> = old_record.agent_names.iter().cloned().collect();
        let new_set: HashSet<String> = new_record.agent_names.iter().cloned().collect();
        if old_set != new_set {
            continue;
        }
        let changed: Vec<String> = ["order", "layout_spec", "agent_names"]
            .iter()
            .filter(|&&field| {
                let (old_val, new_val) = match field {
                    "order" => (
                        serde_json::to_value(old_record.order).unwrap(),
                        serde_json::to_value(new_record.order).unwrap(),
                    ),
                    "layout_spec" => (
                        serde_json::to_value(old_record.layout_spec.clone()).unwrap(),
                        serde_json::to_value(new_record.layout_spec.clone()).unwrap(),
                    ),
                    "agent_names" => (
                        serde_json::to_value(old_record.agent_names.clone()).unwrap(),
                        serde_json::to_value(new_record.agent_names.clone()).unwrap(),
                    ),
                    _ => unreachable!(),
                };
                old_val != new_val
            })
            .map(|s| s.to_string())
            .collect();
        if !changed.is_empty() {
            operations.push(ReloadOperation {
                op: "layout_change".into(),
                details: serde_json::from_value(serde_json::json!({
                    "window": window_name,
                    "fields": changed,
                    "reason": "existing window layout changed without adding or removing agents",
                }))
                .unwrap(),
            });
        }
    }

    if current_config.entry_window != new_config.entry_window {
        operations.push(ReloadOperation {
            op: "layout_change".into(),
            details: serde_json::from_value(serde_json::json!({
                "field": "entry_window",
                "old": current_config.entry_window,
                "new": new_config.entry_window,
                "reason": "entry window changed",
            }))
            .unwrap(),
        });
    }

    let old_sidebar = current_config.sidebar.as_ref().map(|s| s.to_record());
    let new_sidebar = new_config.sidebar.as_ref().map(|s| s.to_record());
    if old_sidebar != new_sidebar {
        operations.push(ReloadOperation {
            op: "layout_change".into(),
            details: serde_json::from_value(serde_json::json!({
                "field": "sidebar",
                "reason": "sidebar topology changed",
            }))
            .unwrap(),
        });
    }

    operations
}

fn tool_window_operations(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
    old_tools: &HashMap<String, ccbr_agents::models::ToolWindowSpec>,
    new_tools: &HashMap<String, ccbr_agents::models::ToolWindowSpec>,
) -> Vec<ReloadOperation> {
    let mut operations: Vec<ReloadOperation> = Vec::new();

    let added_tools: HashSet<String> = set_difference(
        &new_tools.keys().cloned().collect(),
        &old_tools.keys().cloned().collect(),
    );
    for window_name in ordered_tool_windows(new_config, &added_tools) {
        let record = &new_tools[&window_name];
        operations.push(ReloadOperation {
            op: "add_tool_window".into(),
            details: serde_json::from_value(serde_json::json!({
                "window": window_name,
                "command": record.command,
                "reason": "tool window exists only in new config",
            }))
            .unwrap(),
        });
    }

    let removed_tools: HashSet<String> = set_difference(
        &old_tools.keys().cloned().collect(),
        &new_tools.keys().cloned().collect(),
    );
    for window_name in ordered_tool_windows(current_config, &removed_tools) {
        operations.push(ReloadOperation {
            op: "remove_tool_window".into(),
            details: serde_json::from_value(serde_json::json!({
                "window": window_name,
                "reason": "tool window exists only in current published config",
            }))
            .unwrap(),
        });
    }

    let common_tools: HashSet<String> = set_intersection(
        &old_tools.keys().cloned().collect(),
        &new_tools.keys().cloned().collect(),
    );
    for window_name in ordered_tool_windows(new_config, &common_tools) {
        let old_record = &old_tools[&window_name];
        let new_record = &new_tools[&window_name];
        if old_record.command != new_record.command {
            operations.push(ReloadOperation {
                op: "change_tool_window".into(),
                details: serde_json::from_value(serde_json::json!({
                    "window": window_name,
                    "fields": ["command"],
                    "reason": "existing tool window changed; explicit restart policy is not implemented",
                }))
                .unwrap(),
            });
        }
    }

    operations
}

fn maintenance_operations(
    current_config: &ProjectConfig,
    new_config: &ProjectConfig,
) -> Vec<ReloadOperation> {
    let old_record = current_config
        .maintenance_heartbeat
        .as_ref()
        .map(|m| m.to_record());
    let new_record = new_config
        .maintenance_heartbeat
        .as_ref()
        .map(|m| m.to_record());
    if old_record == new_record {
        return Vec::new();
    }
    vec![ReloadOperation {
        op: "maintenance_change".into(),
        details: serde_json::from_value(serde_json::json!({
            "fields": changed_value_fields(old_record.as_ref(), new_record.as_ref()),
            "reason": "maintenance heartbeat policy changed",
        }))
        .unwrap(),
    }]
}

fn select_plan_class(operations: &[ReloadOperation]) -> String {
    if operations.is_empty() {
        return "no_change".into();
    }
    let priority_map: HashMap<&str, i32> = PLAN_PRIORITY.iter().copied().collect();
    operations
        .iter()
        .map(|o| o.op.as_str())
        .max_by_key(|op| priority_map.get(op).copied().unwrap_or(60))
        .unwrap_or("layout_change")
        .to_string()
}

fn future_safe_to_apply(plan_class: &str, operations: &[ReloadOperation]) -> bool {
    match plan_class {
        "no_change" | "view_only_change" | "maintenance_change" => true,
        "add_tool_window" | "remove_tool_window" => {
            let unsafe_tool_ops: HashSet<&str> = ["change_tool_window"].iter().copied().collect();
            let unsafe_agent_ops: HashSet<&str> =
                ["replace_agent", "move_agent"].iter().copied().collect();
            !operations.iter().any(|o| {
                unsafe_tool_ops.contains(o.op.as_str()) || unsafe_agent_ops.contains(o.op.as_str())
            })
        }
        "remove_agent" => remove_agent_operations_are_safe(operations),
        _ => {
            let unsafe_ops: HashSet<&str> = [
                "replace_agent",
                "move_agent",
                "layout_change",
                "change_tool_window",
            ]
            .iter()
            .copied()
            .collect();
            if operations
                .iter()
                .any(|o| unsafe_ops.contains(o.op.as_str()))
            {
                return false;
            }
            matches!(plan_class, "add_agent" | "add_window")
        }
    }
}

fn remove_agent_operations_are_safe(operations: &[ReloadOperation]) -> bool {
    if operations.is_empty() {
        return false;
    }
    for item in operations {
        if item.op == "remove_agent" {
            continue;
        }
        if item.op == "layout_change" {
            if let Some(change) = item.details.get("change").and_then(|v| v.as_str()) {
                if change == "remove_window" {
                    continue;
                }
            }
        }
        return false;
    }
    operations.iter().any(|o| o.op == "remove_agent")
}

pub(crate) fn warnings_for_status(status: &str) -> Vec<String> {
    match status {
        "planned" => vec!["Namespace patch apply is explicit and only supports additive or idle remove_agent operations.".into()],
        "blocked" => vec!["Namespace patch plan is blocked; reload must remain dry-run/rejected.".into()],
        _ => Vec::new(),
    }
}

fn operation_reasons(operations: &[ReloadOperation]) -> Vec<String> {
    operations
        .iter()
        .map(|item| {
            let reason = item.reason();
            let target = item
                .details
                .get("agent")
                .or_else(|| item.details.get("window"))
                .or_else(|| item.details.get("field"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !reason.is_empty() && !target.is_empty() {
                format!("{} {}: {}", item.op, target, reason)
            } else if !reason.is_empty() {
                format!("{}: {}", item.op, reason)
            } else {
                reason
            }
        })
        .collect()
}

fn canonical_config_record(
    config: &ProjectConfig,
    include_sidebar_view: bool,
) -> serde_json::Value {
    let mut record = config.to_record();
    if let serde_json::Value::Object(ref mut map) = record {
        map.remove("schema_version");
        map.remove("record_type");
        map.remove("source_path");
        if !include_sidebar_view {
            map.remove("sidebar_view");
        }
    }
    record
}

fn agent_window_map(config: &ProjectConfig) -> HashMap<String, String> {
    let mut mapping: HashMap<String, String> = HashMap::new();
    if let Some(windows) = &config.windows {
        for window in windows {
            for agent_name in &window.agent_names {
                mapping.insert(agent_name.clone(), window.name.clone());
            }
        }
    }
    mapping
}

fn window_record_map(config: &ProjectConfig) -> HashMap<String, WindowSpec> {
    config
        .windows
        .as_ref()
        .map(|windows| {
            windows
                .iter()
                .map(|w| (w.name.clone(), w.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn tool_window_record_map(
    config: &ProjectConfig,
) -> HashMap<String, ccbr_agents::models::ToolWindowSpec> {
    config
        .tool_windows
        .as_ref()
        .map(|tools| tools.iter().map(|t| (t.name.clone(), t.clone())).collect())
        .unwrap_or_default()
}

fn ordered_windows(config: &ProjectConfig, names: &HashSet<String>) -> Vec<String> {
    let order: HashMap<String, u32> = config
        .windows
        .as_ref()
        .map(|windows| windows.iter().map(|w| (w.name.clone(), w.order)).collect())
        .unwrap_or_default();
    let mut sorted: Vec<String> = names.iter().cloned().collect();
    sorted.sort_by(|a, b| {
        let ord_a = order.get(a).copied().unwrap_or(999_999);
        let ord_b = order.get(b).copied().unwrap_or(999_999);
        ord_a.cmp(&ord_b).then_with(|| a.cmp(b))
    });
    sorted
}

fn ordered_tool_windows(config: &ProjectConfig, names: &HashSet<String>) -> Vec<String> {
    let order: HashMap<String, u32> = config
        .tool_windows
        .as_ref()
        .map(|tools| tools.iter().map(|t| (t.name.clone(), t.order)).collect())
        .unwrap_or_default();
    let mut sorted: Vec<String> = names.iter().cloned().collect();
    sorted.sort_by(|a, b| {
        let ord_a = order.get(a).copied().unwrap_or(999_999);
        let ord_b = order.get(b).copied().unwrap_or(999_999);
        ord_a.cmp(&ord_b).then_with(|| a.cmp(b))
    });
    sorted
}

fn changed_fields(old_record: &serde_json::Value, new_record: &serde_json::Value) -> Vec<String> {
    let old_keys: HashSet<String> = old_record
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let new_keys: HashSet<String> = new_record
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let all_keys: HashSet<String> = old_keys.union(&new_keys).cloned().collect();
    let mut changed: Vec<String> = all_keys
        .into_iter()
        .filter(|key| old_record.get(key) != new_record.get(key))
        .collect();
    changed.sort();
    changed
}

fn changed_value_fields(
    old_record: Option<&serde_json::Value>,
    new_record: Option<&serde_json::Value>,
) -> Vec<String> {
    changed_fields(
        old_record.unwrap_or(&serde_json::Value::Null),
        new_record.unwrap_or(&serde_json::Value::Null),
    )
}

fn sorted_set_diff(a: &HashSet<String>, b: &HashSet<String>) -> Vec<String> {
    let mut sorted: Vec<String> = a.difference(b).cloned().collect();
    sorted.sort();
    sorted
}

fn sorted_set_intersection(a: &HashSet<String>, b: &HashSet<String>) -> Vec<String> {
    let mut sorted: Vec<String> = a.intersection(b).cloned().collect();
    sorted.sort();
    sorted
}

fn set_difference(a: &HashSet<String>, b: &HashSet<String>) -> HashSet<String> {
    a.difference(b).cloned().collect()
}

fn set_intersection(a: &HashSet<String>, b: &HashSet<String>) -> HashSet<String> {
    a.intersection(b).cloned().collect()
}

pub(crate) fn clean_text(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Compute the canonical config identity payload used for change detection.
/// Mirrors Python's `project_config_identity_payload`.
pub fn project_config_identity_payload(config: &ProjectConfig) -> serde_json::Value {
    let mut canonical = config.to_record();
    if let serde_json::Value::Object(ref mut map) = canonical {
        map.remove("schema_version");
        map.remove("record_type");
        map.remove("source_path");
        map.remove("sidebar_view");
        if let Some(serde_json::Value::Array(tools)) = map.get_mut("tool_windows") {
            for tool in tools {
                if let serde_json::Value::Object(t) = tool {
                    t.remove("label");
                    t.remove("show_in_sidebar");
                }
            }
        }
        if let Some(serde_json::Value::Object(agents)) = map.get_mut("agents") {
            for payload in agents.values_mut() {
                if let serde_json::Value::Object(a) = payload {
                    a.remove("schema_version");
                    a.remove("record_type");
                }
            }
        }
    }

    let encoded = serde_json::to_string(&canonical).unwrap_or_default();
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(encoded.as_bytes());
    let mut known: Vec<String> = config.agents.keys().cloned().collect();
    known.sort();
    serde_json::json!({
        "known_agents": known,
        "config_signature": hex::encode(hash),
    })
}

fn agents_from_identity(identity: &serde_json::Value) -> Vec<String> {
    identity
        .get("known_agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
