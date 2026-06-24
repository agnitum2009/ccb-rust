use ccbr_heartbeat::engine::evaluate_heartbeat;
use ccbr_heartbeat::lock::MaintenanceHeartbeatLock;
use ccbr_heartbeat::maintenance::{evaluate_project_view, evaluate_ps_summary};
use ccbr_heartbeat::models::{
    HeartbeatAction, HeartbeatPolicy, HeartbeatState, MaintenanceHeartbeatActivation,
    MaintenanceHeartbeatRunner, MaintenanceHeartbeatSchedule, MaintenanceHeartbeatStatus,
};
use ccbr_heartbeat::store::{HeartbeatStateStore, MaintenanceHeartbeatStore, ReadState};
use ccbr_storage::paths::PathLayout;

fn policy() -> HeartbeatPolicy {
    HeartbeatPolicy::new(600.0, 600.0, None).unwrap()
}

fn temp_layout() -> (tempfile::TempDir, PathLayout) {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("repo");
    std::fs::create_dir_all(&project).unwrap();
    let layout = PathLayout::new(project.to_str().unwrap());
    (dir, layout)
}

#[test]
fn heartbeat_policy_validates_fields() {
    assert!(HeartbeatPolicy::new(600.0, 600.0, None).is_ok());
    assert!(HeartbeatPolicy::new(-1.0, 600.0, None).is_err());
    assert!(HeartbeatPolicy::new(600.0, 0.0, None).is_err());
    assert!(HeartbeatPolicy::new(600.0, 600.0, Some(0)).is_err());
}

#[test]
fn heartbeat_enters_and_repeats_after_silence() {
    let policy = policy();

    let (_idle_state, idle_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:00:00Z",
        "2026-04-04T00:09:59Z",
        None,
    );
    assert_eq!(idle_decision.action, HeartbeatAction::Idle);

    let (entered_state, entered_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:00:00Z",
        "2026-04-04T00:10:00Z",
        Some(&_idle_state),
    );
    assert_eq!(entered_decision.action, HeartbeatAction::Enter);
    assert!(entered_decision.notice_due());
    assert_eq!(entered_state.notice_count, 1);

    let (repeated_state, repeated_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:00:00Z",
        "2026-04-04T00:20:00Z",
        Some(&entered_state),
    );
    assert_eq!(repeated_decision.action, HeartbeatAction::Repeat);
    assert!(repeated_decision.notice_due());
    assert_eq!(repeated_state.notice_count, 2);
}

#[test]
fn heartbeat_resets_after_unpersisted_progress_advance() {
    let policy = policy();
    let active_state = HeartbeatState {
        subject_kind: "job_progress".into(),
        subject_id: "job_1".into(),
        owner: "agent1".into(),
        last_progress_at: "2026-04-04T00:00:00Z".into(),
        last_notice_at: Some("2026-04-04T00:10:00Z".into()),
        heartbeat_started_at: Some("2026-04-04T00:10:00Z".into()),
        notice_count: 1,
        updated_at: "2026-04-04T00:10:00Z".into(),
    };

    let (reset_state, reset_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:12:00Z",
        "2026-04-04T00:12:00Z",
        Some(&active_state),
    );
    assert_eq!(reset_decision.action, HeartbeatAction::Reset);
    assert_eq!(reset_state.notice_count, 0);
    assert_eq!(reset_state.last_notice_at, None);

    let (idle_state, idle_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:13:00Z",
        "2026-04-04T00:13:00Z",
        Some(&reset_state),
    );
    assert_eq!(idle_decision.action, HeartbeatAction::Idle);
    assert_eq!(idle_state.last_progress_at, "2026-04-04T00:13:00Z");

    let (entered_state, entered_decision) = evaluate_heartbeat(
        &policy,
        "job_progress",
        "job_1",
        "agent1",
        "2026-04-04T00:13:00Z",
        "2026-04-04T00:24:00Z",
        Some(&reset_state),
    );
    assert_eq!(entered_decision.action, HeartbeatAction::Enter);
    assert!(entered_decision.notice_due());
    assert_eq!(entered_state.notice_count, 1);
}

