use std::sync::Arc;

use ccb_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccb_mailbox::facade_recording::CompletionDecision;
use ccb_mailbox::models::{DeliveryScope, JobRecord, JobStatus, MessageEnvelope, TargetKind};
use ccb_mailbox::{InboundEventStore, MailboxStore};
use ccb_storage::paths::PathLayout;
use serde_json::Value;

fn fixed_clock() -> Arc<dyn Fn() -> String + Send + Sync> {
    Arc::new(|| "2025-01-01T00:00:00Z".to_string())
}

fn make_envelope(from_actor: &str, to_agent: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "p1".into(),
        to_agent: to_agent.into(),
        from_actor: from_actor.into(),
        body: "hello".into(),
        task_id: None,
        reply_to: None,
        message_type: "task_request".into(),
        delivery_scope: DeliveryScope::Agent,
        silence_on_success: false,
        route_options: Value::Object(Default::default()),
        body_artifact: None,
    }
}

fn make_job(job_id: &str, agent_name: &str) -> JobRecord {
    JobRecord {
        job_id: job_id.into(),
        submission_id: None,
        agent_name: agent_name.into(),
        provider: "claude".into(),
        request: make_envelope("user", agent_name),
        status: JobStatus::Accepted,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
        workspace_path: None,
        target_kind: TargetKind::Agent,
        target_name: agent_name.into(),
        provider_instance: None,
        provider_options: Value::Object(Default::default()),
    }
}

fn config_with_agent(agent_name: &str) -> Value {
    serde_json::json!({
        "agents": { agent_name: {} }
    })
}

#[test]
fn test_full_message_lifecycle() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("claude");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout.clone(), Some(config.clone()), Arc::clone(&clock));
    let control = MessageBureauControlService::new(layout, Some(config), Some(Arc::clone(&clock)));

    // Submit a message for claude.
    let jobs = vec![make_job("job_1", "claude")];
    let message_id = facade
        .record_submission(
            &make_envelope("user", "claude"),
            &jobs,
            Some("sub_1"),
            &clock(),
            None,
        )
        .unwrap();

    // Queue summary should show one queued item.
    let summary = control.queue_summary("all", None);
    assert_eq!(
        summary.get("total_queue_depth").and_then(|v| v.as_u64()),
        Some(1)
    );

    // Agent queue detail.
    let agent = control.agent_queue("claude");
    assert_eq!(agent.get("queue_depth").and_then(|v| v.as_u64()), Some(1));

    // Claimable job id.
    let claimable = facade.claimable_request_job_ids("claude");
    assert_eq!(claimable, vec!["job_1"]);

    // Mark attempt started.
    facade.mark_attempt_started(&jobs[0], &clock());

    // Mark the job completed and record a terminal outcome.
    let mut completed_job = jobs[0].clone();
    completed_job.status = JobStatus::Completed;
    let decision = CompletionDecision::completed("done");
    let reply_id = facade
        .record_terminal(&completed_job, &decision, &clock(), true, true)
        .unwrap();
    assert!(!reply_id.is_empty());

    // Message state should be completed.
    let message = facade.get_message(&message_id).unwrap();
    assert!(matches!(
        message.message_state,
        ccb_mailbox::models::MessageState::Completed
    ));

    // Inbox detail should now be empty.
    let inbox = control.inbox("claude", Some(true));
    assert_eq!(inbox.get("item_count").and_then(|v| v.as_u64()), Some(0));
}

#[test]
fn test_trace_message() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("claude");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout.clone(), Some(config.clone()), Arc::clone(&clock));
    let control = MessageBureauControlService::new(layout, Some(config), Some(Arc::clone(&clock)));

    let jobs = vec![make_job("job_2", "claude")];
    let message_id = facade
        .record_submission(
            &make_envelope("user", "claude"),
            &jobs,
            Some("sub_2"),
            &clock(),
            None,
        )
        .unwrap();

    let trace = control.trace(&message_id);
    assert_eq!(
        trace.get("resolved_kind").and_then(|v| v.as_str()),
        Some("message")
    );
    assert_eq!(
        trace.get("message_id").and_then(|v| v.as_str()),
        Some(message_id.as_str())
    );
}

