use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::app::CcbdApp;
use crate::reload_plan::{NamespacePatchStep, ReloadPlan};
use crate::services::project_namespace::{NamespaceWindow, ProjectNamespace};
use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};

/// Result of applying a reload transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadTransactionResult {
    pub status: String,
    pub stage: String,
    pub plan_class: String,
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub added_agents: Vec<String>,
    pub removed_agents: Vec<String>,
    pub added_windows: Vec<String>,
    pub removed_windows: Vec<String>,
    pub registry_before: Vec<String>,
    pub registry_after: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_patch: Option<serde_json::Value>,
}

impl ReloadTransactionResult {
    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "status": self.status,
            "stage": self.stage,
            "plan_class": self.plan_class,
            "applied": self.applied,
            "blocker": self.blocker,
            "error": self.error,
            "added_agents": self.added_agents,
            "removed_agents": self.removed_agents,
            "added_windows": self.added_windows,
            "removed_windows": self.removed_windows,
            "registry_before": self.registry_before,
            "registry_after": self.registry_after,
            "namespace_patch": self.namespace_patch,
        })
    }
}

/// Lightweight transaction context for a reload apply. Holds the planned
/// mutation and bookkeeping; it intentionally does not manage tmux pane
/// lifecycle directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadTransaction {
    pub transaction_id: String,
    pub plan: ReloadPlan,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub rollback_available: bool,
}

impl ReloadTransaction {
    pub fn new(plan: ReloadPlan) -> Self {
        Self {
            transaction_id: uuid::Uuid::new_v4().to_string(),
            plan,
            status: "pending".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            rollback_available: false,
        }
    }

