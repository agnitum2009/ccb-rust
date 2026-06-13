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
        vec!["default".to_string()]
    } else {
        agent_names
    };

    let restore = bool_field(payload, "restore", true);
    let auto_permission = bool_field(payload, "auto_permission", true);
    let _terminal_size = terminal_size_from_payload(payload);

    let result = app.run_start_flow(&agent_names, restore, auto_permission)?;
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
