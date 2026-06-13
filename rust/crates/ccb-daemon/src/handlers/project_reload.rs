use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_reload(_app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let dry_run = payload
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(json!({
        "status": "ok",
        "dry_run": dry_run,
        "plan_class": if dry_run { "dry_run" } else { "applied" },
        "added_agents": [],
        "removed_agents": [],
        "modified_agents": [],
        "unchanged_agents": [],
    }))
}
