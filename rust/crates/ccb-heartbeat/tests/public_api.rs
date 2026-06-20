use ccb_heartbeat::{
    evaluate_project_view, evaluate_ps_summary, HeartbeatAction, HeartbeatDecision, HeartbeatError,
    HeartbeatPolicy, HeartbeatState, HeartbeatStateStore, MaintenanceHeartbeatActivation,
    MaintenanceHeartbeatEvaluation, MaintenanceHeartbeatLockBusy, MaintenanceHeartbeatReadResult,
    MaintenanceHeartbeatRunner, MaintenanceHeartbeatSchedule, MaintenanceHeartbeatStatus,
    MaintenanceHeartbeatStore, ReadState, Result, ACTIVATION_RECORD_TYPE, HEALTH_CONCERN,
    HEALTH_FAILING, HEALTH_HEALTHY, HEALTH_UNKNOWN, HEARTBEAT_STATE_RECORD_TYPE,
    MAINTENANCE_HEARTBEAT_ACTIVATION_RECORD_TYPE, MAINTENANCE_HEARTBEAT_RUNNER_RECORD_TYPE,
    MAINTENANCE_HEARTBEAT_SCHEDULE_RECORD_TYPE, MAINTENANCE_HEARTBEAT_STATUS_RECORD_TYPE,
    RECOMMENDED_ACTION_ASSESS_LATER, RECOMMENDED_ACTION_NONE, RUNNER_RECORD_TYPE,
    SCHEDULE_RECORD_TYPE, SCHEMA_VERSION, STATUS_RECORD_TYPE,
};
use ccb_storage::paths::PathLayout;

fn temp_layout() -> (tempfile::TempDir, PathLayout) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("repo");
    std::fs::create_dir_all(&project).unwrap();
    let layout = PathLayout::new(project.to_str().unwrap());
    (dir, layout)
}

#[test]
fn crate_root_re_exports_are_reachable() {
    // Compile-time assertion that every Python-public name is reachable.
    let _: HeartbeatAction = HeartbeatAction::Idle;
    let _: HeartbeatDecision = HeartbeatDecision {
        action: HeartbeatAction::Idle,
        subject_kind: "kind".into(),
        subject_id: "id".into(),
        owner: "owner".into(),
        last_progress_at: "2026-01-01T00:00:00Z".into(),
        last_notice_at: None,
        silence_seconds: 0.0,
        notice_count: 0,
    };
    let _: HeartbeatPolicy = HeartbeatPolicy::new(1.0, 1.0, None).unwrap();
    let _: MaintenanceHeartbeatEvaluation = evaluate_project_view(&serde_json::json!({}));
    let _: MaintenanceHeartbeatEvaluation = evaluate_ps_summary(&serde_json::json!({}), None);
    let _: MaintenanceHeartbeatLockBusy = MaintenanceHeartbeatLockBusy;

    // Constants.
    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(HEALTH_HEALTHY, "healthy");
    assert_eq!(HEALTH_CONCERN, "concern");
    assert_eq!(HEALTH_FAILING, "failing");
    assert_eq!(HEALTH_UNKNOWN, "unknown");
    assert_eq!(RECOMMENDED_ACTION_NONE, "none");
    assert_eq!(RECOMMENDED_ACTION_ASSESS_LATER, "assess_later");
    assert_eq!(HEARTBEAT_STATE_RECORD_TYPE, "heartbeat_state");
    assert_eq!(SCHEDULE_RECORD_TYPE, "maintenance_heartbeat_schedule");
    assert_eq!(STATUS_RECORD_TYPE, "maintenance_heartbeat_status");
    assert_eq!(ACTIVATION_RECORD_TYPE, "maintenance_heartbeat_activation");
    assert_eq!(RUNNER_RECORD_TYPE, "maintenance_heartbeat_runner");
    assert_eq!(
        MAINTENANCE_HEARTBEAT_SCHEDULE_RECORD_TYPE,
        "maintenance_heartbeat_schedule"
    );
    assert_eq!(
        MAINTENANCE_HEARTBEAT_STATUS_RECORD_TYPE,
        "maintenance_heartbeat_status"
    );
    assert_eq!(
        MAINTENANCE_HEARTBEAT_ACTIVATION_RECORD_TYPE,
        "maintenance_heartbeat_activation"
    );
    assert_eq!(
        MAINTENANCE_HEARTBEAT_RUNNER_RECORD_TYPE,
        "maintenance_heartbeat_runner"
    );

    // Result and error types.
    let _: Result<()> = Ok(());
    let _: HeartbeatError = HeartbeatError::Validation("test".into());

    // Store types.
    let (_dir, layout) = temp_layout();
    let _: HeartbeatStateStore = HeartbeatStateStore::new(layout.clone());
    let _: MaintenanceHeartbeatReadResult<MaintenanceHeartbeatSchedule> =
        MaintenanceHeartbeatReadResult {
            state: ReadState::Missing,
            path: String::new(),
            value: None,
            error: None,
        };
    let _: MaintenanceHeartbeatStore = MaintenanceHeartbeatStore::new(layout, "project").unwrap();
}

