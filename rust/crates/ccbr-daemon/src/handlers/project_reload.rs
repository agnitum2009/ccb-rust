use crate::app::CcbdApp;
use crate::reload_plan::{build_invalid_reload_dry_run_plan, build_reload_dry_run_plan};
use crate::reload_transaction::apply_reload_plan;
use ccbr_agents::config::{load_project_config, project_config_path};
use serde_json::Value;

pub fn handle_project_reload(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let dry_run = payload
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let current_config = app
        .current_config
        .clone()
        .unwrap_or_else(|| current_config_from_registry(app));

    let config_path = project_config_path(&app.layout);
    if !config_path.exists() {
        let error = format!("project config not found: {config_path}");
        let plan = build_invalid_reload_dry_run_plan(&current_config, &error, None);
        return Ok(plan.to_record());
    }

    let new_config = match load_project_config(&app.layout) {
        Ok(result) => result.config,
        Err(e) => {
            let error = format!("{e}");
            let plan = build_invalid_reload_dry_run_plan(&current_config, &error, None);
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

    let result = apply_reload_plan(app, &plan, &new_config);
    if result.status == "ok" {
        app.current_config = Some(new_config);
    }
    Ok(result.to_record())
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
