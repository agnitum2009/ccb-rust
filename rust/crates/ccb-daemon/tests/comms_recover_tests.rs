//! Parity tests for `JobDispatcher::comms_recover`, mirroring Python
//! `test/test_ccbd_comms_recover.py`.
//!
//! Slice 1 covers the noop paths (tests 1 & 5): a RUNNING job whose agent
//! runtime is healthy, with no hint or an unrecognized hint, is not recoverable.
//! Later slices add terminal-retry, stale-running cancel+retry, and
//! reply-delivery recovery.

use std::sync::Arc;

use camino::Utf8PathBuf;
use ccb_daemon::models::api_models::common::{DeliveryScope, JobStatus};
use ccb_daemon::models::api_models::messages::MessageEnvelope;
use ccb_daemon::services::dispatcher::JobDispatcher;
use ccb_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccb_mailbox::stores::{AttemptStore, InboundEventStore};
use ccb_storage::paths::PathLayout;
use serde_json::json;
use tempfile::TempDir;

fn envelope(to_agent: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj-test".to_string(),
        to_agent: to_agent.to_string(),
        from_actor: "user".to_string(),
        body: "work".to_string(),
        task_id: None,
        reply_to: None,
        message_type: "ask".to_string(),
        delivery_scope: DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    }
}

