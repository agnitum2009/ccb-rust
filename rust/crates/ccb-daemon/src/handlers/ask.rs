use std::path::Path;

use ccb_completion::models::JobRecord;
use ccb_providers::execution::ProviderRuntimeContext;
use ccb_terminal::TerminalBackend;
use serde_json::{json, Value};

use crate::adapters::mailbox::{to_mailbox_envelope, to_mailbox_job_record};
use crate::app::CcbdApp;
use crate::handlers::str_field;
use crate::models::api_models::messages::MessageEnvelope;
use crate::provider_launcher::default_session_path;

pub fn handle_ask(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let project_id = payload
        .get("project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let to_agent = payload
        .get("to_agent")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let from_actor = payload
        .get("from_actor")
        .and_then(|v| v.as_str())
        .unwrap_or("user")
        .to_string();
    let body = payload
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let envelope = MessageEnvelope {
        project_id: project_id.clone(),
        to_agent: to_agent.clone(),
        from_actor: from_actor.clone(),
        body: body.clone(),
        task_id: str_field(payload, "task_id"),
        reply_to: str_field(payload, "reply_to"),
        message_type: payload
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("ask")
            .to_string(),
        delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    };
    envelope.validate()?;

    let (pane_id, provider, workspace_path) = app
        .registry
        .get(&to_agent)
        .map(|entry| {
            (
                entry.pane_id.clone(),
                entry.provider.clone(),
                entry.workspace_path.clone(),
            )
        })
        .ok_or_else(|| format!("agent {to_agent} has no active pane"))?;
    let pane_id = pane_id.ok_or_else(|| format!("agent {to_agent} has no active pane"))?;

    // Surface a clear auth error before trying to send, so a missing auth.json
    // fails fast instead of silently hanging in the provider CLI.
    if let Some(auth_path) =
        crate::provider_launcher::provider_runtime_auth_path(&provider, app.project_root.to_string_lossy().as_ref(), &to_agent)
    {
        if !auth_path.exists() {
            let utf8_auth = camino::Utf8Path::from_path(&auth_path)
                .unwrap_or_else(|| camino::Utf8Path::new("/tmp/unknown"));
            let detail = ccb_provider_profiles::format_codex_auth_missing_error(utf8_auth);
            return Err(format!(
                "agent {to_agent} ({provider}) cannot ask: {detail}",
            ));
        }
    }

    // Record the ask in the dispatcher for tracking.
    let workspace = workspace_path.as_deref().unwrap_or("");
    let receipt = app.dispatcher.submit(&envelope, &provider, Some(workspace));
    let job_id = receipt
        .jobs
        .first()
        .map(|job| job.job_id.clone())
        .unwrap_or_default();

    // Persist the submission in the mailbox layer.
    if let Some(job) = app.dispatcher.get(&job_id) {
        let mailbox_job = to_mailbox_job_record(job);
        app.mailbox.record_submission(
            &to_mailbox_envelope(&envelope),
            &[mailbox_job],
            None,
            &receipt.accepted_at,
            None,
        );
    }

    // Start execution tracking through the provider adapter so the adapter can
    // prepare a provider-specific prompt (e.g. wrapping with CCB_REQ_ID).
    let mut prompt_text = body.clone();
    if !job_id.is_empty() && !provider.is_empty() {
        let completion_job = JobRecord::new(&job_id, &to_agent, &provider)
            .with_request_body(&body)
            .with_request_message_type(&envelope.message_type);
        let workspace = workspace_path.as_deref().map(Path::new);
        let session_ref = workspace.and_then(|ws| {
            default_session_path(&provider, &to_agent, &app.project_root, ws)
                .map(|p| p.to_string_lossy().to_string())
        });
        let runtime_context = ProviderRuntimeContext {
            agent_name: to_agent.clone(),
            workspace_path: workspace_path.clone(),
            backend_type: Some("tmux".to_string()),
            runtime_ref: Some(pane_id.clone()),
            session_ref,
            runtime_pid: None,
            runtime_health: None,
            runtime_binding_source: None,
        };
        if let Some(submission) = app.execution.start(&completion_job, Some(&runtime_context)) {
            if let Some(Value::String(adapter_prompt)) = submission.runtime_state.get("prompt_text")
            {
                if !adapter_prompt.is_empty() {
                    prompt_text = adapter_prompt.clone();
                }
            }
        }
        app.dispatcher.mark_running(&job_id);
        if let Some(job) = app.dispatcher.get(&job_id) {
            let mailbox_job = to_mailbox_job_record(job);
            let started_at = chrono::Utc::now().to_rfc3339();
            app.mailbox.mark_attempt_started(&mailbox_job, &started_at);
        }
    }

    let socket_path = app
        .project_namespace
        .load()
        .map(|ns| ns.tmux_socket_path.clone());
    let delivered = if let Some(socket) = socket_path {
        let backend = ccb_terminal::TmuxBackend::new(None, Some(socket));
        backend
            .send_text(&pane_id, &prompt_text)
            .map_err(|e| format!("failed to send message to pane: {e}"))?;
        true
    } else {
        false
    };

    if delivered {
        app.execution.feed_runtime_state(
            &job_id,
            [("prompt_sent".to_string(), Value::Bool(true))]
                .into_iter()
                .collect(),
        );
    }

    Ok(json!({
        "status": "ok",
        "delivered": delivered,
        "to_agent": to_agent,
        "from_actor": from_actor,
        "pane_id": pane_id,
        "job_id": job_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CcbdApp;
    use crate::services::registry::AgentRuntimeEntry;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_ask_fails_when_agent_has_no_pane() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: None,
            workspace_path: Some(dir.path().to_string_lossy().to_string()),
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let payload = json!({
            "project_id": "proj-1",
            "to_agent": "claude",
            "from_actor": "user",
            "body": "hello",
        });
        let result = handle_ask(&mut app, &payload);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no active pane"));
    }

    #[test]
    fn test_ask_fails_fast_on_missing_auth_json() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "codex".to_string(),
            provider: "codex".to_string(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%1".to_string()),
            workspace_path: Some(dir.path().to_string_lossy().to_string()),
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let payload = json!({
            "project_id": "proj-1",
            "to_agent": "codex",
            "from_actor": "user",
            "body": "hello",
        });
        let result = handle_ask(&mut app, &payload);
        assert!(result.is_err(), "ask should fail when auth.json is missing");
        let err = result.unwrap_err();
        assert!(
            err.contains("no Codex credentials were found") && err.contains("auth.json"),
            "error should surface codex auth path and hint, got: {err}"
        );
    }
}
