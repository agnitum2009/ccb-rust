use serde_json::{json, Value};

use crate::app::CcbdApp;

pub fn handle_get(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let job_id = payload.get("job_id").and_then(|v| v.as_str());
    let agent_name = payload.get("agent_name").and_then(|v| v.as_str());

    match (job_id, agent_name) {
        (Some(jid), _) => {
            if let Some(job) = app.dispatcher.get(jid) {
                Ok(json!({
                    "job_id": jid,
                    "status": job.status,
                    "reply": "",
                    "agent_name": job.agent_name,
                }))
            } else {
                Ok(json!({
                    "job_id": jid,
                    "status": "unknown",
                    "reply": "",
                }))
            }
        }
        (_, Some(agent)) => {
            if let Some(job) = app.dispatcher.latest_for_agent(agent) {
                Ok(json!({
                    "agent_name": agent,
                    "job_id": job.job_id,
                    "status": job.status,
                    "reply": "",
                }))
            } else {
                Ok(json!({
                    "agent_name": agent,
                    "status": "unknown",
                    "reply": "",
                }))
            }
        }
        _ => Err("get requires job_id or agent_name".into()),
    }
}