/// Build a dispatcher with healthy agent runtimes registered (mirrors Python
/// `_dispatcher` which seeds `registry.upsert(_runtime(...))` with healthy
/// defaults: state=IDLE, health='healthy', pane_state='alive').
fn healthy_dispatcher() -> JobDispatcher {
    let mut dispatcher = JobDispatcher::new(
        ["agent1", "agent2", "agent3"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    for agent in ["agent1", "agent2", "agent3"] {
        dispatcher.set_runtime_state(agent, "idle", "healthy", "alive");
    }
    dispatcher
}

/// Mirrors `test_comms_recover_does_not_cancel_healthy_running_job`.
#[test]
fn comms_recover_does_not_cancel_healthy_running_job() {
    let mut dispatcher = healthy_dispatcher();
    let receipt = dispatcher.submit(&envelope("agent1"), "codex", None);
    let job_id = receipt.jobs[0].job_id.clone();
    dispatcher.tick();

    let payload = dispatcher.comms_recover(&json!({ "job_id": job_id }));

    assert_eq!(payload["status"].as_str(), Some("noop"));
    assert_eq!(payload["noop_reason"].as_str(), Some("not_recoverable"));
    assert_eq!(dispatcher.get(&job_id).unwrap().status, JobStatus::Running);
}

/// Mirrors `test_comms_recover_rejects_unknown_running_hint`.
#[test]
fn comms_recover_rejects_unknown_running_hint() {
    let mut dispatcher = healthy_dispatcher();
    let receipt = dispatcher.submit(&envelope("agent1"), "codex", None);
    let job_id = receipt.jobs[0].job_id.clone();
    dispatcher.tick();

    let payload = dispatcher.comms_recover(&json!({
        "job_id": job_id,
        "block_reason": "provider_idle_untrusted"
    }));

    assert_eq!(payload["status"].as_str(), Some("noop"));
    assert_eq!(payload["noop_reason"].as_str(), Some("not_recoverable"));
    assert_eq!(dispatcher.get(&job_id).unwrap().status, JobStatus::Running);
}

fn dispatcher_with_attempts() -> (JobDispatcher, PathLayout, TempDir) {
    let dir = TempDir::new().unwrap();
    let layout = PathLayout::new(Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap());
    let attempt_store = AttemptStore::new(&layout);
    let mut dispatcher = JobDispatcher::new(
        ["agent1", "agent2", "agent3"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
    .with_attempt_store(attempt_store);
    for agent in ["agent1", "agent2", "agent3"] {
        dispatcher.set_runtime_state(agent, "idle", "healthy", "alive");
    }
    (dispatcher, layout, dir)
}

/// Mirrors `test_comms_recover_cancels_stale_running_and_starts_waiting_job`.
#[test]
fn comms_recover_cancels_stale_running_and_starts_waiting_job() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_attempts();
    let stuck = dispatcher.submit(&envelope("agent1"), "codex", None).jobs[0]
        .job_id
        .clone();
    let waiting_1 = dispatcher.submit(&envelope("agent1"), "codex", None).jobs[0]
        .job_id
        .clone();
    let waiting_2 = dispatcher.submit(&envelope("agent1"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick(); // stuck → running; waiting_1/2 queued

    dispatcher.set_runtime_state("agent1", "degraded", "pane-dead", "dead");
    let payload = dispatcher.comms_recover(&json!({ "job_id": &stuck }));

    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_eq!(payload["block_reason"].as_str(), Some("pane_dead"));
    assert_eq!(
        payload["cancelled_old"]["job_id"].as_str(),
        Some(stuck.as_str())
    );
    assert_eq!(
        payload["retried_job"]["agent_name"].as_str(),
        Some("agent1")
    );
    let next_started: Vec<&str> = payload["next_started"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["job_id"].as_str().unwrap())
        .collect();
    assert_eq!(next_started, vec![waiting_1.as_str()]);
    assert_eq!(dispatcher.get(&stuck).unwrap().status, JobStatus::Cancelled);
    assert_eq!(
        dispatcher.get(&waiting_1).unwrap().status,
        JobStatus::Running
    );
    assert_eq!(
        dispatcher.get(&waiting_2).unwrap().status,
        JobStatus::Queued
    );

    // retry_index bumped to 1 in the attempt lineage.
    let attempt_store = AttemptStore::new(&layout);
    let message_id = attempt_store
        .get_latest_by_job_id(&stuck)
        .unwrap()
        .message_id;
    let max_retry = attempt_store
        .list_message(&message_id)
        .into_iter()
        .filter(|a| a.agent_name == "agent1")
        .map(|a| a.retry_index)
        .max()
        .unwrap();
    assert_eq!(max_retry, 1);
}

/// Mirrors `test_comms_recover_is_idempotent_after_retry`.
#[test]
fn comms_recover_is_idempotent_after_retry() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_attempts();
    let job_id = dispatcher.submit(&envelope("agent1"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.set_runtime_state("agent1", "degraded", "pane-dead", "dead");

    let first = dispatcher.comms_recover(&json!({ "job_id": &job_id }));
    let second = dispatcher.comms_recover(&json!({ "job_id": &job_id }));

    assert_eq!(
        first["retried_job"]["job_id"].as_str(),
        second["latest_job_id"].as_str(),
    );
    assert_eq!(second["status"].as_str(), Some("noop"));
    assert_eq!(second["noop_reason"].as_str(), Some("already_retried"));

    // The lineage now holds two distinct job ids (original + retry).
    let attempt_store = AttemptStore::new(&layout);
    let message_id = attempt_store
        .get_latest_by_job_id(&job_id)
        .unwrap()
        .message_id;
    let distinct_jobs: std::collections::HashSet<String> = attempt_store
        .list_message(&message_id)
        .into_iter()
        .map(|a| a.job_id)
        .collect();
    assert_eq!(distinct_jobs.len(), 2);
}

/// Mirrors `test_comms_recover_accepts_provider_prompt_idle_hint_for_running_job`.
#[test]
fn comms_recover_accepts_provider_prompt_idle_hint() {
    let (mut dispatcher, layout, _dir) = dispatcher_with_attempts();
    let stuck = dispatcher.submit(&envelope("agent3"), "codex", None).jobs[0]
        .job_id
        .clone();
    let waiting = dispatcher.submit(&envelope("agent3"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick();

    let payload = dispatcher.comms_recover(&json!({
        "job_id": &stuck,
        "block_reason": "provider_prompt_idle"
    }));
    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_eq!(
        payload["block_reason"].as_str(),
        Some("provider_prompt_idle")
    );
    assert_eq!(
        payload["cancelled_old"]["job_id"].as_str(),
        Some(stuck.as_str())
    );
    assert_eq!(
        payload["retried_job"]["agent_name"].as_str(),
        Some("agent3")
    );
    let next_started: Vec<&str> = payload["next_started"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["job_id"].as_str().unwrap())
        .collect();
    assert_eq!(next_started, vec![waiting.as_str()]);
    assert_eq!(dispatcher.get(&stuck).unwrap().status, JobStatus::Cancelled);

    let attempt_store = AttemptStore::new(&layout);
    let message_id = attempt_store
        .get_latest_by_job_id(&stuck)
        .unwrap()
        .message_id;
    let max_retry = attempt_store
        .list_message(&message_id)
        .into_iter()
        .filter(|a| a.agent_name == "agent3")
        .map(|a| a.retry_index)
        .max()
        .unwrap();
    assert_eq!(max_retry, 1);
}

/// Mirrors `test_comms_recover_accepts_provider_prompt_idle_stale_hint_for_running_job`.
#[test]
fn comms_recover_accepts_provider_prompt_idle_stale_hint() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_attempts();
    let stuck = dispatcher.submit(&envelope("agent3"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick();

    let payload = dispatcher.comms_recover(&json!({
        "job_id": &stuck,
        "block_reason": "provider_prompt_idle_stale"
    }));
    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_eq!(
        payload["block_reason"].as_str(),
        Some("provider_prompt_idle_stale")
    );
    assert_eq!(
        payload["cancelled_old"]["job_id"].as_str(),
        Some(stuck.as_str())
    );
    assert_eq!(
        payload["retried_job"]["agent_name"].as_str(),
        Some("agent3")
    );
    assert_eq!(dispatcher.get(&stuck).unwrap().status, JobStatus::Cancelled);
}

/// Mirrors `test_comms_recover_accepts_provider_prompt_input_stuck_hint_for_running_job`.
#[test]
fn comms_recover_accepts_provider_prompt_input_stuck_hint() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_attempts();
    let stuck = dispatcher.submit(&envelope("agent3"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick();

    let payload = dispatcher.comms_recover(&json!({
        "job_id": &stuck,
        "block_reason": "provider_prompt_input_stuck"
    }));
    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_eq!(
        payload["block_reason"].as_str(),
        Some("provider_prompt_input_stuck")
    );
    assert_eq!(
        payload["cancelled_old"]["job_id"].as_str(),
        Some(stuck.as_str())
    );
    assert_eq!(
        payload["retried_job"]["agent_name"].as_str(),
        Some("agent3")
    );
    assert_eq!(dispatcher.get(&stuck).unwrap().status, JobStatus::Cancelled);
}

fn envelope_ask(from_actor: &str, to_agent: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj-test".to_string(),
        to_agent: to_agent.to_string(),
        from_actor: from_actor.to_string(),
        body: "work".to_string(),
        task_id: None,
        reply_to: None,
        message_type: "ask".to_string(),
        delivery_scope: DeliveryScope::Single,
        silence_on_success: false,
        route_options: json!({}),
        body_artifact: None,
    }
}

fn reply_delivery_job_id(d: &JobDispatcher) -> String {
    d.job_store
        .iter()
        .find(|j| j.request.message_type == "reply_delivery")
        .expect("reply_delivery job exists")
        .job_id
        .clone()
}

/// Mirrors `test_comms_recover_reply_delivery_race_is_noop_after_delivery_completes`.
#[test]
fn comms_recover_reply_delivery_race_is_noop_after_delivery_completes() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_attempts();
    let source = dispatcher
        .submit(&envelope_ask("agent2", "agent1"), "codex", None)
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&source, JobStatus::Completed, "OK");
    dispatcher.tick();
    let delivery = reply_delivery_job_id(&dispatcher);
    dispatcher.complete(&delivery, JobStatus::Completed, "delivered");

    let payload = dispatcher.comms_recover(&json!({
        "job_id": &source,
        "reply_delivery_job_id": &delivery
    }));
    assert_eq!(payload["status"].as_str(), Some("noop"));
    assert_eq!(payload["noop_reason"].as_str(), Some("not_recoverable"));
    let delivery_count = dispatcher
        .job_store
        .iter()
        .filter(|j| j.request.message_type == "reply_delivery")
        .count();
    assert_eq!(delivery_count, 1);
}

/// Mirrors `test_comms_recover_failed_reply_delivery_resets_reply_head_and_schedules_delivery`.
#[test]
fn comms_recover_failed_reply_delivery_schedules_new_delivery() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_attempts();
    let source = dispatcher
        .submit(&envelope_ask("agent2", "agent1"), "codex", None)
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&source, JobStatus::Completed, "OK");
    dispatcher.tick();
    let delivery = reply_delivery_job_id(&dispatcher);
    dispatcher.complete(&delivery, JobStatus::Failed, "pane_dead");

    let payload = dispatcher.comms_recover(&json!({
        "job_id": &source,
        "reply_delivery_job_id": &delivery
    }));
    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_ne!(
        payload["retried_job"]["job_id"].as_str(),
        Some(delivery.as_str())
    );
    assert_eq!(
        payload["retried_job"]["request"]["message_type"].as_str(),
        Some("reply_delivery")
    );
    assert_eq!(
        payload["next_started"][0]["job_id"].as_str(),
        payload["retried_job"]["job_id"].as_str()
    );
    assert_eq!(
        payload["recoverability_after"]["recoverable"].as_bool(),
        Some(false)
    );
}

/// Mirrors `test_comms_recover_failed_reply_delivery_is_idempotent_after_new_delivery_starts`.
#[test]
fn comms_recover_failed_reply_delivery_is_idempotent() {
    let (mut dispatcher, _layout, _dir) = dispatcher_with_attempts();
    let source = dispatcher
        .submit(&envelope_ask("agent2", "agent1"), "codex", None)
        .jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&source, JobStatus::Completed, "OK");
    dispatcher.tick();
    let delivery = reply_delivery_job_id(&dispatcher);
    dispatcher.complete(&delivery, JobStatus::Failed, "pane_dead");

    let first = dispatcher.comms_recover(&json!({
        "job_id": &source,
        "reply_delivery_job_id": &delivery
    }));
    let second = dispatcher.comms_recover(&json!({
        "job_id": &source,
        "reply_delivery_job_id": &delivery
    }));
    assert_eq!(
        first["retried_job"]["job_id"].as_str(),
        second["latest_job_id"].as_str(),
    );
    assert_eq!(second["status"].as_str(), Some("noop"));
    assert_eq!(second["noop_reason"].as_str(), Some("already_retried"));
    let delivery_count = dispatcher
        .job_store
        .iter()
        .filter(|j| j.request.message_type == "reply_delivery")
        .count();
    assert_eq!(delivery_count, 2);
}

/// Mirrors `test_comms_recover_releases_only_targeted_mailbox_head`.
#[test]
fn comms_recover_releases_only_targeted_mailbox_head() {
    let dir = TempDir::new().unwrap();
    let layout = PathLayout::new(Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap());
    let facade = MessageBureauFacade::new(
        layout.clone(),
        None,
        Arc::new(|| "2026-01-01T00:00:00Z".to_string()),
    );
    let control = MessageBureauControlService::from_facade(&facade, None, None, None);
    let mut dispatcher =
        JobDispatcher::new(["agent1", "agent2"].iter().map(|s| s.to_string()).collect())
            .with_mailbox(facade)
            .with_mailbox_control(control);
    dispatcher.set_runtime_state("agent1", "idle", "healthy", "alive");
    dispatcher.set_runtime_state("agent2", "idle", "healthy", "alive");

    let source = dispatcher.submit(&envelope("agent1"), "codex", None).jobs[0]
        .job_id
        .clone();
    dispatcher.tick();
    dispatcher.complete(&source, JobStatus::Incomplete, "manual_fail");
    // An unrelated job for another agent must be unaffected by recovery.
    let unrelated = dispatcher.submit(&envelope("agent2"), "codex", None).jobs[0]
        .job_id
        .clone();

    // The source's task_request inbound is agent1's mailbox head (file-backed
    // on the shared layout, so fresh store instances read it).
    let attempt = AttemptStore::new(&layout)
        .get_latest_by_job_id(&source)
        .expect("attempt recorded");
    let inbound = InboundEventStore::new(&layout)
        .get_latest_for_attempt("agent1", &attempt.attempt_id)
        .expect("inbound recorded");

    let payload = dispatcher.comms_recover(&json!({ "job_id": &source }));

    assert_eq!(payload["status"].as_str(), Some("recovered"));
    assert_eq!(
        payload["released_event"]["inbound_event_id"].as_str(),
        Some(inbound.inbound_event_id.as_str())
    );
    assert!(payload["retried_job"]["job_id"].as_str().is_some());
    // The unrelated agent's job is untouched by the targeted recovery (not
    // cancelled/terminated). (Python asserts Accepted; the Rust tick promotes
    // it to Running — both are non-terminal / unaffected.)
    assert!(
        !dispatcher.get(&unrelated).unwrap().status.is_terminal(),
        "unrelated job should not be terminated by recovery"
    );
}
