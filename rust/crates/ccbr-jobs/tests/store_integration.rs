use camino::Utf8Path;
use ccbr_jobs::{
    DeliveryScope, JobEvent, JobEventStore, JobRecord, JobStatus, JobStore, MessageEnvelope,
    SubmissionRecord, SubmissionStore, TargetKind,
};
use std::io::Write;
use tempfile::TempDir;

fn make_job(job_id: &str, agent_name: &str) -> JobRecord {
    JobRecord {
        job_id: job_id.into(),
        submission_id: None,
        agent_name: agent_name.into(),
        provider: "claude".into(),
        request: MessageEnvelope {
            project_id: "p1".into(),
            to_agent: agent_name.into(),
            from_actor: "user".into(),
            body: "hello".into(),
            task_id: None,
            reply_to: None,
            message_type: "task_request".into(),
            delivery_scope: DeliveryScope::Agent,
            silence_on_success: false,
            route_options: serde_json::Value::Object(Default::default()),
            body_artifact: None,
        },
        status: JobStatus::Accepted,
        terminal_decision: None,
        cancel_requested_at: None,
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
        workspace_path: None,
        target_kind: TargetKind::Agent,
        target_name: agent_name.into(),
        provider_instance: None,
        provider_options: serde_json::Value::Object(Default::default()),
    }
}

#[test]
fn job_store_round_trip() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = JobStore::new(&layout);
    store.append(&make_job("job1", "claude")).unwrap();
    let jobs = store.list_agent("claude");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, "job1");
}

#[test]
fn job_store_latest_by_id() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = JobStore::new(&layout);
    store.append(&make_job("job1", "claude")).unwrap();
    let mut updated = make_job("job1", "claude");
    updated.status = JobStatus::Running;
    store.append(&updated).unwrap();

    let latest = store.get_latest("claude", "job1").unwrap();
    assert_eq!(latest.status, JobStatus::Running);
}

#[test]
fn submission_store_round_trip() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = SubmissionStore::new(&layout);
    let record = SubmissionRecord {
        submission_id: "sub1".into(),
        project_id: "p1".into(),
        from_actor: "user".into(),
        target_scope: "agent".into(),
        task_id: None,
        job_ids: vec!["job1".into()],
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    store.append(&record).unwrap();
    let all = store.list_all();
    assert_eq!(all.len(), 1);
    assert_eq!(store.get_latest("sub1").unwrap().submission_id, "sub1");
}

#[test]
fn job_event_serializes_type_field_for_python_compatibility() {
    let event = JobEvent {
        event_id: "e1".into(),
        job_id: "job1".into(),
        agent_name: "claude".into(),
        target_kind: TargetKind::Agent,
        target_name: "claude".into(),
        event_type: "started".into(),
        payload: serde_json::Value::Object(Default::default()),
        timestamp: "2025-01-01T00:00:00Z".into(),
    };
    let record = serde_json::to_value(&event).unwrap();
    let obj = record.as_object().unwrap();
    assert!(
        obj.contains_key("type"),
        "JobEvent record must use 'type' field for Python compatibility"
    );
    assert!(
        !obj.contains_key("event_type"),
        "JobEvent record must not use 'event_type' field"
    );
    assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "started");

    // Roundtrip
    let deserialized: JobEvent = serde_json::from_value(record).unwrap();
    assert_eq!(deserialized.event_type, "started");
}

fn read_first_jsonl_line(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn job_store_record_has_header() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = JobStore::new(&layout);
    store.append(&make_job("job1", "claude")).unwrap();

    let path = layout.target_jobs_path("agent", "claude").unwrap();
    let line = read_first_jsonl_line(path.as_std_path());
    let record: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(record.get("schema_version").unwrap().as_i64().unwrap(), 2);
    assert_eq!(
        record.get("record_type").unwrap().as_str().unwrap(),
        "job_record"
    );
    assert!(record.get("job_id").is_some());
}

#[test]
fn submission_store_record_has_header() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = SubmissionStore::new(&layout);
    let record = SubmissionRecord {
        submission_id: "sub1".into(),
        project_id: "p1".into(),
        from_actor: "user".into(),
        target_scope: "agent".into(),
        task_id: None,
        job_ids: vec!["job1".into()],
        created_at: "2025-01-01T00:00:00Z".into(),
        updated_at: "2025-01-01T00:00:00Z".into(),
    };
    store.append(&record).unwrap();

    let path = layout.ccbd_submissions_path();
    let line = read_first_jsonl_line(path.as_std_path());
    let record: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(record.get("schema_version").unwrap().as_i64().unwrap(), 2);
    assert_eq!(
        record.get("record_type").unwrap().as_str().unwrap(),
        "submission_record"
    );
    assert!(record.get("submission_id").is_some());
}

#[test]
fn job_event_store_record_has_header() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = JobEventStore::new(&layout);
    let event = JobEvent {
        event_id: "e1".into(),
        job_id: "job1".into(),
        agent_name: "claude".into(),
        target_kind: TargetKind::Agent,
        target_name: "claude".into(),
        event_type: "started".into(),
        payload: serde_json::Value::Object(Default::default()),
        timestamp: "2025-01-01T00:00:00Z".into(),
    };
    store.append(&event).unwrap();

    let path = layout.target_events_path("agent", "claude").unwrap();
    let line = read_first_jsonl_line(path.as_std_path());
    let record: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(record.get("schema_version").unwrap().as_i64().unwrap(), 2);
    assert_eq!(
        record.get("record_type").unwrap().as_str().unwrap(),
        "job_event"
    );
    assert!(record.get("type").is_some());
}

#[test]
fn event_store_skips_non_job_event_records() {
    let dir = TempDir::new().unwrap();
    let p = Utf8Path::from_path(dir.path()).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(p);
    let store = JobEventStore::new(&layout);

    let event = |event_id: &str, event_type: &str| JobEvent {
        event_id: event_id.into(),
        job_id: "job1".into(),
        agent_name: "claude".into(),
        target_kind: TargetKind::Agent,
        target_name: "claude".into(),
        event_type: event_type.into(),
        payload: serde_json::Value::Object(Default::default()),
        timestamp: "2025-01-01T00:00:00Z".into(),
    };

    store.append(&event("e1", "started")).unwrap();
    store.append(&event("e2", "running")).unwrap();

    // Inject a foreign record type directly into the events JSONL file.
    let path = layout.target_events_path("agent", "claude").unwrap();
    let foreign = serde_json::json!({
        "schema_version": 2,
        "record_type": "heartbeat_state",
        "agent_name": "claude",
        "timestamp": "2025-01-01T00:00:00Z",
    });
    std::fs::OpenOptions::new()
        .append(true)
        .open(path.as_std_path())
        .unwrap()
        .write_all(format!("{}\n", foreign).as_bytes())
        .unwrap();

    store.append(&event("e3", "completed")).unwrap();

    let (line_no, events) = store.read_since("claude", 0);
    assert_eq!(line_no, 4, "line count should include the foreign record");
    assert_eq!(events.len(), 3, "only job_event records should be returned");
    assert_eq!(events[0].event_id, "e1");
    assert_eq!(events[1].event_id, "e2");
    assert_eq!(events[2].event_id, "e3");
}