#[test]
fn heartbeat_respects_max_notice_count() {
    let policy = HeartbeatPolicy::new(1.0, 1.0, Some(2)).unwrap();
    let mut state = None;
    for i in 1..=5 {
        let (new_state, decision) = evaluate_heartbeat(
            &policy,
            "job",
            "1",
            "owner",
            "2026-01-01T00:00:00Z",
            &format!("2026-01-01T00:00:{:02}Z", i),
            state.as_ref(),
        );
        state = Some(new_state);
        if i == 1 {
            assert_eq!(decision.action, HeartbeatAction::Enter);
        } else if i == 2 {
            assert_eq!(decision.action, HeartbeatAction::Repeat);
        } else {
            assert_eq!(decision.action, HeartbeatAction::Idle);
        }
    }
    assert_eq!(state.unwrap().notice_count, 2);
}

#[test]
fn heartbeat_state_store_round_trips_and_lists() {
    let (_dir, layout) = temp_layout();
    let store = HeartbeatStateStore::new(layout.clone());

    assert!(store.load("job_progress", "job_1").unwrap().is_none());

    let state = HeartbeatState {
        subject_kind: "job_progress".into(),
        subject_id: "job_1".into(),
        owner: "agent1".into(),
        last_progress_at: "2026-04-04T00:00:00Z".into(),
        last_notice_at: None,
        heartbeat_started_at: None,
        notice_count: 0,
        updated_at: "2026-04-04T00:00:00Z".into(),
    };
    store.save(&state).unwrap();

    let loaded = store.load("job_progress", "job_1").unwrap().unwrap();
    assert_eq!(loaded, state);

    let state2 = HeartbeatState {
        subject_id: "job_2".into(),
        ..state.clone()
    };
    store.save(&state2).unwrap();

    let all = store.list_all(None).unwrap();
    assert_eq!(all.len(), 2);
    let filtered = store.list_all(Some("job_progress")).unwrap();
    assert_eq!(filtered.len(), 2);

    store.remove("job_progress", "job_1").unwrap();
    assert!(store.load("job_progress", "job_1").unwrap().is_none());
}

