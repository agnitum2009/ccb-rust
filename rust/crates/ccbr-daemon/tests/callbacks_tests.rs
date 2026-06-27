//! Parity tests for callback subsystem, mirroring Python
//! `test/test_v2_message_bureau_dispatcher_integration.py` callback cases.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use camino::Utf8PathBuf;
use ccbr_completion::models::{CompletionDecision, CompletionStatus};
use ccbr_daemon::models::api_models::common::{DeliveryScope, JobStatus};
use ccbr_daemon::models::api_models::messages::MessageEnvelope;
use ccbr_daemon::services::dispatcher::JobDispatcher;
use ccbr_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccbr_mailbox::models::{
    CallbackEdgeRecord, CallbackEdgeState, MessageState, ReplyRecord, ReplyTerminalStatus,
};
use ccbr_mailbox::stores::{CallbackEdgeStore, MessageStore, ReplyStore};
use ccbr_storage::paths::PathLayout;
use serde_json::json;
use tempfile::TempDir;

fn ask_envelope(to_agent: &str, from_actor: &str, body: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj-test".to_string(),
        to_agent: to_agent.to_string(),
        from_actor: from_actor.to_string(),
        body: body.to_string(),
        task_id: Some("task-callback".to_string()),
        reply_to: None,
        message_type: "ask".to_string(),
        delivery_scope: DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    }
}

fn callback_envelope(to_agent: &str, from_actor: &str, body: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj-test".to_string(),
        to_agent: to_agent.to_string(),
        from_actor: from_actor.to_string(),
        body: body.to_string(),
        task_id: Some("task-callback".to_string()),
        reply_to: None,
        message_type: "ask".to_string(),
        delivery_scope: DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({"mode": "callback"}),
        body_artifact: None,
    }
}

fn decision(reply: &str) -> CompletionDecision {
    CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("task_complete".to_string()),
        confidence: None,
        reply: reply.to_string(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: None,
        finished_at: Some("2026-03-30T00:00:10Z".to_string()),
        diagnostics: Default::default(),
    }
}

fn failed_decision() -> CompletionDecision {
    CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some("api_error".to_string()),
        confidence: None,
        reply: "unauthorized".to_string(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: None,
        finished_at: Some("2026-03-30T00:00:10Z".to_string()),
        diagnostics: json!({
            "error_type": "provider_api_error",
            "error_code": "unauthorized",
            "error_message": "login required",
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    }
}

fn dispatcher_with_mailbox_and_clock(
    clock: Arc<dyn Fn() -> String + Send + Sync>,
) -> (JobDispatcher, PathLayout, TempDir) {
    let dir = TempDir::new().unwrap();
    let layout = PathLayout::new(Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap());
    let config = json!({ "agents": { "codex": {}, "claude": {}, "gemini": {} } });
    let facade = MessageBureauFacade::new(layout.clone(), Some(config.clone()), Arc::clone(&clock));
    let control = MessageBureauControlService::from_facade(&facade, Some(config), None, None);
    let dispatcher = JobDispatcher::new(vec!["codex".into(), "claude".into(), "gemini".into()])
        .with_clock(clock)
        .with_mailbox(facade)
        .with_mailbox_control(control)
        .with_layout(layout.clone());
    (dispatcher, layout, dir)
}

fn dispatcher_with_mailbox() -> (JobDispatcher, PathLayout, TempDir) {
    let clock: Arc<dyn Fn() -> String + Send + Sync> =
        Arc::new(|| "2026-03-30T00:00:00Z".to_string());
    dispatcher_with_mailbox_and_clock(clock)
}

#[test]
fn callback_routes_child_result_as_parent_continuation() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();

    let parent_job_id = dispatcher
        .submit(
            &ask_envelope("codex", "user", "review with help"),
            "codex",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();

    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "collect evidence"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .expect("edge should exist");
    assert_eq!(edge.parent_job_id, parent_job_id);

    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated to claude"));
    let parent_job = dispatcher.get(&parent_job_id).unwrap();
    assert!(parent_job.terminal_decision.as_ref().unwrap()["delegated"]
        .as_bool()
        .unwrap());
    assert_eq!(
        MessageStore::new(&layout)
            .get_latest(&edge.parent_message_id)
            .unwrap()
            .message_state,
        MessageState::Running
    );

    dispatcher.tick();
    dispatcher.complete_with_decision(&child_job_id, &decision("evidence found"));

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .expect("edge still exists");
    assert_eq!(edge.state, CallbackEdgeState::ContinuationSubmitted);
    assert!(edge.continuation_job_id.is_some());

    dispatcher.tick();
    let continuation_job = dispatcher
        .get(edge.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(continuation_job.agent_name, "codex");
    assert_eq!(continuation_job.request.to_agent, "codex");
    assert_eq!(continuation_job.request.from_actor, "user");
    assert_eq!(
        continuation_job.request.task_id.as_deref(),
        Some("task-callback")
    );
    assert_eq!(
        continuation_job.request.reply_to.as_deref(),
        Some(edge.parent_message_id.as_str())
    );
    assert_eq!(
        continuation_job.request.message_type,
        "callback_continuation"
    );
    assert_eq!(
        continuation_job.request.route_options["callback_edge_id"],
        edge.edge_id
    );
    assert_eq!(
        continuation_job.request.route_options["callback_parent_job_id"],
        parent_job_id
    );
    assert_eq!(
        continuation_job.request.route_options["callback_child_job_id"],
        child_job_id
    );
    assert_eq!(
        continuation_job.request.route_options["callback_child_message_id"],
        edge.child_message_id
    );
    assert!(continuation_job.request.body.contains("evidence found"));

    dispatcher.complete_with_decision(
        edge.continuation_job_id.as_ref().unwrap(),
        &decision("final answer"),
    );
    let final_edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    assert_eq!(final_edge.state, CallbackEdgeState::Done);
    assert_eq!(
        MessageStore::new(&layout)
            .get_latest(&edge.parent_message_id)
            .unwrap()
            .message_state,
        MessageState::Completed
    );
    let replies = ReplyStore::new(&layout).list_message(&edge.parent_message_id);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].reply, "final answer");
}

#[test]
fn callback_rejects_without_active_parent() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_mailbox();
    dispatcher.tick();
    let err = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "collect evidence"),
            "claude",
            None,
        )
        .unwrap_err();
    assert!(err.contains("active parent job"));
}

