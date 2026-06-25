use crate::app::CcbdApp;
use crate::reload_apply_graph::build_reload_service_graph;
use crate::reload_apply_models::AdditiveReloadApplyResult;
use crate::reload_apply_service::run_additive_reload_apply;
use crate::reload_plan::{build_invalid_reload_dry_run_plan, build_reload_dry_run_plan};
use ccbr_agents::config::{load_project_config, project_config_path};
use serde_json::{json, Map, Value};

pub fn handle_project_reload(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let dry_run = truthy(payload.get("dry_run"));

    let current_config = app
        .current_config
        .clone()
        .unwrap_or_else(|| current_config_from_registry(app));

    let config_path = project_config_path(&app.layout);
    if !config_path.exists() {
        let error = format!("project config not found: {config_path}");
        let plan = build_invalid_reload_dry_run_plan(&current_config, &error, None);
        if !dry_run {
            return Ok(non_dry_run_invalid_config_payload(plan.to_record()));
        }
        return Ok(plan.to_record());
    }

    let new_config = match load_project_config(&app.layout) {
        Ok(result) => result.config,
        Err(e) => {
            let error = format!("{e}");
            let plan = build_invalid_reload_dry_run_plan(&current_config, &error, None);
            if !dry_run {
                return Ok(non_dry_run_invalid_config_payload(plan.to_record()));
            }
            return Ok(plan.to_record());
        }
    };

    let current_namespace = app.project_namespace.load();
    let plan = build_reload_dry_run_plan(
        &current_config,
        &new_config,
        None,
        Some(app.project_id()),
        current_namespace,
    );

    if dry_run {
        return Ok(plan.to_record());
    }

    let result = run_additive_reload_apply(
        app,
        &new_config,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let published = result.status == "published";
    let payload = apply_reload_payload(result);
    if published {
        let graph = build_reload_service_graph(app, &new_config);
        app.publish_service_graph(&graph);
    }
    Ok(payload)
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }
        Some(Value::Number(value)) => value.as_i64().is_some_and(|n| n != 0),
        _ => false,
    }
}

fn apply_reload_payload(result: AdditiveReloadApplyResult) -> Value {
    let mut payload: Map<String, Value> = result.to_record().into_iter().collect();
    let plan = payload
        .get("plan")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    if payload.get("status").and_then(Value::as_str) == Some("published") {
        let mut diagnostics = payload
            .get("diagnostics")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        diagnostics.insert("project_view_cache_invalidated".into(), json!(false));
        diagnostics.insert("sidebar_refresh_signal_sent".into(), json!(false));
        payload.insert("diagnostics".into(), Value::Object(diagnostics));
    }
    let published = payload.get("status").and_then(Value::as_str) == Some("published");
    payload.insert("dry_run".into(), json!(false));
    payload.insert("mutation_enabled".into(), json!(published));
    payload.insert("safe_to_apply".into(), json!(published));
    payload.insert(
        "future_safe_to_apply".into(),
        json!(plan
            .get("future_safe_to_apply")
            .and_then(Value::as_bool)
            .unwrap_or(false)),
    );
    for key in [
        "operations",
        "drain_intents",
        "reasons",
        "warnings",
        "namespace_patch_plan",
    ] {
        payload.insert(
            key.into(),
            plan.get(key).cloned().unwrap_or_else(|| json!([])),
        );
    }
    if !plan.contains_key("namespace_patch_plan") {
        payload.insert("namespace_patch_plan".into(), Value::Null);
    }
    payload.insert("errors".into(), json!(apply_errors(&payload)));
    Value::Object(payload)
}

