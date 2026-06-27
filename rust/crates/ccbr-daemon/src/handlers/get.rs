use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::models::api_models::records::JobRecord;

pub fn handle_get(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let job_id = payload.get("job_id").and_then(|v| v.as_str());
    let agent_name = payload.get("agent_name").and_then(|v| v.as_str());
    let generation = app.health_monitor.daemon_health().generation;

    match (job_id, agent_name) {
        (Some(jid), _) => app
            .dispatcher
            .get(jid)
            .map(|job| append_generation(build_result_payload(job), generation))
            .ok_or_else(|| "job not found".to_string()),
        (_, Some(agent)) => app
            .dispatcher
            .latest_for_agent(agent)
            .map(|job| append_generation(build_result_payload(job), generation))
            .ok_or_else(|| "job not found".to_string()),
        _ => Err("get requires job_id or agent_name".into()),
    }
}

fn build_result_payload(job: &JobRecord) -> Value {
    let terminal = job.terminal_decision.as_ref();
    let reply = terminal
        .and_then(|decision| decision.get("reply"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let completion_reason = terminal
        .and_then(|decision| decision.get("reason"))
        .cloned()
        .unwrap_or(Value::Null);
    let completion_confidence = terminal
        .and_then(|decision| decision.get("confidence"))
        .cloned()
        .unwrap_or(Value::Null);
    let visible_reply_source = if terminal.is_some() {
        "job_terminal_decision"
    } else {
        "none"
    };
    json!({
        "job_id": job.job_id,
        "agent_name": job.agent_name,
        "target_kind": job.target_kind,
        "target_name": job.target_name,
        "provider_instance": Value::Null,
        "provider": job.provider,
        "status": job.status,
        "job": job.to_record(),
        "snapshot": Value::Null,
        "reply": reply,
        "completion_reason": completion_reason,
        "completion_confidence": completion_confidence,
        "updated_at": job.updated_at,
        "visible_reply_source": visible_reply_source,
        "visible_reply_id": Value::Null,
        "message_id": Value::Null,
    })
}

fn append_generation(mut payload: Value, generation: u32) -> Value {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("generation".to_string(), json!(generation));
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api_models::common::JobStatus;
    use crate::models::api_models::messages::MessageEnvelope;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn app_with_job() -> (CcbdApp, String) {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let receipt = app.dispatcher.submit(
            &MessageEnvelope {
                project_id: "proj-1".into(),
                to_agent: "agent1".into(),
                from_actor: "user".into(),
                body: "hello".into(),
                task_id: None,
                reply_to: None,
                message_type: "ask".into(),
                delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
                silence_on_success: false,
                route_options: json!({}),
                body_artifact: None,
            },
            "codex",
            None,
        );
        let job_id = receipt.unwrap().jobs[0].job_id.clone();
        app.dispatcher.update_job_status(
            &job_id,
            JobStatus::Completed,
            Some(json!({
                "terminal": true,
                "status": "completed",
                "reason": "done",
                "confidence": "high",
                "reply": "answer",
            })),
        );
        (app, job_id)
    }

    #[test]
    fn get_returns_python_result_payload_shape() {
        let (mut app, job_id) = app_with_job();

        let result = handle_get(&mut app, &json!({"job_id": job_id})).unwrap();

        assert_eq!(result["status"], "completed");
        assert_eq!(result["reply"], "answer");
        assert_eq!(result["completion_reason"], "done");
        assert_eq!(result["job"]["record_type"], "job_record");
        assert!(result.get("snapshot").is_some());
        assert_eq!(result["visible_reply_source"], "job_terminal_decision");
        assert!(result["visible_reply_id"].is_null());
        assert_eq!(result["generation"], 1);
    }

    #[test]
    fn get_unknown_job_fails_like_python_handler() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );

        let err = handle_get(&mut app, &json!({"job_id": "job_missing"})).unwrap_err();

        assert!(err.contains("job not found"));
    }
}