#[test]
fn plain_nested_ask_from_active_parent_is_rejected() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_mailbox();
    dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap();
    dispatcher.tick();
    let err = dispatcher
        .submit(
            &ask_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap_err();
    assert!(err.contains("plain ask from an active CCBR task requires --callback"));
}

#[test]
fn silent_nested_ask_from_active_parent_is_allowed() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap();
    dispatcher.tick();
    let mut env = ask_envelope("claude", "codex", "independent child task");
    env.silence_on_success = true;
    let receipt = dispatcher.submit(&env, "claude", None).unwrap();
    assert_eq!(receipt.jobs[0].agent_name, "claude");
    assert_eq!(receipt.jobs[0].status, JobStatus::Accepted);
    assert!(CallbackEdgeStore::new(&layout).list_all().is_empty());
}

#[test]
fn callback_chain_waits_for_nested_child_message() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let a_job = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let b_job = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "middle task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge_ab = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&b_job)
        .unwrap();
    dispatcher.complete_with_decision(&a_job, &decision("delegated to b"));
    dispatcher.tick();

    let c_job = dispatcher
        .submit(
            &callback_envelope("gemini", "claude", "leaf task"),
            "gemini",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge_bc = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&c_job)
        .unwrap();
    dispatcher.complete_with_decision(&b_job, &decision("delegated to c"));

    let edge_ab = CallbackEdgeStore::new(&layout)
        .get_latest(&edge_ab.edge_id)
        .unwrap();
    assert_eq!(edge_ab.state, CallbackEdgeState::Pending);

    dispatcher.tick();
    dispatcher.complete_with_decision(&c_job, &decision("leaf result"));
    let edge_bc = CallbackEdgeStore::new(&layout)
        .get_latest(&edge_bc.edge_id)
        .unwrap();
    assert_eq!(edge_bc.state, CallbackEdgeState::ContinuationSubmitted);
    assert!(edge_bc.continuation_job_id.is_some());

    dispatcher.tick();
    dispatcher.complete_with_decision(
        edge_bc.continuation_job_id.as_ref().unwrap(),
        &decision("middle final"),
    );
    let edge_bc = CallbackEdgeStore::new(&layout)
        .get_latest(&edge_bc.edge_id)
        .unwrap();
    assert_eq!(edge_bc.state, CallbackEdgeState::Done);
    let edge_ab = CallbackEdgeStore::new(&layout)
        .get_latest(&edge_ab.edge_id)
        .unwrap();
    assert!(edge_ab.continuation_job_id.is_some());

    dispatcher.tick();
    let continuation_a = dispatcher
        .get(edge_ab.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert!(continuation_a.request.body.contains("middle final"));
}