#[test]
fn maintenance_store_round_trips_and_reports_missing() {
    let (_dir, layout) = temp_layout();
    let project_id = layout.project_id().to_string();
    let store = MaintenanceHeartbeatStore::new(layout.clone(), &project_id).unwrap();

    assert_eq!(store.load_schedule().state, ReadState::Missing);
    assert_eq!(store.load_status().state, ReadState::Missing);
    assert_eq!(store.load_runner().state, ReadState::Missing);

    let schedule = MaintenanceHeartbeatSchedule {
        project_id: project_id.clone(),
        next_run_at: Some("2026-06-10T12:00:00Z".into()),
        reason: Some("manual_test".into()),
        updated_at: Some("2026-06-10T11:00:00Z".into()),
        updated_by: Some("test".into()),
    };
    store.save_schedule(&schedule).unwrap();

    let status = MaintenanceHeartbeatStatus {
        project_id: project_id.clone(),
        last_tick_status: Some("idle".into()),
        last_tick_at: Some("2026-06-10T11:00:00Z".into()),
        last_ok_at: Some("2026-06-10T11:00:00Z".into()),
        last_error: None,
        unknown_streak: 0,
        updated_at: Some("2026-06-10T11:00:00Z".into()),
        source_kind: None,
        recommended_action: None,
        next_heartbeat_after_s: None,
        needs_user: false,
        summary: None,
        evidence: Vec::new(),
        last_activation_status: None,
        last_activation_id: None,
        last_activation_job_id: None,
        last_activation_target: None,
        last_activation_dedup_key: None,
    };
    store.save_status(&status).unwrap();

    let runner = MaintenanceHeartbeatRunner {
        project_id: project_id.clone(),
        runner_id: "runner_1".into(),
        pid: Some(123),
        state: "running".into(),
        source: Some("test".into()),
        started_at: Some("2026-06-10T11:00:00Z".into()),
        last_seen_at: Some("2026-06-10T11:00:01Z".into()),
        last_wake_at: None,
        last_tick_at: None,
        last_tick_status: None,
        observed_next_run_at: None,
        sleep_until: None,
        exit_reason: None,
    };
    store.save_runner(&runner).unwrap();

    let activation = MaintenanceHeartbeatActivation {
        project_id: project_id.clone(),
        activation_id: "act_1".into(),
        status: "submitted".into(),
        condition_kind: "heartbeat_state_check".into(),
        trigger_kind: "state_check".into(),
        source: "project_view".into(),
        observed_at: "2026-06-10T11:00:00Z".into(),
        target_agent: "demo".into(),
        delivery_mode: "ask_silence".into(),
        payload_kind: "maintenance_diagnostic".into(),
        dedup_key: "maintenance:test".into(),
        reason: "provider_prompt_idle".into(),
        created_by: "maintenance-heartbeat".into(),
        not_before: None,
        expires_at: None,
        job_id: Some("job_1".into()),
        submitted_at: Some("2026-06-10T11:00:00Z".into()),
        suppressed_reason: None,
        error: None,
        repeat_count: 0,
        payload_summary: None,
        evidence: Vec::new(),
    };
    store.append_activation(&activation).unwrap();

    let schedule_result = store.load_schedule();
    let status_result = store.load_status();
    let runner_result = store.load_runner();

    assert_eq!(schedule_result.state, ReadState::Ok);
    assert_eq!(
        schedule_result.value.unwrap().next_run_at,
        Some("2026-06-10T12:00:00Z".into())
    );
    assert_eq!(status_result.state, ReadState::Ok);
    assert_eq!(
        status_result.value.unwrap().last_tick_status,
        Some("idle".into())
    );
    assert_eq!(runner_result.state, ReadState::Ok);
    assert_eq!(runner_result.value.unwrap().runner_id, "runner_1");

    let activations = store.load_activation_tail(5).unwrap();
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].job_id, Some("job_1".into()));
}

#[test]
fn maintenance_store_reports_corrupt_files() {
    let (_dir, layout) = temp_layout();
    let project_id = layout.project_id().to_string();
    let store = MaintenanceHeartbeatStore::new(layout.clone(), &project_id).unwrap();
    let path = layout.ccbd_maintenance_heartbeat_schedule_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{not json}\n").unwrap();

    let result = store.load_schedule();
    assert_eq!(result.state, ReadState::Corrupt);
    assert!(result.error.is_some());
    assert!(result.value.is_none());
}

fn project_view_payload(
    agent: serde_json::Value,
    comms: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "view": {
            "ccbd": {"state": "mounted", "health": "healthy", "generation": 1},
            "agents": [agent],
            "comms": comms,
        },
        "cache": {"generated_at": "2026-06-10T12:00:00Z"},
    })
}

fn base_agent() -> serde_json::Value {
    serde_json::json!({
        "name": "demo",
        "activity_state": "idle",
        "activity_reason": "pane_alive",
        "activity_source": "pane_liveness",
        "queue_depth": 0,
        "runtime_state": "idle",
        "pane_id": "%1",
        "window": "main",
    })
}

#[test]
fn maintenance_classifier_flags_provider_work_without_control_work() {
    let mut agent = base_agent();
    agent["activity_state"] = "active".into();
    agent["activity_reason"] = "provider_working".into();
    agent["activity_source"] = "provider_pane".into();
    agent["pane_id"] = "%3".into();

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![]));
    assert_eq!(evaluation.health, "concern");
    assert_eq!(evaluation.summary["suspicion_count"], 1);
    let envelope = evaluation.evidence[0].as_object().unwrap();
    assert_eq!(envelope["kind"], "suspicion_envelope");
    assert_eq!(
        envelope["condition_kind"],
        "provider_work_without_control_work"
    );
    assert_eq!(envelope["agent"], "demo");
    let allowed = envelope["allowed_actions"].as_array().unwrap();
    assert!(allowed.iter().any(|v| v == "capture_pane_readonly"));
}