    pub fn commit(&mut self) {
        self.status = "committed".into();
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn rollback(&mut self) {
        self.status = "rolled_back".into();
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn to_record(&self) -> serde_json::Value {
        serde_json::json!({
            "transaction_id": self.transaction_id,
            "plan": self.plan.to_record(),
            "status": self.status,
            "created_at": self.created_at,
            "completed_at": self.completed_at,
            "rollback_available": self.rollback_available,
        })
    }
}

/// Apply a reload plan to the daemon application state.
///
/// Only additive and idle-removal mutations are performed. Any plan that is
/// not future-safe or contains unsupported operations is rejected without
/// touching tmux panes.
pub fn apply_reload_plan(
    app: &mut CcbdApp,
    plan: &ReloadPlan,
    new_config: &ccb_agents::models::ProjectConfig,
) -> ReloadTransactionResult {
    let registry_before = app.registry.all_entries();
    let registry_before_names: Vec<String> = registry_before
        .iter()
        .map(|e| e.agent_name.clone())
        .collect();

    if let Some(blocker) = plan_blocker(plan) {
        return ReloadTransactionResult {
            status: "blocked".into(),
            stage: "plan".into(),
            plan_class: plan.plan_class.clone(),
            applied: false,
            blocker: Some(blocker),
            error: None,
            added_agents: Vec::new(),
            removed_agents: Vec::new(),
            added_windows: Vec::new(),
            removed_windows: Vec::new(),
            registry_before: registry_before_names.clone(),
            registry_after: registry_before_names,
            namespace_patch: Some(
                serde_json::to_value(&plan.namespace_patch_plan).unwrap_or_default(),
            ),
        };
    }

    if plan.plan_class == "no_change"
        || plan.plan_class == "view_only_change"
        || plan.plan_class == "maintenance_change"
    {
        let registry_after = app.registry.all_entries();
        return ReloadTransactionResult {
            status: "ok".into(),
            stage: "noop".into(),
            plan_class: plan.plan_class.clone(),
            applied: false,
            blocker: None,
            error: None,
            added_agents: Vec::new(),
            removed_agents: Vec::new(),
            added_windows: Vec::new(),
            removed_windows: Vec::new(),
            registry_before: registry_before_names.clone(),
            registry_after: registry_after
                .iter()
                .map(|e| e.agent_name.clone())
                .collect(),
            namespace_patch: Some(
                serde_json::to_value(&plan.namespace_patch_plan).unwrap_or_default(),
            ),
        };
    }

    let namespace = app.project_namespace.load().cloned();
    let mut added_agents: Vec<String> = Vec::new();
    let mut removed_agents: Vec<String> = Vec::new();
    let mut added_windows: Vec<String> = Vec::new();
    let mut removed_windows: Vec<String> = Vec::new();

    // Apply registry mutations first. Agent removal is only allowed when the
    // agent is idle (no pane, no outstanding dispatcher work).
    for op in &plan.operations {
        match op.op.as_str() {
            "add_agent" => {
                if let Some(agent_name) = op.details.get("agent").and_then(|v| v.as_str()) {
                    apply_add_agent(app, new_config, agent_name, &mut added_agents);
                }
            }
            "remove_agent" => {
                if let Some(agent_name) = op.details.get("agent").and_then(|v| v.as_str()) {
                    apply_remove_agent(app, agent_name, &mut removed_agents);
                }
            }
            _ => {}
        }
    }

    // Apply namespace mutations if scope is verified and a namespace exists.
    // Pane creation is simulated by registering the expected slot; pane
    // removal is only performed for idle (empty/tool) windows.
    if let Some(mut namespace) = namespace {
        if plan.namespace_patch_plan.scope.verified {
            apply_namespace_patch(
                &plan.namespace_patch_plan.steps,
                &mut namespace,
                &app.registry,
                &mut added_windows,
                &mut removed_windows,
            );
            if let Err(e) = app.project_namespace.mount(namespace) {
                return failed_result(
                    &plan.plan_class,
                    &registry_before_names,
                    &format!("failed to persist namespace: {e}"),
                    Some(serde_json::to_value(&plan.namespace_patch_plan).unwrap_or_default()),
                );
            }
        }
    }

    let registry_after = app.registry.all_entries();
    let registry_after_names: Vec<String> = registry_after
        .iter()
        .map(|e| e.agent_name.clone())
        .collect();

    ReloadTransactionResult {
        status: "ok".into(),
        stage: "applied".into(),
        plan_class: plan.plan_class.clone(),
        applied: true,
        blocker: None,
        error: None,
        added_agents,
        removed_agents,
        added_windows,
        removed_windows,
        registry_before: registry_before_names,
        registry_after: registry_after_names,
        namespace_patch: Some(serde_json::to_value(&plan.namespace_patch_plan).unwrap_or_default()),
    }
}

fn plan_blocker(plan: &ReloadPlan) -> Option<String> {
    if plan.status != "ok" {
        return Some(format!("plan status is '{}'", plan.status));
    }
    let allowed_classes: HashSet<&str> = [
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
    if !allowed_classes.contains(plan.plan_class.as_str()) {
        return Some(format!(
            "unsupported plan_class '{}' for additive reload apply",
            plan.plan_class
        ));
    }

    let allowed_ops: HashSet<&str> = [
        "view_only_change",
        "maintenance_change",
        "add_agent",
        "add_window",
        "remove_agent",
        "add_tool_window",
        "remove_tool_window",
        "layout_change",
    ]
    .iter()
    .copied()
    .collect();
    let unsupported: Vec<String> = plan
        .operations
        .iter()
        .filter(|o| {
            if o.op == "layout_change"
                && o.details.get("change").and_then(|v| v.as_str()) == Some("remove_window")
            {
                return false;
            }
            !allowed_ops.contains(o.op.as_str())
        })
        .map(|o| o.op.clone())
        .collect();
    if !unsupported.is_empty() {
        return Some(format!(
            "unsupported operations: {}",
            unsupported.join(", ")
        ));
    }

    if !plan.future_safe_to_apply {
        return Some("plan is not future-safe for additive apply".into());
    }

    // Mutating plans require a verified namespace patch plan with no blocked
    // operations.
    if is_mutating_plan_class(&plan.plan_class) {
        if plan.namespace_patch_plan.status != "planned" {
            return Some(format!(
                "namespace patch plan is '{}' but must be 'planned'",
                plan.namespace_patch_plan.status
            ));
        }
        if !plan.namespace_patch_plan.blocked_operations.is_empty() {
            return Some("namespace patch plan contains blocked operations".into());
        }
        if !plan.namespace_patch_plan.scope.verified {
            return Some("namespace scope is unverified".into());
        }
    }

    None
}

fn is_mutating_plan_class(plan_class: &str) -> bool {
    matches!(
        plan_class,
        "add_agent" | "add_window" | "remove_agent" | "add_tool_window" | "remove_tool_window"
    )
}

fn apply_add_agent(
    app: &mut CcbdApp,
    new_config: &ccb_agents::models::ProjectConfig,
    agent_name: &str,
    added_agents: &mut Vec<String>,
) {
    if app.registry.get(agent_name).is_some() {
        return;
    }
    let spec = new_config.agents.get(agent_name);
    let workspace_path = spec
        .and_then(|s| s.workspace_path.clone())
        .unwrap_or_else(|| app.layout.project_root.to_string());
    app.registry.register(AgentRuntimeEntry {
        agent_name: agent_name.to_string(),
        provider: spec.map(|s| s.provider.clone()).unwrap_or_default(),
        state: "registered".into(),
        health: "unknown".into(),
        pane_id: None,
        workspace_path: Some(workspace_path),
        runtime_pid: None,
        session_id: None,
        restart_count: 0,
    });
    if !app.dispatcher.agent_names.contains(&agent_name.to_string()) {
        app.dispatcher.agent_names.push(agent_name.to_string());
    }
    added_agents.push(agent_name.to_string());
}

fn apply_remove_agent(app: &mut CcbdApp, agent_name: &str, removed_agents: &mut Vec<String>) {
    let Some(entry) = app.registry.get(agent_name) else {
        return;
    };
    // Only remove idle agents to avoid killing running panes.
    let safe_states: HashSet<&str> = ["registered", "idle", "stopped"].iter().copied().collect();
    if !safe_states.contains(entry.state.as_str()) {
        return;
    }
    if entry.pane_id.is_some() {
        return;
    }
    if app.dispatcher.state.has_outstanding(agent_name) {
        return;
    }
    app.registry.remove(agent_name);
    app.dispatcher.agent_names.retain(|n| n != agent_name);
    removed_agents.push(agent_name.to_string());
}

fn apply_namespace_patch(
    steps: &[NamespacePatchStep],
    namespace: &mut ProjectNamespace,
    registry: &AgentRegistry,
    added_windows: &mut Vec<String>,
    removed_windows: &mut Vec<String>,
) {
    let mut pending_windows: Vec<NamespaceWindow> = namespace.windows.clone();
    let pending_names: HashSet<String> = pending_windows.iter().map(|w| w.name.clone()).collect();

    for step in steps {
        match step.action.as_str() {
            "create_window" => {
                if let Some(window_name) = &step.window {
                    if !pending_names.contains(window_name) {
                        pending_windows.push(NamespaceWindow {
                            name: window_name.clone(),
                            window_id: None,
                            agents: Vec::new(),
                        });
                        added_windows.push(window_name.clone());
                    }
                }
            }
            "create_agent_pane" => {
                if let (Some(window_name), Some(agent_name)) = (&step.window, &step.agent) {
                    if let Some(window) =
                        pending_windows.iter_mut().find(|w| &w.name == window_name)
                    {
                        if !window.agents.contains(agent_name) {
                            window.agents.push(agent_name.clone());
                        }
                    }
                }
            }
            "kill_agent_pane" => {
                if let (Some(window_name), Some(agent_name)) = (&step.window, &step.agent) {
                    if let Some(window) =
                        pending_windows.iter_mut().find(|w| &w.name == window_name)
                    {
                        // Only remove the agent reference if it is safe to do
                        // so in the registry state.
                        if is_agent_removable_from_namespace(registry, agent_name) {
                            window.agents.retain(|a| a != agent_name);
                        }
                    }
                }
            }
            "create_tool_pane" => {
                // Tool panes are represented as windows in the namespace; the
                // create_window step already added the window.
            }
            "kill_tool_window" => {
                if let Some(window_name) = &step.window {
                    pending_windows.retain(|w| {
                        if &w.name == window_name {
                            removed_windows.push(window_name.clone());
                            false
                        } else {
                            true
                        }
                    });
                }
            }
            "refresh_project_view" => {
                // No namespace mutation required.
            }
            _ => {}
        }
    }

    namespace.windows = pending_windows;
    namespace.agent_names = namespace
        .windows
        .iter()
        .flat_map(|w| w.agents.clone())
        .collect();
}

fn is_agent_removable_from_namespace(registry: &AgentRegistry, agent_name: &str) -> bool {
    let Some(entry) = registry.get(agent_name) else {
        return true;
    };
    let safe_states: HashSet<&str> = ["registered", "idle", "stopped"].iter().copied().collect();
    safe_states.contains(entry.state.as_str()) && entry.pane_id.is_none()
}

fn failed_result(
    plan_class: &str,
    registry_before: &[String],
    error: &str,
    namespace_patch: Option<serde_json::Value>,
) -> ReloadTransactionResult {
    ReloadTransactionResult {
        status: "error".into(),
        stage: "apply".into(),
        plan_class: plan_class.into(),
        applied: false,
        blocker: None,
        error: Some(error.into()),
        added_agents: Vec::new(),
        removed_agents: Vec::new(),
        added_windows: Vec::new(),
        removed_windows: Vec::new(),
        registry_before: registry_before.to_vec(),
        registry_after: registry_before.to_vec(),
        namespace_patch,
    }
}