#[test]
fn test_ack_reply_flow() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("claude");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout.clone(), Some(config.clone()), Arc::clone(&clock));
    let control = MessageBureauControlService::new(layout, Some(config), Some(Arc::clone(&clock)));

    // Submit and complete a job so a reply delivery event is queued for the caller.
    let mut job = make_job("job_3", "claude");
    job.request.from_actor = "claude".into(); // self-reply for simplicity
    let jobs = vec![job.clone()];
    let message_id = facade
        .record_submission(&job.request, &jobs, Some("sub_3"), &clock(), None)
        .unwrap();

    facade.mark_attempt_started(&job, &clock());

    let decision = CompletionDecision::completed("reply body");
    facade.record_terminal(&job, &decision, &clock(), true, true);

    // After terminal, the original task request is consumed, leaving the reply delivery event.
    let inbox = control.inbox("claude", Some(true));
    assert_eq!(inbox.get("item_count").and_then(|v| v.as_u64()), Some(1));

    // Ack the reply.
    let ack = control.ack_reply("claude", None);
    assert_eq!(
        ack.get("acknowledged_event_type").and_then(|v| v.as_str()),
        Some("task_reply")
    );

    // Inbox should now be empty.
    let inbox = control.inbox("claude", Some(true));
    assert_eq!(inbox.get("item_count").and_then(|v| v.as_u64()), Some(0));

    // No further events remain.
    assert_eq!(ack.get("next_event_type").and_then(|v| v.as_str()), None);

    // Trace the message.
    let trace = control.trace(&message_id);
    assert_eq!(trace.get("reply_count").and_then(|v| v.as_u64()), Some(1));
}

#[test]
fn test_notice_and_cancelled_terminal() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("claude");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout, Some(config), Arc::clone(&clock));

    let mut job = make_job("job_4", "claude");
    job.status = JobStatus::Cancelled;
    let jobs = vec![job.clone()];
    let message_id = facade
        .record_submission(&job.request, &jobs, None, &clock(), None)
        .unwrap();

    facade.mark_attempt_started(&job, &clock());

    // Terminal notice updates attempt state and message state to cancelled.
    let decision = CompletionDecision {
        terminal: true,
        status: JobStatus::Cancelled,
        reason: Some("cancelled".into()),
        reply: "cancelled notice".into(),
        provider_turn_ref: None,
        diagnostics: serde_json::json!({}),
    };
    let notice_id = facade
        .record_terminal(&job, &decision, &clock(), false, true)
        .unwrap();
    assert!(!notice_id.is_empty());

    let message = facade.get_message(&message_id).unwrap();
    assert!(matches!(
        message.message_state,
        ccb_mailbox::models::MessageState::Cancelled
    ));
}

/// Mirrors Python `test_message_bureau_submission_fastpath.py::test_record_retry_attempt_does_not_refresh_mailbox`.
#[test]
fn test_record_retry_attempt_increments_queue_without_refreshing_mailbox() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("agent1");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout.clone(), Some(config.clone()), Arc::clone(&clock));
    let job = make_job("job_1", "agent1");
    let jobs = vec![job.clone()];
    let message_id = facade
        .record_submission(&job.request, &jobs, None, &clock(), None)
        .unwrap();

    let retry_job = make_job("job_2", "agent1");
    facade
        .record_retry_attempt(&message_id, &retry_job, &clock())
        .unwrap();

    let events = InboundEventStore::new(&layout).list_agent("agent1");
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[1].event_type,
        ccb_mailbox::models::InboundEventType::TaskRequest
    );

    let mailbox = MailboxStore::new(&layout).load("agent1").unwrap().unwrap();
    assert_eq!(mailbox.queue_depth, 2);
    assert_eq!(mailbox.pending_reply_count, 0);
    assert!(matches!(
        mailbox.mailbox_state,
        ccb_mailbox::models::MailboxState::Blocked
    ));
}

/// Mirrors Python `test_message_bureau_submission_fastpath.py::test_record_reply_delivery_skips_non_mailbox_caller`.
#[test]
fn test_record_reply_delivery_skips_non_mailbox_caller() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = camino::Utf8Path::from_path(dir.path()).unwrap();
    let layout = PathLayout::new(path);
    let config = config_with_agent("agent1");
    let clock = fixed_clock();

    let facade = MessageBureauFacade::new(layout.clone(), Some(config), Arc::clone(&clock));
    let job = make_job("job_1", "agent1");
    let jobs = vec![job.clone()];
    facade
        .record_submission(&job.request, &jobs, None, &clock(), None)
        .unwrap();

    let mut completed_job = job.clone();
    completed_job.status = JobStatus::Completed;
    let decision = CompletionDecision::completed("done");
    facade.record_reply(&completed_job, &decision, &clock(), true);

    assert!(!layout.ccbd_mailboxes_dir().join("user").exists());
}
