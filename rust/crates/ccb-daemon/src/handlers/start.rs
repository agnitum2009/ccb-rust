use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::handlers::bool_field;

pub fn handle_start(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_names: Vec<String> = payload
        .get("agent_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let agent_names = if agent_names.is_empty() {
        // Derive agent names from config windows
        derive_agent_names_from_config(app)
    } else {
        agent_names
    };

    let restore = bool_field(payload, "restore", true);
    let auto_permission = bool_field(payload, "auto_permission", true);
    let _terminal_size = terminal_size_from_payload(payload);

    // Get config windows to pass to start_flow
    let config_windows = app.current_config.as_ref().and_then(|c| c.windows.clone());

    let result = app.run_start_flow(&agent_names, restore, auto_permission, config_windows)?;
    Ok(json!({
        "status": result.status,
        "agent_results": result.agent_results,
        "actions_taken": result.actions_taken,
    }))
}

fn terminal_size_from_payload(payload: &Value) -> Option<(u32, u32)> {
    let width = payload.get("terminal_width")?.as_u64()? as u32;
    let height = payload.get("terminal_height")?.as_u64()? as u32;
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

/// Derive agent names from project config windows topology.
/// Returns all unique agent names referenced in the windows config.
fn derive_agent_names_from_config(app: &CcbdApp) -> Vec<String> {
    let Some(config) = &app.current_config else {
        // No config available, fall back to default
        return vec!["default".to_string()];
    };

    let Some(windows) = &config.windows else {
        // No windows configured, use default_agents or all agents
        return if config.default_agents.is_empty() {
            config.agents.keys().cloned().collect()
        } else {
            config.default_agents.clone()
        };
    };

    // Collect all unique agent names from all windows
    let mut agent_names = std::collections::HashSet::new();
    for window in windows {
        for name in &window.agent_names {
            agent_names.insert(name.clone());
        }
    }

    if agent_names.is_empty() {
        // Fallback to default_agents if no agents found in windows
        return if config.default_agents.is_empty() {
            config.agents.keys().cloned().collect()
        } else {
            config.default_agents.clone()
        };
    }

    // Convert to sorted Vec for deterministic ordering
    let mut sorted: Vec<_> = agent_names.into_iter().collect();
    sorted.sort();
    sorted
}