#[test]
fn callback_continuation_uses_artifact_for_large_child_reply() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(
            &ask_envelope("codex", "user", "review with long evidence"),
            "codex",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "collect long evidence"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));
    dispatcher.tick();

    let long_reply = format!("child-start\n{}\nchild-end", "z".repeat(5000));
    dispatcher.complete_with_decision(&child_job_id, &decision(&long_reply));

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    let continuation_job = dispatcher
        .get(edge.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert!(continuation_job.request.body.len() <= 4096);
    assert!(continuation_job
        .request
        .body
        .contains("Full child reply artifact:"));
    assert!(!continuation_job.request.body.contains("child-end"));
    let child_reply = ReplyStore::new(&layout)
        .get_latest(edge.child_reply_id.as_ref().unwrap())
        .unwrap();
    assert!(child_reply.reply_artifact.is_some());
}

#[test]
fn callback_continuation_uses_forced_artifact_for_short_child_reply() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(
            &ask_envelope("codex", "user", "review with file-backed evidence"),
            "codex",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let mut env = callback_envelope("claude", "codex", "collect short evidence");
    env.route_options = json!({"mode": "callback", "artifact_reply": true});
    let child_job_id = dispatcher.submit(&env, "claude", None).unwrap().jobs[0]
        .job_id
        .clone();
    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));
    dispatcher.tick();
    dispatcher.complete_with_decision(&child_job_id, &decision("short child result"));

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    let continuation_job = dispatcher
        .get(edge.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert!(continuation_job
        .request
        .body
        .contains("Full child reply artifact:"));
    assert!(continuation_job.request.body.contains("short child result"));
    let child_reply = ReplyStore::new(&layout)
        .get_latest(edge.child_reply_id.as_ref().unwrap())
        .unwrap();
    assert!(child_reply.reply_artifact.is_some());
}

#[test]
fn callback_child_failure_still_continues_parent() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(
            &ask_envelope("codex", "user", "review with help"),
            "codex",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "collect evidence"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));
    dispatcher.tick();
    dispatcher.complete_with_decision(&child_job_id, &failed_decision());

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    assert_eq!(edge.state, CallbackEdgeState::ContinuationSubmitted);
    let continuation = dispatcher
        .get(edge.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert!(continuation.request.body.contains("Child status: failed"));
}

#[test]
fn callback_rejects_actor_cycle() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_mailbox();
    let a_job = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let b_job = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "middle task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&a_job, &decision("delegated to b"));
    dispatcher.tick();
    let _c_job = dispatcher
        .submit(
            &callback_envelope("gemini", "claude", "leaf task"),
            "gemini",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&b_job, &decision("delegated to c"));
    dispatcher.tick();
    let err = dispatcher
        .submit(
            &callback_envelope("codex", "gemini", "cycle back"),
            "codex",
            None,
        )
        .unwrap_err();
    assert!(err.contains("cycle detected"));
}

#[test]
fn callback_rejects_depth_limit() {
    let (mut dispatcher, _layout, _dir) = {
        let (d, l, t) = dispatcher_with_mailbox();
        (d.with_max_callback_depth(2), l, t)
    };

    let a_job = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let b_job = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "middle task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&a_job, &decision("delegated to b"));
    dispatcher.tick();
    let _c_job = dispatcher
        .submit(
            &callback_envelope("gemini", "claude", "leaf task"),
            "gemini",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&b_job, &decision("delegated to c"));
    dispatcher.tick();
    let err = dispatcher
        .submit(
            &callback_envelope("codex", "gemini", "too deep"),
            "codex",
            None,
        )
        .unwrap_err();
    assert!(err.contains("max callback depth 2"));
}

#[test]
fn callback_timeout_fails_expired_edge() {
    let offset = Arc::new(AtomicU64::new(0));
    let offset_c = Arc::clone(&offset);
    let clock: Arc<dyn Fn() -> String + Send + Sync> = Arc::new(move || {
        let base = chrono::DateTime::parse_from_rfc3339("2026-03-30T00:00:00Z").unwrap();
        let dt = base + chrono::Duration::milliseconds(offset_c.load(Ordering::SeqCst) as i64);
        dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    });
    let (mut dispatcher, layout, _dir) = {
        let (d, l, t) = dispatcher_with_mailbox_and_clock(clock);
        (d.with_callback_timeout_s(0.1), l, t)
    };

    let parent_job_id = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));

    offset.store(200, Ordering::SeqCst);
    dispatcher.tick();

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    assert_eq!(edge.state, CallbackEdgeState::TimedOut);
    assert!(edge.timeout_at.is_none());
    assert!(edge.diagnostics.get("failure_reason").is_some());
}