fn non_dry_run_invalid_config_payload(plan: Value) -> Value {
    let mut payload = plan.as_object().cloned().unwrap_or_default();
    let errors = payload
        .get("errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let message = errors
        .iter()
        .filter_map(Value::as_str)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    payload.insert("dry_run".into(), json!(false));
    payload.insert("mutation_enabled".into(), json!(false));
    payload.insert("safe_to_apply".into(), json!(false));
    payload.insert(
        "diagnostics".into(),
        json!({
            "reason": "invalid_config",
            "message": message,
            "graph_published": false,
            "lease_or_lifecycle_written": false,
            "config_watch_started": false,
            "unload_or_replace_executed": false,
        }),
    );
    Value::Object(payload)
}

fn apply_errors(payload: &Map<String, Value>) -> Vec<String> {
    if matches!(
        payload.get("status").and_then(Value::as_str),
        Some("published" | "noop")
    ) {
        return Vec::new();
    }
    let diagnostics = payload
        .get("diagnostics")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let reason = diagnostics
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let message = diagnostics
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    match (reason.is_empty(), message.is_empty()) {
        (false, false) => vec![format!("{reason}: {message}")],
        (false, true) => vec![reason.to_string()],
        _ => Vec::new(),
    }
}

/// Reconstruct a minimal ProjectConfig from the current registry state.
/// This is used as the "old" config when no project config has been loaded
/// yet, and as a fallback for invalid-config plans.
fn current_config_from_registry(app: &CcbdApp) -> ccbr_agents::models::ProjectConfig {
    let agents: std::collections::HashMap<String, ccbr_agents::models::AgentSpec> = app
        .registry
        .all_entries()
        .iter()
        .map(|e| {
            let spec = minimal_agent_spec(&e.agent_name, &e.provider);
            (e.agent_name.clone(), spec)
        })
        .collect();
    let default_agents: Vec<String> = app
        .registry
        .all_entries()
        .iter()
        .map(|e| e.agent_name.clone())
        .collect();
    ccbr_agents::models::ProjectConfig {
        version: ccbr_agents::models::SCHEMA_VERSION,
        default_agents,
        agents,
        cmd_enabled: false,
        layout_spec: None,
        windows: None,
        tool_windows: None,
        entry_window: None,
        sidebar: None,
        sidebar_view: None,
        maintenance_heartbeat: None,
        windows_explicit: None,
        topology_signature: None,
        source_path: None,
    }
}

fn minimal_agent_spec(name: &str, provider: &str) -> ccbr_agents::models::AgentSpec {
    ccbr_agents::models::AgentSpec {
        name: name.to_string(),
        provider: provider.to_string(),
        target: name.to_string(),
        workspace_mode: ccbr_agents::models::WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: ccbr_agents::models::RuntimeMode::PaneBacked,
        restore_default: ccbr_agents::models::RestoreMode::Fresh,
        permission_default: ccbr_agents::models::PermissionMode::Manual,
        queue_policy: ccbr_agents::models::QueuePolicy::SerialPerAgent,
        workspace_path: None,
        workspace_group: None,
        provider_command_template: None,
        model: None,
        startup_args: Vec::new(),
        env: std::collections::HashMap::new(),
        api: ccbr_agents::models::AgentApiSpec::default(),
        provider_profile: ccbr_provider_profiles::ProviderProfileSpec::default(),
        branch_template: None,
        labels: Vec::new(),
        description: None,
        role: None,
        watch_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_dry_run_apply_payload_matches_python_reload_shape() {
        let result = AdditiveReloadApplyResult {
            status: "blocked".into(),
            stage: "plan".into(),
            plan_class: Some("remove_agent".into()),
            old_graph_version: Some("old".into()),
            target_graph_version: None,
            published_graph_version: None,
            old_config_signature: Some("old-sig".into()),
            new_config_signature: Some("new-sig".into()),
            plan: Some(
                json!({
                    "future_safe_to_apply": false,
                    "operations": [{"op": "remove_agent", "agent": "agent2"}],
                    "drain_intents": [],
                    "namespace_patch_plan": {"status": "blocked"},
                    "reasons": ["agent removed"],
                    "warnings": ["busy"],
                })
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            ),
            namespace_patch: None,
            runtime_mount: None,
            publish_transaction: None,
            diagnostics: [
                ("reason".to_string(), json!("agent_busy")),
                ("message".to_string(), json!("agent2 is busy")),
            ]
            .into_iter()
            .collect(),
        };

        let payload = apply_reload_payload(result);

        assert_eq!(payload["dry_run"], false);
        assert_eq!(payload["mutation_enabled"], false);
        assert_eq!(payload["safe_to_apply"], false);
        assert_eq!(payload["future_safe_to_apply"], false);
        assert_eq!(payload["operations"][0]["op"], "remove_agent");
        assert_eq!(payload["namespace_patch_plan"]["status"], "blocked");
        assert_eq!(payload["errors"], json!(["agent_busy: agent2 is busy"]));
    }

    #[test]
    fn published_reload_payload_is_marked_mutating_without_errors() {
        let result = AdditiveReloadApplyResult {
            status: "published".into(),
            stage: "publish_transaction".into(),
            plan_class: Some("view_only_change".into()),
            old_graph_version: Some("old".into()),
            target_graph_version: Some("new".into()),
            published_graph_version: Some("new".into()),
            old_config_signature: Some("sig".into()),
            new_config_signature: Some("sig".into()),
            plan: Some(
                json!({
                    "future_safe_to_apply": true,
                    "operations": [],
                    "drain_intents": [],
                    "namespace_patch_plan": {"status": "planned"},
                    "reasons": [],
                    "warnings": [],
                })
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            ),
            namespace_patch: None,
            runtime_mount: None,
            publish_transaction: None,
            diagnostics: [("graph_published".to_string(), json!(true))]
                .into_iter()
                .collect(),
        };

        let payload = apply_reload_payload(result);

        assert_eq!(payload["status"], "published");
        assert_eq!(payload["dry_run"], false);
        assert_eq!(payload["mutation_enabled"], true);
        assert_eq!(payload["safe_to_apply"], true);
        assert_eq!(payload["errors"], json!([]));
        assert_eq!(payload["diagnostics"]["sidebar_refresh_signal_sent"], false);
    }
}
