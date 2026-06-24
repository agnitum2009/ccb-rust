use serde_json::{json, Value};

use crate::app::CcbdApp;

/// Handle a manual maintenance tick by invoking the daemon heartbeat.
pub fn handle_maintenance_tick(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let _force = payload
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let agent_names: Vec<String> = app
        .registry
        .all_entries()
        .iter()
        .map(|e| e.agent_name.clone())
        .collect();

    app.heartbeat();

    Ok(json!({
        "ticked": true,
        "agents": agent_names,
    }))
}