#[test]
fn callback_repair_submits_missing_continuation() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&parent_job_id, JobStatus::Completed, "delegated");
    let parent_job = dispatcher.get(&parent_job_id).cloned().unwrap();
    let child_job_id = dispatcher
        .submit(
            &ask_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job = dispatcher.get(&child_job_id).cloned().unwrap();
    dispatcher.complete(&child_job.job_id, JobStatus::Completed, "repaired result");

    let parent_attempt = ccbr_mailbox::stores::AttemptStore::new(&layout)
        .get_latest_by_job_id(&parent_job.job_id)
        .unwrap();
    let child_attempt = ccbr_mailbox::stores::AttemptStore::new(&layout)
        .get_latest_by_job_id(&child_job.job_id)
        .unwrap();
    let reply = ReplyRecord {
        reply_id: format!(
            "rep_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        ),
        message_id: child_attempt.message_id.clone(),
        attempt_id: child_attempt.attempt_id.clone(),
        agent_name: child_job.agent_name.clone(),
        terminal_status: ReplyTerminalStatus::Completed,
        reply: "repaired result".to_string(),
        reply_artifact: None,
        diagnostics: serde_json::json!({}),
        finished_at: "2026-03-30T00:00:05Z".to_string(),
    };
    ccbr_mailbox::stores::ReplyStore::new(&layout)
        .append(&reply)
        .unwrap();

    let edge = CallbackEdgeRecord {
        edge_id: format!(
            "cb_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        ),
        parent_job_id: parent_job.job_id.clone(),
        parent_message_id: parent_attempt.message_id.clone(),
        parent_agent: parent_job.agent_name.clone(),
        child_job_id: child_job.job_id.clone(),
        child_message_id: child_attempt.message_id.clone(),
        callback_target_agent: parent_job.agent_name.clone(),
        original_caller: "user".to_string(),
        original_task_id: parent_job.request.task_id.clone(),
        state: CallbackEdgeState::ChildCompleted,
        child_reply_id: None,
        child_status: Some("completed".to_string()),
        continuation_job_id: None,
        continuation_message_id: None,
        timeout_at: Some("2099-01-01T00:00:00Z".to_string()),
        created_at: "2026-03-30T00:00:00Z".to_string(),
        updated_at: "2026-03-30T00:00:00Z".to_string(),
        diagnostics: serde_json::json!({
            "route_mode": "callback",
            "child_agent": child_job.agent_name,
            "parent_body": "root task",
            "child_body": "child task",
        }),
    };
    CallbackEdgeStore::new(&layout).append(&edge).unwrap();

    dispatcher.tick();

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    assert_eq!(edge.state, CallbackEdgeState::ContinuationSubmitted);
    assert!(edge.continuation_job_id.is_some());
    let continuation = dispatcher
        .get(edge.continuation_job_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(continuation.agent_name, "codex");
    assert!(continuation.request.body.contains("repaired result"));
}

#[test]
fn callback_timeout_records_failure_notice_and_fails_parent_message() {
    let offset = Arc::new(AtomicU64::new(0));
    let offset_c = Arc::clone(&offset);
    let clock: Arc<dyn Fn() -> String + Send + Sync> = Arc::new(move || {
        let base = chrono::DateTime::parse_from_rfc3339("2026-03-30T00:00:00Z").unwrap();
        let dt = base + chrono::Duration::milliseconds(offset_c.load(Ordering::SeqCst) as i64);
        dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    });
    let (mut dispatcher, layout, _dir) = {
        let (d, l, t) = dispatcher_with_mailbox_and_clock(clock);
        (d.with_callback_timeout_s(0.1), l, t)
    };

    let parent_job_id = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));

    offset.store(200, Ordering::SeqCst);
    dispatcher.tick();

    let parent_message = MessageStore::new(&layout)
        .get_latest(&edge.parent_message_id)
        .unwrap();
    assert_eq!(parent_message.message_state, MessageState::Failed);
    let replies = ReplyStore::new(&layout).list_message(&edge.parent_message_id);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].terminal_status, ReplyTerminalStatus::Failed);
    assert!(replies[0].diagnostics.get("callback_failure").is_some());
}

