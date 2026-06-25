use serde_json::{json, Value};

use crate::adapters::mailbox::{to_mailbox_envelope, to_mailbox_job_record};
use crate::app::CcbdApp;
use crate::models::api_models::common::DeliveryScope;
use crate::models::api_models::messages::MessageEnvelope;
use crate::models::api_models::receipts::AcceptedJobReceipt;
use crate::models::api_models::records::JobRecord;

use ccbr_mailbox::models::{AttemptRecord, AttemptState, MessageRecord};
use std::collections::HashMap;

pub fn handle_resubmit(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let message_id = payload
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if message_id.is_empty() {
        return Err("resubmit requires message_id".into());
    }
    resubmit_message(app, message_id)
}

fn resubmit_message(app: &mut CcbdApp, message_id: &str) -> Result<Value, String> {
    let accepted_at = chrono::Utc::now().to_rfc3339();
    let message = app
        .mailbox_control
        .message_store()
        .get_latest(message_id)
        .ok_or_else(|| format!("message not found: {message_id}"))?;
    let latest_attempts =
        latest_attempts_by_agent(app.mailbox_control.attempt_store().list_message(message_id));
    if latest_attempts.is_empty() {
        return Err(format!("message has no attempts to resubmit: {message_id}"));
    }

    let mut source_jobs = Vec::new();
    let mut active_agents = Vec::new();
    for agent in &message.target_agents {
        let attempt = latest_attempts
            .get(agent)
            .ok_or_else(|| format!("message is missing attempt lineage for agents: {agent}"))?;
        if !is_terminal_attempt(attempt.attempt_state) {
            active_agents.push(agent.clone());
        }
        let job = app
            .dispatcher
            .get(&attempt.job_id)
            .cloned()
            .ok_or_else(|| format!("job not found for attempt: {}", attempt.attempt_id))?;
        source_jobs.push((agent.clone(), job));
    }
    if !active_agents.is_empty() {
        return Err(format!(
            "message still has active attempts: {}",
            active_agents.join(", ")
        ));
    }

    let request = resubmission_request(&message, &source_jobs)?;
    let submission_id = if request.delivery_scope == DeliveryScope::Broadcast {
        Some(new_id("sub"))
    } else {
        None
    };

    let mut receipts = Vec::new();
    let mut mailbox_jobs = Vec::new();
    for (agent_name, source_job) in source_jobs {
        let registry = app
            .registry
            .get(&agent_name)
            .ok_or_else(|| format!("agent not registered: {agent_name}"))?;
        let per_agent_request = message_for_agent(&request, &agent_name);
        let (job, receipt) = app.dispatcher.enqueue_planned_job(
            per_agent_request,
            registry.provider.clone(),
            registry
                .workspace_path
                .clone()
                .or_else(|| source_job.workspace_path.clone()),
            submission_id.clone(),
            &accepted_at,
        );
        mailbox_jobs.push(to_mailbox_job_record(&job));
        receipts.push(receipt);
    }

    let new_message_id = app
        .mailbox
        .record_submission(
            &to_mailbox_envelope(&request),
            &mailbox_jobs,
            submission_id.as_deref(),
            &accepted_at,
            Some(message_id),
        )
        .ok_or_else(|| "resubmit did not create message".to_string())?;

    Ok(json!({
        "accepted_at": accepted_at,
        "original_message_id": message_id,
        "message_id": new_message_id,
        "submission_id": submission_id,
        "jobs": receipts.iter().map(AcceptedJobReceipt::to_record).collect::<Vec<_>>(),
    }))
}

fn latest_attempts_by_agent(attempts: Vec<AttemptRecord>) -> HashMap<String, AttemptRecord> {
    let mut by_agent = HashMap::new();
    for attempt in attempts {
        let keep = by_agent
            .get(&attempt.agent_name)
            .is_none_or(|current| attempt_sort_key(&attempt) > attempt_sort_key(current));
        if keep {
            by_agent.insert(attempt.agent_name.clone(), attempt);
        }
    }
    by_agent
}

fn attempt_sort_key(attempt: &AttemptRecord) -> (u32, String, String) {
    (
        attempt.retry_index,
        attempt.updated_at.clone(),
        attempt.attempt_id.clone(),
    )
}

