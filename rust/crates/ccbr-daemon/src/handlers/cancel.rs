use serde_json::Value;

use crate::adapters::mailbox::to_mailbox_job_record;
use crate::app::CcbdApp;

fn active_pane_for_job(app: &CcbdApp, job_id: &str) -> Option<(String, String)> {
    let contexts = app.execution.active_contexts();
    let (_, ctx) = contexts.iter().find(|(id, _)| id == job_id)?;
    let pane_id = ctx.runtime_ref.clone()?;
    let socket_path = app
        .project_namespace
        .load()
        .map(|ns| ns.tmux_socket_path.clone())?;
    Some((pane_id, socket_path))
}

pub fn handle_cancel(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let job_id = payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if job_id.is_empty() {
        return Err("cancel requires job_id".into());
    }

    // Try to interrupt the provider CLI in its tmux pane before tearing down
    // the execution tracker, so a true mid-run ask can be cancelled.
    if let Some((pane_id, socket_path)) = active_pane_for_job(app, job_id) {
        let backend = ccbr_terminal::TmuxBackend::new(None, Some(socket_path));
        if let Err(e) = backend.tmux_run(
            &["send-keys", "-t", &pane_id, "C-c"],
            false,
            false,
            None,
            None,
        ) {
            eprintln!("ccbrd: failed to send Ctrl-C to pane {pane_id}: {e}");
        }
    }

    app.execution.cancel(job_id);
    let receipt = app.dispatcher.cancel(job_id)?;

    // Keep the mailbox layer consistent with the dispatcher: record a terminal
    // outcome for the cancelled job.
    if let Some(job) = app.dispatcher.get(job_id) {
        let mailbox_job = to_mailbox_job_record(job);
        let decision = ccbr_mailbox::facade_recording::CompletionDecision {
            terminal: true,
            status: ccbr_mailbox::models::JobStatus::Cancelled,
            reason: Some("cancelled".into()),
            reply: "".into(),
            provider_turn_ref: None,
            diagnostics: Value::Object(Default::default()),
        };
        let _ = app.mailbox.record_terminal(
            &mailbox_job,
            &decision,
            &receipt.cancelled_at,
            true,
            true,
        );
    }

    Ok(receipt.to_record())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api_models::common::JobStatus;
    use crate::services::registry::AgentRuntimeEntry;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_cancel_missing_job_id_fails() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let result = handle_cancel(&mut app, &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("job_id"));
    }

    #[test]
    fn test_cancel_terminal_job_errors() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "claude".into(),
            provider: "claude".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%1".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        let receipt = app.dispatcher.submit(
            &crate::models::api_models::messages::MessageEnvelope {
                project_id: "proj-1".into(),
                to_agent: "claude".into(),
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
            "claude",
            None,
        );
        let job_id = receipt.jobs[0].job_id.clone();
        app.dispatcher.tick();
        app.dispatcher.update_job_status(&job_id, JobStatus::Completed, None);

        let result = handle_cancel(&mut app, &json!({"job_id": job_id}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already terminal"));
    }

    #[test]
    fn test_cancel_running_job_terminalizes_immediately() {
        let dir = TempDir::new().unwrap();
        let ccbr_dir = dir.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr_dir).unwrap();
        std::fs::write(
            ccbr_dir.join("ccbr.config"),
            r#"version = 2
default_agents = ["claude"]

[agents.claude]
provider = "claude"
target = "claude"
"#,
        )
        .unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "claude".into(),
            provider: "claude".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%1".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });
        let receipt = app.dispatcher.submit(
            &crate::models::api_models::messages::MessageEnvelope {
                project_id: "proj-1".into(),
                to_agent: "claude".into(),
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
            "claude",
            None,
        );
        let job_id = receipt.jobs[0].job_id.clone();
        app.dispatcher.tick();

        let result = handle_cancel(&mut app, &json!({"job_id": job_id})).unwrap();
        assert_eq!(result["status"], "cancelled");

        let job = app.dispatcher.get(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.terminal_decision.is_some());
    }
}
