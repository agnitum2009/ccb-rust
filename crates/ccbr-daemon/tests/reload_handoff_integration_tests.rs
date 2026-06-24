//! Integration tests for reload-handoff behavior (signature/expiry/holder parity
//! with the Python reference implementation).

use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::reload_handoff::{
    begin_reload_handoff, reload_handoff_allows_signature_mismatch, ReloadHandoff,
    ReloadHandoffStore, RELOAD_HANDOFF_TTL_S,
};
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use serde_json::json;
use tempfile::TempDir;

const TARGET_SIG: &str = "target-sig";
const WRONG_TARGET_SIG: &str = "wrong-target-sig";
const OLD_SIG: &str = "old-sig";

// These tests run single-threaded (`--test-threads=1`), so any fixed positive
// pid is safe for ownership acquisition.
const CURRENT_PID: u32 = 12_345;
const FAKE_PID: u32 = 42;
const OTHER_PID: u32 = 999_999;
const FAKE_GENERATION: u32 = 7;

const STARTED_AT: &str = "2024-01-01T00:00:00Z";
const INSTANCE_ID: &str = "test-instance-001";
const EXPECTED_STATUS: &str = "applying";

fn stub_app_with_owner(dir: &TempDir) -> CcbdApp {
    let mut app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let socket_path = app.socket_path();
    let _ = app
        .ownership
        .acquire(CURRENT_PID, &socket_path, INSTANCE_ID);
    app
}

fn begin_test_handoff(dir: &TempDir) -> (CcbdApp, ReloadHandoff) {
    let mut app = stub_app_with_owner(dir);
    let identity = json!({"config_signature": TARGET_SIG});
    let handoff = begin_reload_handoff(&mut app, &identity).expect("handoff should be created");
    (app, handoff)
}

#[test]
fn test_reload_handoff_allows_signature_mismatch_when_holder_matches() {
    let dir = TempDir::new().unwrap();
    let (app, handoff) = begin_test_handoff(&dir);

    assert!(reload_handoff_allows_signature_mismatch(
        &app,
        TARGET_SIG,
        &handoff.old_config_signature,
        Some(&handoff.started_at),
    ));

    // A mismatched target signature should be rejected even when the holder matches.
    assert!(!reload_handoff_allows_signature_mismatch(
        &app,
        WRONG_TARGET_SIG,
        &handoff.old_config_signature,
        Some(&handoff.started_at),
    ));
}

#[test]
fn test_reload_handoff_rejects_expired_handoff() {
    let dir = TempDir::new().unwrap();
    let (app, handoff) = begin_test_handoff(&dir);

    // Move past the TTL window.
    let started = chrono::DateTime::parse_from_rfc3339(&handoff.started_at)
        .expect("valid RFC 3339 timestamp");
    let expired_now =
        (started + chrono::Duration::seconds(RELOAD_HANDOFF_TTL_S as i64 + 1)).to_rfc3339();

    assert!(!reload_handoff_allows_signature_mismatch(
        &app,
        TARGET_SIG,
        &handoff.old_config_signature,
        Some(&expired_now),
    ));
}

#[test]
fn test_reload_handoff_rejects_wrong_holder() {
    let dir = TempDir::new().unwrap();
    let (mut app, handoff) = begin_test_handoff(&dir);

    // Re-acquire ownership with a different pid, which advances the generation.
    let socket_path = app.socket_path();
    let _ = app.ownership.acquire(OTHER_PID, &socket_path, INSTANCE_ID);

    assert!(!reload_handoff_allows_signature_mismatch(
        &app,
        TARGET_SIG,
        &handoff.old_config_signature,
        Some(&handoff.started_at),
    ));
}

#[test]
fn test_reload_handoff_rejects_wrong_instance_id() {
    let dir = TempDir::new().unwrap();
    let (app, handoff) = begin_test_handoff(&dir);

    // Save a handoff whose PID and generation match the current lease but whose
    // daemon_instance_id differs.
    let store = ReloadHandoffStore::new(&app.layout);
    let lease = app.ownership.current().expect("current ownership");
    let mismatched = ReloadHandoff::new(
        app.project_id(),
        handoff.started_at.clone(),
        handoff.old_config_signature.clone(),
        handoff.target_config_signature.clone(),
        lease.owner_pid,
        "other-instance-id",
        lease.generation,
    );
    store.save(&mismatched).expect("save should succeed");

    assert!(!reload_handoff_allows_signature_mismatch(
        &app,
        TARGET_SIG,
        &handoff.old_config_signature,
        Some(&handoff.started_at),
    ));
}

#[test]
fn test_reload_handoff_rejects_project_mismatch() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app_with_owner(&dir);
    let identity = json!({"config_signature": TARGET_SIG});
    let handoff = begin_reload_handoff(&mut app, &identity).expect("handoff should be created");

    // Replace the persisted handoff with one for a different project but the same holder.
    let store = ReloadHandoffStore::new(&app.layout);
    store.clear().expect("clear should succeed");

    let lease = app.ownership.current().expect("current ownership");
    let mismatched = ReloadHandoff::new(
        "other-project-id",
        handoff.started_at.clone(),
        handoff.old_config_signature.clone(),
        handoff.target_config_signature.clone(),
        lease.owner_pid,
        INSTANCE_ID,
        lease.generation,
    );
    store.save(&mismatched).expect("save should succeed");

    assert!(!reload_handoff_allows_signature_mismatch(
        &app,
        TARGET_SIG,
        &handoff.old_config_signature,
        Some(&handoff.started_at),
    ));
}

#[test]
fn test_reload_handoff_store_roundtrip() {
    let dir = TempDir::new().unwrap();
    let app = stub_app_with_owner(&dir);
    let store = ReloadHandoffStore::new(&app.layout);

    assert!(store.load().unwrap().is_none());

    let handoff = ReloadHandoff::new(
        app.project_id(),
        STARTED_AT,
        OLD_SIG,
        TARGET_SIG,
        FAKE_PID,
        INSTANCE_ID,
        FAKE_GENERATION,
    );
    store.save(&handoff).expect("save should succeed");

    let loaded = store.load().unwrap().expect("handoff should be loadable");
    assert_eq!(loaded.project_id, app.project_id());
    assert_eq!(loaded.started_at, STARTED_AT);
    assert_eq!(loaded.old_config_signature, OLD_SIG);
    assert_eq!(loaded.target_config_signature, TARGET_SIG);
    assert_eq!(loaded.daemon_pid, FAKE_PID);
    assert_eq!(loaded.daemon_instance_id, INSTANCE_ID);
    assert_eq!(loaded.generation, FAKE_GENERATION);
    assert_eq!(loaded.status, EXPECTED_STATUS);
    assert_eq!(loaded.ttl_s, RELOAD_HANDOFF_TTL_S);

    store.clear().expect("clear should succeed");
    assert!(store.load().unwrap().is_none());
}
