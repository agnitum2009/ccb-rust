use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_project_view(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let schema_version = payload
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    let ns = app.project_namespace.load();
    // agent -> window name mapping. The sidebar groups agents under windows by
    // the `window` field (Python `_agent_view` returns `'window': window_name`),
    // so each agent object must carry its window name.
    let mut agent_window: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Some(n) = ns.as_ref() {
        for w in &n.windows {
            for a in &w.agents {
                agent_window.insert(a.clone(), w.name.clone());
            }
        }
    }
    let agents: Vec<Value> = app
        .registry
        .all_entries()
        .iter()
        .filter(|e| e.state != "stopped")
        .map(|e| {
            let is_active = matches!(e.state.as_str(), "busy" | "running" | "active");
            let (activity_state, activity_symbol, activity_color) = if is_active {
                ("active", "●", "green")
            } else {
                ("idle", "○", "blue")
            };
            json!({
                "name": e.agent_name,
                "provider": e.provider,
                "window": agent_window.get(&e.agent_name).cloned().unwrap_or_default(),
                "order": 0,
                "pane_id": e.pane_id,
                "active": is_active,
                "queue_depth": 0,
                "state": e.state,
                "health": e.health,
                "activity_state": activity_state,
                "activity_symbol": activity_symbol,
                "activity_color": activity_color,
            })
        })
        .collect();

    let namespace_mounted = ns.is_some();
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

    // Python wire shape: build_response returns {view, cache, schema_version}
    // with agents/windows/comms nested under `view`. The sidebar reads
    // response["view"]["agents"] etc., so the view payload must be wrapped.
    // Build basic comms from dispatcher's latest jobs per agent (Python _comms_view port).
    let comms: Vec<Value> = app
        .registry
        .all_entries()
        .iter()
        .filter_map(|e| {
            let job = app.dispatcher.latest_for_agent(&e.agent_name)?;
            let body = job.request.body.as_str();
            let preview: String = body.chars().take(80).collect();
            Some(json!({
                "id": job.job_id,
                "from_actor": job.request.from_actor,
                "to_agent": job.agent_name,
                "message_type": job.request.message_type,
                "body_preview": preview,
                "status": format!("{:?}", job.status).to_lowercase(),
                "created_at": job.created_at,
            }))
        })
        .collect();

    let generated_at = chrono::Utc::now().to_rfc3339();
    Ok(json!({
        "schema_version": schema_version,
        "view": {
            "generated_at": generated_at,
            "namespace": {
                "mounted": namespace_mounted,
                "project_root": app.project_root,
                "project_id": app.project_id(),
                "project_slug": app.layout.project_slug(),
                "daemon_status": if app.is_shutdown_requested() { "stopping" } else { "running" },
            },
            "windows": windows.unwrap_or_default(),
            "agents": agents,
            "comms": comms,
        },
        "cache": {
            "generated_at": generated_at,
            "ttl_ms": 0,
            "sequence": 0,
        },
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