#[test]
fn heartbeat_state_store_accepts_injected_store() {
    let (_dir, layout) = temp_layout();
    let injected = ccb_storage::json::JsonStore::new();
    let store = HeartbeatStateStore::with_store(layout.clone(), Some(injected));
    assert!(store.load("kind", "id").unwrap().is_none());

    let store_default = HeartbeatStateStore::with_store(layout, None);
    assert!(store_default.load("kind", "id").unwrap().is_none());
}

#[test]
fn maintenance_heartbeat_store_accepts_injected_stores() {
    let (_dir, layout) = temp_layout();
    let project_id = layout.project_id().to_string();
    let json_store = ccb_storage::json::JsonStore::new();
    let jsonl_store = ccb_storage::jsonl::JsonlStore::new();
    let store = MaintenanceHeartbeatStore::with_stores(
        layout.clone(),
        &project_id,
        Some(json_store),
        Some(jsonl_store),
    )
    .unwrap();
    assert_eq!(store.load_schedule().state, ReadState::Missing);

    let store_default =
        MaintenanceHeartbeatStore::with_stores(layout, &project_id, None, None).unwrap();
    assert_eq!(store_default.load_schedule().state, ReadState::Missing);
}

#[test]
fn maintenance_heartbeat_store_rejects_empty_project_id() {
    let (_dir, layout) = temp_layout();
    let err = MaintenanceHeartbeatStore::with_stores(layout, "   ", None, None).unwrap_err();
    assert!(matches!(err, HeartbeatError::Validation(_)));
}

#[test]
fn heartbeat_state_validates_required_fields() {
    let base = HeartbeatState::new(
        "kind",
        "id",
        "owner",
        "2026-01-01T00:00:00Z",
        None,
        None,
        0,
        "2026-01-01T00:00:00Z",
    );
    assert!(base.is_ok());

    assert!(HeartbeatState::new(
        "",
        "id",
        "owner",
        "2026-01-01T00:00:00Z",
        None,
        None,
        0,
        "2026-01-01T00:00:00Z",
    )
    .is_err());
    assert!(HeartbeatState::new(
        "kind",
        "   ",
        "owner",
        "2026-01-01T00:00:00Z",
        None,
        None,
        0,
        "2026-01-01T00:00:00Z",
    )
    .is_err());
    assert!(HeartbeatState::new(
        "kind",
        "id",
        "",
        "2026-01-01T00:00:00Z",
        None,
        None,
        0,
        "2026-01-01T00:00:00Z",
    )
    .is_err());
    assert!(HeartbeatState::new(
        "kind",
        "id",
        "owner",
        "",
        None,
        None,
        0,
        "2026-01-01T00:00:00Z",
    )
    .is_err());
    assert!(HeartbeatState::new(
        "kind",
        "id",
        "owner",
        "2026-01-01T00:00:00Z",
        None,
        None,
        0,
        "",
    )
    .is_err());
}

#[test]
fn maintenance_heartbeat_schedule_validates_project_id() {
    assert!(MaintenanceHeartbeatSchedule::new("project", None, None, None, None).is_ok());
    assert!(MaintenanceHeartbeatSchedule::new("", None, None, None, None).is_err());
    assert!(MaintenanceHeartbeatSchedule::new("   ", None, None, None, None).is_err());
}

