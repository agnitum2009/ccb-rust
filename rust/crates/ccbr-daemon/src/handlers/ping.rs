use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_ping(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("ccbd")
        .trim();

    if target == "ccbd" {
        let health = app.health_monitor.daemon_health();
        return Ok(json!({
            "pong": true,
            "target": "ccbd",
            "status": "ok",
            "health": health.to_record(),
        }));
    }

    let agent_health = app
        .registry
        .get(target)
        .map(|e| json!({"state": e.state, "health": e.health, "pane_id": e.pane_id }));

    Ok(json!({
        "pong": true,
        "target": target,
        "status": "ok",
        "agent_health": agent_health,
    }))
}