fn is_terminal_attempt(state: AttemptState) -> bool {
    matches!(
        state,
        AttemptState::Completed
            | AttemptState::Incomplete
            | AttemptState::Failed
            | AttemptState::Cancelled
            | AttemptState::Superseded
            | AttemptState::DeadLetter
    )
}

fn resubmission_request(
    message: &MessageRecord,
    source_jobs: &[(String, JobRecord)],
) -> Result<MessageEnvelope, String> {
    let first = source_jobs
        .first()
        .map(|(_, job)| job)
        .ok_or_else(|| "no source job for resubmission request".to_string())?;
    let delivery_scope = if message.target_agents.len() > 1 {
        DeliveryScope::Broadcast
    } else {
        DeliveryScope::Single
    };
    let to_agent = if delivery_scope == DeliveryScope::Broadcast {
        "all".to_string()
    } else {
        message
            .target_agents
            .first()
            .cloned()
            .ok_or_else(|| "message has no target agents".to_string())?
    };
    Ok(MessageEnvelope {
        project_id: first.request.project_id.clone(),
        to_agent,
        from_actor: message.from_actor.clone(),
        body: first.request.body.clone(),
        task_id: first.request.task_id.clone(),
        reply_to: first.request.reply_to.clone(),
        message_type: first.request.message_type.clone(),
        delivery_scope,
        silence_on_success: first.request.silence_on_success,
        route_options: first.request.route_options.clone(),
        body_artifact: first.request.body_artifact.clone(),
    })
}

fn message_for_agent(request: &MessageEnvelope, agent_name: &str) -> MessageEnvelope {
    let mut per_agent = request.clone();
    per_agent.to_agent = agent_name.to_string();
    per_agent.delivery_scope = DeliveryScope::Single;
    per_agent
}

fn new_id(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::submit::handle_submit;
    use crate::models::api_models::common::JobStatus;
    use crate::services::registry::AgentRuntimeEntry;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn app_with_agent(dir: &TempDir) -> CcbdApp {
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent1".to_string(),
            provider: "codex".to_string(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%1".into()),
            workspace_path: Some(dir.path().to_string_lossy().to_string()),
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        app
    }

    fn submit_terminal_failed(app: &mut CcbdApp) -> (String, String) {
        let receipt = handle_submit(
            app,
            &json!({
                "project_id": "proj-1",
                "to_agent": "agent1",
                "from_actor": "user",
                "body": "hello",
            }),
        )
        .unwrap();
        let job_id = receipt["job_id"].as_str().unwrap().to_string();
        app.dispatcher.update_job_status(
            &job_id,
            JobStatus::Failed,
            Some(json!({
                "terminal": true,
                "status": "failed",
                "reason": "failed",
                "confidence": "low",
                "reply": "",
            })),
        );
        let job = app.dispatcher.get(&job_id).unwrap();
        let mailbox_job = to_mailbox_job_record(job);
        let decision = ccbr_mailbox::facade_recording::CompletionDecision {
            terminal: true,
            status: ccbr_mailbox::models::JobStatus::Failed,
            reason: Some("failed".into()),
            reply: "".into(),
            provider_turn_ref: None,
            diagnostics: Value::Object(Default::default()),
        };
        app.mailbox
            .record_terminal(&mailbox_job, &decision, "2026-06-26T00:00:00Z", true, true);
        let message_id = app
            .mailbox_control
            .attempt_store()
            .get_latest_by_job_id(&job_id)
            .unwrap()
            .message_id;
        (job_id, message_id)
    }

    #[test]
    fn resubmit_recreates_message_with_python_payload_shape() {
        let dir = TempDir::new().unwrap();
        let mut app = app_with_agent(&dir);
        let (_job_id, message_id) = submit_terminal_failed(&mut app);

        let result = handle_resubmit(&mut app, &json!({"message_id": message_id})).unwrap();

        assert_eq!(result["original_message_id"], message_id);
        assert!(result["message_id"].as_str().unwrap().starts_with("msg_"));
        assert!(result["submission_id"].is_null());
        assert_eq!(result["jobs"].as_array().unwrap().len(), 1);
        assert_eq!(result["jobs"][0]["agent_name"], "agent1");
    }

    #[test]
    fn resubmit_missing_message_fails_like_python_handler() {
        let dir = TempDir::new().unwrap();
        let mut app = app_with_agent(&dir);

        let err = handle_resubmit(&mut app, &json!({"message_id": "msg_missing"})).unwrap_err();

        assert!(err.contains("message not found"));
    }
}