#[test]
fn maintenance_classifier_keeps_active_ccbr_job_healthy() {
    let mut agent = base_agent();
    agent["activity_state"] = "active".into();
    agent["activity_reason"] = "job_running".into();
    agent["activity_source"] = "ccbr_job".into();
    agent["current_job_id"] = "job_running_1234".into();
    agent["queue_depth"] = 1.into();

    let comm = serde_json::json!({
        "id": "job_running_1234",
        "target": "demo",
        "business_status": "replying",
        "status": "running",
    });

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![comm]));
    assert_eq!(evaluation.health, "healthy");
    assert_eq!(evaluation.summary["suspicion_count"], 0);
    assert!(evaluation.evidence.is_empty());
}

#[test]
fn maintenance_classifier_keeps_active_comms_without_current_job_healthy() {
    let agent = base_agent();
    let comm = serde_json::json!({
        "id": "job_replying_1234",
        "target": "demo",
        "business_status": "replying",
        "status": "running",
    });

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![comm]));
    assert_eq!(evaluation.health, "healthy");
    assert_eq!(evaluation.summary["active_comms_count"], 1);
    assert_eq!(evaluation.summary["suspicion_count"], 0);
    assert!(evaluation.evidence.is_empty());
}

#[test]
fn maintenance_classifier_flags_degraded_activity_evidence() {
    let mut agent = base_agent();
    agent["activity_state"] = "pending".into();
    agent["activity_reason"] = "".into();
    agent["activity_source"] = "".into();

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![]));
    assert_eq!(evaluation.health, "unknown");
    assert_eq!(evaluation.summary["suspicion_count"], 1);
    let envelope = evaluation.evidence[0].as_object().unwrap();
    assert_eq!(envelope["condition_kind"], "degraded_activity_evidence");
    assert_eq!(envelope["source"], "unknown");
}

#[test]
fn maintenance_classifier_flags_active_degraded_activity_evidence() {
    let mut agent = base_agent();
    agent["activity_state"] = "active".into();
    agent["activity_reason"] = "provider_working".into();
    agent["activity_source"] = "".into();

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![]));
    assert_eq!(evaluation.health, "unknown");
    assert_eq!(evaluation.summary["suspicion_count"], 1);
    let envelope = evaluation.evidence[0].as_object().unwrap();
    assert_eq!(envelope["condition_kind"], "degraded_activity_evidence");
    assert_eq!(envelope["control_state"]["activity_state"], "active");
}

#[test]
fn maintenance_classifier_ignores_fresh_pending_anchor_runtime() {
    let mut agent = base_agent();
    agent["activity_state"] = "active".into();
    agent["activity_reason"] = "job_running".into();
    agent["activity_source"] = "ccbr_job".into();
    agent["current_job_id"] = "job_running_1234".into();
    agent["provider_runtime"] = serde_json::json!({
        "job_id": "job_running_1234",
        "agent_name": "demo",
        "provider": "codex",
        "primary_authority": "protocol_log",
        "runtime_state": {
            "delivery_state": "pending_anchor",
            "anchor_seen": false,
            "delivery_started_at": "2026-06-10T11:59:55Z",
            "delivery_timeout_s": 120.0,
        },
    });

    let comm = serde_json::json!({
        "id": "job_running_1234",
        "target": "demo",
        "business_status": "replying",
        "status": "running",
    });

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![comm]));
    assert_eq!(evaluation.health, "healthy");
    assert_eq!(evaluation.summary["suspicion_count"], 0);
}