#[test]
fn maintenance_heartbeat_status_validates_fields() {
    let base = MaintenanceHeartbeatStatus::new(
        "project",
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        false,
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
    );
    assert!(base.is_ok());

    assert!(MaintenanceHeartbeatStatus::new(
        "",
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        false,
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());

    assert!(MaintenanceHeartbeatStatus::new(
        "project",
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        Some(0),
        false,
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());

    assert!(MaintenanceHeartbeatStatus::new(
        "project",
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        false,
        Some(serde_json::json!("not an object")),
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());

    assert!(MaintenanceHeartbeatStatus::new(
        "project",
        None,
        None,
        None,
        None,
        0,
        None,
        None,
        None,
        None,
        false,
        None,
        vec![serde_json::json!("not an object")],
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());
}

#[test]
fn maintenance_heartbeat_runner_validates_fields() {
    let base = MaintenanceHeartbeatRunner::new(
        "project", "runner_1", None, "unknown", None, None, None, None, None, None, None, None,
        None,
    );
    assert!(base.is_ok());

    assert!(MaintenanceHeartbeatRunner::new(
        "", "runner_1", None, "unknown", None, None, None, None, None, None, None, None, None,
    )
    .is_err());
    assert!(MaintenanceHeartbeatRunner::new(
        "project", "", None, "unknown", None, None, None, None, None, None, None, None, None,
    )
    .is_err());
    assert!(MaintenanceHeartbeatRunner::new(
        "project", "runner_1", None, "", None, None, None, None, None, None, None, None, None,
    )
    .is_err());
    assert!(MaintenanceHeartbeatRunner::new(
        "project",
        "runner_1",
        Some(0),
        "unknown",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .is_err());
}

#[test]
fn maintenance_heartbeat_activation_validates_fields() {
    let make = || {
        MaintenanceHeartbeatActivation::new(
            "project",
            "activation_1",
            "submitted",
            "condition",
            "trigger",
            "source",
            "2026-01-01T00:00:00Z",
            "agent",
            "mode",
            "payload",
            "dedup",
            "reason",
            "maintenance-heartbeat",
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            None,
            Vec::new(),
        )
    };
    assert!(make().is_ok());

    for (field, value) in [
        ("project_id", ""),
        ("activation_id", ""),
        ("status", ""),
        ("condition_kind", ""),
        ("trigger_kind", ""),
        ("source", ""),
        ("observed_at", ""),
        ("target_agent", ""),
        ("delivery_mode", ""),
        ("payload_kind", ""),
        ("dedup_key", ""),
        ("reason", ""),
        ("created_by", ""),
    ] {
        let mut base = make().unwrap();
        match field {
            "project_id" => base.project_id = value.into(),
            "activation_id" => base.activation_id = value.into(),
            "status" => base.status = value.into(),
            "condition_kind" => base.condition_kind = value.into(),
            "trigger_kind" => base.trigger_kind = value.into(),
            "source" => base.source = value.into(),
            "observed_at" => base.observed_at = value.into(),
            "target_agent" => base.target_agent = value.into(),
            "delivery_mode" => base.delivery_mode = value.into(),
            "payload_kind" => base.payload_kind = value.into(),
            "dedup_key" => base.dedup_key = value.into(),
            "reason" => base.reason = value.into(),
            "created_by" => base.created_by = value.into(),
            _ => unreachable!(),
        }
        // Validation happens only through `new`, so re-construct and check it fails.
        let result = MaintenanceHeartbeatActivation::new(
            if field == "project_id" {
                value
            } else {
                "project"
            },
            if field == "activation_id" {
                value
            } else {
                "activation_1"
            },
            if field == "status" {
                value
            } else {
                "submitted"
            },
            if field == "condition_kind" {
                value
            } else {
                "condition"
            },
            if field == "trigger_kind" {
                value
            } else {
                "trigger"
            },
            if field == "source" { value } else { "source" },
            if field == "observed_at" {
                value
            } else {
                "2026-01-01T00:00:00Z"
            },
            if field == "target_agent" {
                value
            } else {
                "agent"
            },
            if field == "delivery_mode" {
                value
            } else {
                "mode"
            },
            if field == "payload_kind" {
                value
            } else {
                "payload"
            },
            if field == "dedup_key" { value } else { "dedup" },
            if field == "reason" { value } else { "reason" },
            if field == "created_by" {
                value
            } else {
                "maintenance-heartbeat"
            },
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            None,
            Vec::new(),
        );
        assert!(result.is_err(), "expected error for empty {field}");
    }

    assert!(MaintenanceHeartbeatActivation::new(
        "project",
        "activation_1",
        "submitted",
        "condition",
        "trigger",
        "source",
        "2026-01-01T00:00:00Z",
        "agent",
        "mode",
        "payload",
        "dedup",
        "reason",
        "maintenance-heartbeat",
        None,
        None,
        None,
        None,
        None,
        None,
        0,
        Some(serde_json::json!("not an object")),
        Vec::new(),
    )
    .is_err());

    assert!(MaintenanceHeartbeatActivation::new(
        "project",
        "activation_1",
        "submitted",
        "condition",
        "trigger",
        "source",
        "2026-01-01T00:00:00Z",
        "agent",
        "mode",
        "payload",
        "dedup",
        "reason",
        "maintenance-heartbeat",
        None,
        None,
        None,
        None,
        None,
        None,
        0,
        None,
        vec![serde_json::json!("not an object")],
    )
    .is_err());
}
