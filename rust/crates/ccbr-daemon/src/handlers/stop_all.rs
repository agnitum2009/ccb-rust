use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::handlers::bool_field;

pub fn handle_stop_all(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let forced = bool_field(payload, "force", false);
    let result = app.stop_all(forced, "stop_all");
    Ok(json!({
        "status": result.status,
        "forced": result.forced,
        "trigger": "stop_all",
        "reason": "stop_all",
        "stopped_agents": result.stopped_agents,
        "actions_taken": result.actions_taken,
        "cleanup_summaries": result.cleanup_summaries,
    }))
}