#[test]
fn maintenance_classifier_flags_pending_anchor_runtime_after_window() {
    let mut agent = base_agent();
    agent["activity_state"] = "active".into();
    agent["activity_reason"] = "job_running".into();
    agent["activity_source"] = "ccbr_job".into();
    agent["current_job_id"] = "job_running_1234".into();
    agent["provider_runtime"] = serde_json::json!({
        "job_id": "job_running_1234",
        "agent_name": "demo",
        "provider": "codex",
        "primary_authority": "protocol_log",
        "runtime_state": {
            "delivery_state": "pending_anchor",
            "anchor_seen": false,
            "delivery_started_at": "2026-06-10T11:59:15Z",
            "delivery_timeout_s": 120.0,
        },
    });

    let comm = serde_json::json!({
        "id": "job_running_1234",
        "target": "demo",
        "business_status": "replying",
        "status": "running",
    });

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![comm]));
    assert_eq!(evaluation.health, "concern");
    assert_eq!(evaluation.summary["suspicion_count"], 1);
    let envelope = evaluation.evidence[0].as_object().unwrap();
    assert_eq!(
        envelope["condition_kind"],
        "provider_delivery_pending_anchor"
    );
}

#[test]
fn maintenance_classifier_flags_provider_runtime_without_control_job() {
    let mut agent = base_agent();
    agent["provider_runtime"] = serde_json::json!({
        "job_id": "job_orphan_runtime",
        "agent_name": "demo",
        "provider": "codex",
        "runtime_state": {
            "delivery_state": "accepted",
            "anchor_seen": true,
        },
    });

    let evaluation = evaluate_project_view(&project_view_payload(agent, vec![]));
    assert_eq!(evaluation.health, "concern");
    assert_eq!(evaluation.summary["suspicion_count"], 1);
    assert_eq!(
        evaluation.evidence[0]["condition_kind"],
        "provider_runtime_without_control_job"
    );
}

#[test]
fn maintenance_ps_summary_detects_failed_and_binding_concern() {
    let payload = serde_json::json!({
        "ccbd_state": "mounted",
        "agents": [
            {"agent_name": "demo", "state": "failed", "binding_status": "bound"},
            {"agent_name": "other", "state": "idle", "binding_status": "unbound"},
        ],
    });

    let evaluation = evaluate_ps_summary(&payload, None);
    assert_eq!(evaluation.health, "failing");
    assert_eq!(evaluation.summary["failed_agent_count"], 1);
    assert_eq!(evaluation.summary["concern_agent_count"], 1);
}

#[test]
fn maintenance_ps_summary_falls_back_with_error() {
    let payload = serde_json::json!({"ccbd_state": "mounted", "agents": []});
    let evaluation = evaluate_ps_summary(&payload, Some("ccbd unavailable"));
    assert_eq!(evaluation.health, "unknown");
    assert_eq!(evaluation.summary["fallback_error"], "ccbd unavailable");
}

#[test]
fn maintenance_lock_acquire_and_release() {
    let (_dir, layout) = temp_layout();
    let path = layout.ccbd_maintenance_heartbeat_lock_path();
    let payload = serde_json::json!({
        "schema_version": 1,
        "record_type": "maintenance_heartbeat_lock",
        "project_id": layout.project_id(),
        "pid": 123,
    });

    {
        let lock = MaintenanceHeartbeatLock::try_acquire(&path, payload.clone());
        assert!(lock.is_ok());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"held\": true"));
    }

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("\"held\": false"));
}

#[test]
fn maintenance_lock_reports_busy() {
    let (_dir, layout) = temp_layout();
    let path = layout.ccbd_maintenance_heartbeat_lock_path();
    let payload = serde_json::json!({"pid": 123});

    let _first = MaintenanceHeartbeatLock::try_acquire(&path, payload.clone()).unwrap();
    let result = MaintenanceHeartbeatLock::try_acquire(&path, payload.clone());
    assert!(result.is_err());
}

#[test]
fn maintenance_lock_release_is_idempotent() {
    let (_dir, layout) = temp_layout();
    let path = layout.ccbd_maintenance_heartbeat_lock_path();
    let mut lock = MaintenanceHeartbeatLock::try_acquire(&path, serde_json::json!({})).unwrap();
    lock.release().unwrap();
    lock.release().unwrap();
}
