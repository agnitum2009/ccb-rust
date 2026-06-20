//! Mirrors Python `test/test_v2_api_models.py`.

use ccb_daemon::models::api_models::common::{DeliveryScope, JobStatus, TargetKind};
use ccb_daemon::models::api_models::messages::MessageEnvelope;
use ccb_daemon::models::api_models::records::{JobEvent, JobRecord, SubmissionRecord};

fn envelope(delivery_scope: DeliveryScope, to_agent: &str, from_actor: &str) -> MessageEnvelope {
    MessageEnvelope {
        project_id: "proj".into(),
        to_agent: to_agent.into(),
        from_actor: from_actor.into(),
        body: "hello".into(),
        task_id: None,
        reply_to: None,
        message_type: "ask".into(),
        delivery_scope,
        silence_on_success: false,
        route_options: serde_json::Value::Object(Default::default()),
        body_artifact: None,
    }
}

#[test]
fn test_message_envelope_validates_delivery_scope() {
    let mut env = envelope(DeliveryScope::Single, "all", "user");
    env.normalize().unwrap();
    assert!(env.validate().is_err());
}

#[test]
fn test_message_envelope_normalizes_agent_names_and_system_sender() {
    let mut env = envelope(DeliveryScope::Single, "Agent1", "System");
    env.normalize().unwrap();
    assert_eq!(env.to_agent, "agent1");
    assert_eq!(env.from_actor, "system");
}

#[test]
fn test_message_envelope_round_trips_body_artifact() {
    let artifact = serde_json::json!({
        "path": "/tmp/body.txt",
        "bytes": 5000,
        "sha256": "abc",
    });
    let mut env = envelope(DeliveryScope::Single, "Agent1", "System");
    env.body_artifact = Some(artifact.clone());
    env.normalize().unwrap();

    let record = env.to_record();
    assert_eq!(record["body_artifact"], artifact);
    assert_eq!(env.body_artifact, Some(artifact));
}

#[test]
fn test_message_envelope_preserves_non_agent_actors() {
    let mut email = envelope(DeliveryScope::Single, "Agent1", "Email");
    email.normalize().unwrap();
    assert_eq!(email.from_actor, "email");

    let mut user = envelope(DeliveryScope::Single, "Agent1", "user");
    user.normalize().unwrap();
    assert_eq!(user.from_actor, "user");
}

#[test]
fn test_message_envelope_rejects_invalid_sender() {
    let mut env = envelope(DeliveryScope::Single, "agent1", "invalid sender");
    assert!(env.normalize().is_err());
}

#[test]
fn test_job_record_requires_terminal_decision_for_terminal_state() {
    let req = envelope(DeliveryScope::Single, "agent1", "user");
    let job = JobRecord {
        job_id: "job-1".into(),
        submission_id: None,
        agent_name: "agent1".into(),
        provider: "codex".into(),
        request: req,
        status: JobStatus::Completed,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:01Z".into(),
        workspace_path: None,
        target_kind: TargetKind::Agent,
        target_name: "agent1".into(),
    };
    assert!(job.validate().is_err());
}

#[test]
fn test_records_include_schema_version() {
    let req = envelope(DeliveryScope::Single, "agent1", "user");
    let job = JobRecord {
        job_id: "job-1".into(),
        submission_id: None,
        agent_name: "agent1".into(),
        provider: "codex".into(),
        request: req.clone(),
        status: JobStatus::Running,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:01Z".into(),
        workspace_path: None,
        target_kind: TargetKind::Agent,
        target_name: "agent1".into(),
    };
    let submission = SubmissionRecord {
        submission_id: "sub-1".into(),
        project_id: "proj".into(),
        from_actor: "system".into(),
        target_scope: "all".into(),
        task_id: Some("task-1".into()),
        job_ids: vec!["job-1".into()],
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:01Z".into(),
    };
    let event = JobEvent {
        event_id: "evt-1".into(),
        job_id: "job-1".into(),
        agent_name: "agent1".into(),
        event_type: "job_started".into(),
        payload: serde_json::json!({"status": "running"}),
        timestamp: "2026-03-18T00:00:00Z".into(),
        target_kind: TargetKind::Agent,
        target_name: "agent1".into(),
    };

    assert_eq!(job.to_record()["schema_version"], 2);
    assert_eq!(submission.to_record()["schema_version"], 2);
    assert_eq!(event.to_record()["schema_version"], 2);
}

#[test]
fn test_submission_record_preserves_user_sender() {
    let mut submission = SubmissionRecord {
        submission_id: "sub-1".into(),
        project_id: "proj".into(),
        from_actor: "USER".into(),
        target_scope: "single".into(),
        task_id: None,
        job_ids: vec!["job-1".into()],
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:01Z".into(),
    };
    submission.normalize().unwrap();
    assert_eq!(submission.from_actor, "user");
}

#[test]
fn test_job_record_normalizes_agent_target_identity() {
    let req = envelope(DeliveryScope::Single, "agent1", "user");
    let mut job = JobRecord {
        job_id: "job-agent-1".into(),
        submission_id: None,
        agent_name: "Agent1".into(),
        provider: "codex".into(),
        request: req,
        status: JobStatus::Accepted,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2026-03-18T00:00:00Z".into(),
        updated_at: "2026-03-18T00:00:01Z".into(),
        workspace_path: None,
        target_kind: TargetKind::Agent,
        target_name: "Agent1".into(),
    };
    job.normalize();
    let record = job.to_record();
    assert!(matches!(job.target_kind, TargetKind::Agent));
    assert_eq!(job.target_name, "agent1");
    assert_eq!(job.agent_name, "agent1");
    assert_eq!(record["target_kind"], "agent");
    assert_eq!(record["target_name"], "agent1");
    assert_eq!(record["provider_instance"], serde_json::Value::Null);
}

#[test]
fn test_job_event_normalizes_agent_target_identity() {
    let mut event = JobEvent {
        event_id: "evt-agent-1".into(),
        job_id: "job-agent-1".into(),
        agent_name: "Agent1".into(),
        event_type: "job_started".into(),
        payload: serde_json::json!({"status": "running"}),
        timestamp: "2026-03-18T00:00:00Z".into(),
        target_kind: TargetKind::Agent,
        target_name: "Agent1".into(),
    };
    event.normalize();
    let record = event.to_record();
    assert!(matches!(event.target_kind, TargetKind::Agent));
    assert_eq!(event.target_name, "agent1");
    assert_eq!(event.agent_name, "agent1");
    assert_eq!(record["target_kind"], "agent");
    assert_eq!(record["target_name"], "agent1");
}
