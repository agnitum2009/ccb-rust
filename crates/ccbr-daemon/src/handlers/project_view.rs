use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_project_view(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let schema_version = payload
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    let ns = app.project_namespace.load();
    let agents: Vec<Value> = app
        .registry
        .all_entries()
        .iter()
        .filter(|e| e.state != "stopped")
        .map(|e| {
            json!({
                "name": e.agent_name,
                "state": e.state,
                "health": e.health,
                "pane_id": e.pane_id,
                "provider": e.provider,
            })
        })
        .collect();

    let windows = ns.map(|n| {
        n.windows
            .iter()
            .map(|w| {
                json!({
                    "name": w.name,
                    "window_id": w.window_id,
                    "agents": w.agents,
                })
            })
            .collect::<Vec<_>>()
    });

    Ok(json!({
        "schema_version": schema_version,
        "project_root": app.project_root,
        "project_slug": app.layout.project_slug(),
        "project_id": app.project_id(),
        "agents": agents,
        "daemon_status": if app.is_shutdown_requested() { "stopping" } else { "running" },
        "windows": windows.unwrap_or_default(),
        "comms": [],
    }))
}

pub fn handle_project_view_dismiss_comms(
    _app: &mut CcbdApp,
    payload: &Value,
) -> Result<Value, String> {
    let comms_id = payload
        .get("id")
        .or_else(|| payload.get("comms_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    Ok(json!({
        "status": "dismissed",
        "id": comms_id,
        "dismissed_count": 0,
    }))
}