#[test]
fn callback_repair_from_pending_edge_with_child_reply() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&parent_job_id, JobStatus::Completed, "delegated");
    let parent_job = dispatcher.get(&parent_job_id).cloned().unwrap();
    let child_job_id = dispatcher
        .submit(
            &ask_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job = dispatcher.get(&child_job_id).cloned().unwrap();
    dispatcher.complete(&child_job.job_id, JobStatus::Completed, "repaired result");

    let parent_attempt = ccbr_mailbox::stores::AttemptStore::new(&layout)
        .get_latest_by_job_id(&parent_job.job_id)
        .unwrap();
    let child_attempt = ccbr_mailbox::stores::AttemptStore::new(&layout)
        .get_latest_by_job_id(&child_job.job_id)
        .unwrap();
    let reply = ReplyRecord {
        reply_id: format!(
            "rep_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        ),
        message_id: child_attempt.message_id.clone(),
        attempt_id: child_attempt.attempt_id.clone(),
        agent_name: child_job.agent_name.clone(),
        terminal_status: ReplyTerminalStatus::Completed,
        reply: "repaired result".to_string(),
        reply_artifact: None,
        diagnostics: serde_json::json!({}),
        finished_at: "2026-03-30T00:00:05Z".to_string(),
    };
    ccbr_mailbox::stores::ReplyStore::new(&layout)
        .append(&reply)
        .unwrap();

    let edge = CallbackEdgeRecord {
        edge_id: format!(
            "cb_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        ),
        parent_job_id: parent_job.job_id.clone(),
        parent_message_id: parent_attempt.message_id.clone(),
        parent_agent: parent_job.agent_name.clone(),
        child_job_id: child_job.job_id.clone(),
        child_message_id: child_attempt.message_id.clone(),
        callback_target_agent: parent_job.agent_name.clone(),
        original_caller: "user".to_string(),
        original_task_id: parent_job.request.task_id.clone(),
        state: CallbackEdgeState::Pending,
        child_reply_id: None,
        child_status: Some("completed".to_string()),
        continuation_job_id: None,
        continuation_message_id: None,
        timeout_at: Some("2099-01-01T00:00:00Z".to_string()),
        created_at: "2026-03-30T00:00:00Z".to_string(),
        updated_at: "2026-03-30T00:00:00Z".to_string(),
        diagnostics: serde_json::json!({
            "route_mode": "callback",
            "child_agent": child_job.agent_name,
            "parent_body": "root task",
            "child_body": "child task",
        }),
    };
    CallbackEdgeStore::new(&layout).append(&edge).unwrap();

    dispatcher.tick();

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    assert_eq!(edge.state, CallbackEdgeState::ContinuationSubmitted);
    assert!(edge.continuation_job_id.is_some());
}

#[test]
fn callback_repair_reuses_existing_continuation_job() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_mailbox();
    let parent_job_id = dispatcher
        .submit(&ask_envelope("codex", "user", "root task"), "codex", None)
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    let child_job_id = dispatcher
        .submit(
            &callback_envelope("claude", "codex", "child task"),
            "claude",
            None,
        )
        .unwrap()
        .jobs[0]
        .job_id
        .clone();
    dispatcher.complete_with_decision(&parent_job_id, &decision("delegated"));
    dispatcher.tick();
    dispatcher.complete_with_decision(&child_job_id, &decision("child result"));

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest_for_child_job(&child_job_id)
        .unwrap();
    let existing_continuation_id = edge.continuation_job_id.clone().unwrap();

    // Simulate crash: roll the edge back to ChildCompleted but leave the
    // continuation job in place.
    let rolled_back = CallbackEdgeRecord {
        state: CallbackEdgeState::ChildCompleted,
        continuation_job_id: None,
        continuation_message_id: None,
        ..edge.clone()
    };
    CallbackEdgeStore::new(&layout)
        .append(&rolled_back)
        .unwrap();

    dispatcher.tick();

    let edge = CallbackEdgeStore::new(&layout)
        .get_latest(&edge.edge_id)
        .unwrap();
    assert_eq!(edge.state, CallbackEdgeState::ContinuationSubmitted);
    assert_eq!(
        edge.continuation_job_id.as_ref().unwrap(),
        &existing_continuation_id
    );
}
