use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::models::lifecycle::CcbdLifecycle;

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

    let lifecycle = app.lifecycle.load();
    let startup_summary = build_startup_summary(&lifecycle, app.last_startup_report.as_ref());
    let shutdown_summary = app
        .last_shutdown_report
        .as_ref()
        .map(|r| r.summary_fields())
        .unwrap_or_else(|| json!(null));

    Ok(json!({
        "ticked": true,
        "agents": agent_names,
        "startup_stage": lifecycle.as_ref().and_then(|l| l.startup_stage.clone()),
        "startup_status": lifecycle_status(&lifecycle),
        "startup_summary": startup_summary,
        "shutdown_summary": shutdown_summary,
    }))
}

fn lifecycle_status(lifecycle: &Option<CcbdLifecycle>) -> String {
    lifecycle
        .as_ref()
        .map(|l| l.phase.clone())
        .unwrap_or_else(|| "unknown".into())
}

fn build_startup_summary(
    lifecycle: &Option<CcbdLifecycle>,
    report: Option<&crate::models::lifecycle::CcbdStartupReport>,
) -> serde_json::Value {
    let stage = lifecycle
        .as_ref()
        .and_then(|l| l.startup_stage.clone())
        .unwrap_or_default();
    let status = lifecycle
        .as_ref()
        .map(|l| l.phase.clone())
        .unwrap_or_else(|| "unknown".into());
    let mut summary = json!({
        "startup_stage": stage,
        "startup_status": status,
    });
    if let Some(r) = report {
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("trigger".into(), json!(r.trigger.clone()));
            obj.insert("status".into(), json!(r.status.clone()));
            obj.insert("actions_taken".into(), json!(r.actions_taken.clone()));
            obj.insert(
                "agent_results".into(),
                json!(r
                    .agent_results
                    .iter()
                    .map(|a| a.to_record())
                    .collect::<Vec<_>>()),
            );
            obj.insert("failure_reason".into(), json!(r.failure_reason.clone()));
        }
    }
    summary
}
