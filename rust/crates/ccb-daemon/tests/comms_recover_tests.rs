//! Parity tests for `JobDispatcher::comms_recover`, mirroring Python
//! `test/test_ccbd_comms_recover.py`.
//!
//! Slice 1 covers the noop paths (tests 1 & 5): a RUNNING job whose agent
//! runtime is healthy, with no hint or an unrecognized hint, is not recoverable.
//! Later slices add terminal-retry, stale-running cancel+retry, and
//! reply-delivery recovery.

use ccb_daemon::models::api_models::common::{DeliveryScope, JobStatus};
use ccb_daemon::models::api_models::messages::MessageEnvelope;
use ccb_daemon::services::dispatcher::JobDispatcher;
use serde_json::json;

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
