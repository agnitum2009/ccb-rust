use serde_json::Value;

use crate::app::CcbdApp;

pub fn handle_queue(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all")
        .trim();
    if target.is_empty() {
        return Err("queue requires target".into());
    }
    let detail = payload.get("detail").and_then(|v| v.as_bool());
    let mut summary = app.mailbox_control.queue_summary(target, detail);

    // Enrich mailbox state with the dispatcher's view of the active job.
    if target == "all" {
        if let Some(agents) = summary.get_mut("agents").and_then(|v| v.as_array_mut()) {
            for agent in agents {
                if let Some(obj) = agent.as_object_mut() {
                    if let Some(name) = obj.get("agent_name").and_then(|v| v.as_str()) {
                        if let Some(job_id) = app.dispatcher.state.active_job(name) {
                            obj.insert("active_job_id".to_string(), job_id.into());
                        }
                    }
                }
            }
        }
    } else if let Some(agent) = summary.get_mut("agent").and_then(|v| v.as_object_mut()) {
        if let Some(job_id) = app.dispatcher.state.active_job(target) {
            agent.insert("active_job_id".to_string(), job_id.into());
        }
    }

    Ok(summary)
}
