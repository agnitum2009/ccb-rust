use serde_json::{json, Value};

use crate::adapters::mailbox::to_mailbox_job_record;
use crate::app::CcbdApp;
use crate::models::api_models::common::DeliveryScope;
use crate::models::api_models::messages::MessageEnvelope;
use crate::models::api_models::records::JobRecord;

use ccbr_mailbox::models::{AttemptRecord, AttemptState};
use std::collections::HashMap;

pub fn handle_retry(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let target = payload
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if target.is_empty() {
        return Err("retry requires target".into());
    }
    retry_attempt(app, target)
}

fn retry_attempt(app: &mut CcbdApp, target: &str) -> Result<Value, String> {
    let accepted_at = chrono::Utc::now().to_rfc3339();
    let original_attempt = resolve_retry_attempt(app, target)?;
    if !is_terminal_attempt(original_attempt.attempt_state) {
        return Err(format!(
            "attempt is still active: {}",
            original_attempt.attempt_id
        ));
    }
    if original_attempt.attempt_state == AttemptState::Completed {
        return Err(format!(
            "retry is not allowed for completed attempts: {}",
            original_attempt.attempt_id
        ));
    }

    let latest_attempts = latest_attempts_by_agent(
        app.mailbox_control
            .attempt_store()
            .list_message(&original_attempt.message_id),
    );
    let latest_attempt = latest_attempts
        .get(&original_attempt.agent_name)
        .ok_or_else(|| {
            format!(
                "message is missing attempt lineage for agent: {}",
                original_attempt.agent_name
            )
        })?;
    if latest_attempt.attempt_id != original_attempt.attempt_id {
        return Err(format!(
            "retry requires latest attempt for agent: {}",
            original_attempt.agent_name
        ));
    }

    let message = app
        .mailbox_control
        .message_store()
        .get_latest(&original_attempt.message_id)
        .ok_or_else(|| {
            format!(
                "message not found for attempt: {}",
                original_attempt.attempt_id
            )
        })?;
    app.registry
        .get(&original_attempt.agent_name)
        .ok_or_else(|| format!("agent not registered: {}", original_attempt.agent_name))?;
    let current = app
        .dispatcher
        .get(&original_attempt.job_id)
        .cloned()
        .ok_or_else(|| format!("job not found for attempt: {}", original_attempt.attempt_id))?;

    let retry_request = retry_request_for_job(&current);
    let (job, receipt) = app.dispatcher.enqueue_planned_job(
        retry_request,
        current.provider.clone(),
        current.workspace_path.clone(),
        message.submission_id.clone(),
        &accepted_at,
    );
    let attempt_id = app
        .mailbox
        .record_retry_attempt(
            &original_attempt.message_id,
            &to_mailbox_job_record(&job),
            &accepted_at,
        )
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "accepted_at": accepted_at,
        "target": target,
        "message_id": original_attempt.message_id,
        "original_attempt_id": original_attempt.attempt_id,
        "attempt_id": attempt_id,
        "job_id": receipt.job_id,
        "agent_name": receipt.agent_name,
        "status": receipt.status,
    }))
}

fn resolve_retry_attempt(app: &CcbdApp, target: &str) -> Result<AttemptRecord, String> {
    if let Some(attempt) = app.mailbox_control.attempt_store().get_latest(target) {
        return Ok(attempt);
    }
    if let Some(attempt) = app
        .mailbox_control
        .attempt_store()
        .get_latest_by_job_id(target)
    {
        return Ok(attempt);
    }
    Err(format!("retry target not found: {target}"))
}

fn latest_attempts_by_agent(attempts: Vec<AttemptRecord>) -> HashMap<String, AttemptRecord> {
    let mut by_attempt_id: HashMap<String, AttemptRecord> = HashMap::new();
    for attempt in attempts {
        by_attempt_id.insert(attempt.attempt_id.clone(), attempt);
    }
    let mut by_agent = HashMap::new();
    for attempt in by_attempt_id.into_values() {
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

fn retry_request_for_job(job: &JobRecord) -> MessageEnvelope {
    let mut request = job.request.clone();
    request.to_agent = job.agent_name.clone();
    request.delivery_scope = DeliveryScope::Single;
    if should_retry_with_continue(job) {
        request.body = "continue".to_string();
    }
    request
}

fn should_retry_with_continue(job: &JobRecord) -> bool {
    if !job.request.message_type.trim().eq_ignore_ascii_case("ask") {
        return false;
    }
    let Some(terminal) = job.terminal_decision.as_ref() else {
        return false;
    };
    terminal
        .get("anchor_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || terminal
            .get("reply_started")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::submit::handle_submit;
    use crate::models::api_models::common::JobStatus as DaemonJobStatus;
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
            DaemonJobStatus::Failed,
            Some(json!({
                "terminal": true,
                "status": "failed",
                "reason": "failed",
                "confidence": "low",
                "reply": "",
                "anchor_seen": true,
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
        let attempt_id = app
            .mailbox_control
            .attempt_store()
            .get_latest_by_job_id(&job_id)
            .unwrap()
            .attempt_id;
        (job_id, attempt_id)
    }

    #[test]
    fn retry_recreates_attempt_with_python_payload_shape() {
        let dir = TempDir::new().unwrap();
        let mut app = app_with_agent(&dir);
        let (_job_id, attempt_id) = submit_terminal_failed(&mut app);

        let result = handle_retry(&mut app, &json!({"target": attempt_id})).unwrap();

        assert_eq!(result["original_attempt_id"], attempt_id);
        assert!(result["attempt_id"].as_str().unwrap().starts_with("att_"));
        assert!(result["job_id"].as_str().unwrap().starts_with("job_"));
        assert_eq!(result["agent_name"], "agent1");
        assert_eq!(result["status"], "accepted");
        let new_job = app
            .dispatcher
            .get(result["job_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(new_job.request.body, "continue");
    }

    #[test]
    fn retry_missing_target_fails_like_python_handler() {
        let dir = TempDir::new().unwrap();
        let mut app = app_with_agent(&dir);

        let err = handle_retry(&mut app, &json!({})).unwrap_err();

        assert!(err.contains("retry requires target"));
    }

    #[test]
    fn retry_active_attempt_is_rejected() {
        let dir = TempDir::new().unwrap();
        let mut app = app_with_agent(&dir);
        let receipt = handle_submit(
            &mut app,
            &json!({
                "project_id": "proj-1",
                "to_agent": "agent1",
                "from_actor": "user",
                "body": "hello",
            }),
        )
        .unwrap();
        let job_id = receipt["job_id"].as_str().unwrap();
        let attempt_id = app
            .mailbox_control
            .attempt_store()
            .get_latest_by_job_id(job_id)
            .unwrap()
            .attempt_id;

        let err = handle_retry(&mut app, &json!({"target": attempt_id})).unwrap_err();

        assert!(err.contains("attempt is still active"));
    }
}
