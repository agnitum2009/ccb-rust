//! Integration tests for CCBR daemon ownership persistence across restarts.

use camino::Utf8Path;
use ccbr_daemon::CcbdApp;
use ccbr_storage::paths::PathLayout;

fn make_test_app(project_root: &std::path::Path) -> CcbdApp {
    CcbdApp::with_backend(
        project_root,
        ccbr_daemon::start_flow::service::StartFlowService::with_stub(),
        ccbr_daemon::stop_flow::service::StopFlowService::with_stub(),
    )
}

#[test]
fn ownership_save_load_roundtrip_preserves_record() {
    let dir = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(Utf8Path::from_path(dir.path()).unwrap());
    let state_path = layout.ccbrd_dir().join("ownership-state.json");

    let mut app = make_test_app(dir.path());
    app.start().unwrap();
    let first = app.ownership.current().unwrap().clone();
    app.shutdown().unwrap();

    assert!(state_path.exists(), "ownership state should be persisted");

    let mut restarted = make_test_app(dir.path());
    restarted.start().unwrap();
    let second = restarted.ownership.current().unwrap().clone();

    // Generation sequence continues from the persisted record.
    assert_eq!(second.generation, first.generation + 1);
    assert_eq!(second.socket_path, first.socket_path);
}

#[test]
fn ownership_restart_reuses_same_guard_without_duplicate() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut app = make_test_app(dir.path());

    app.start().unwrap();
    let first = app.ownership.current().unwrap().clone();

    // Simulate in-process restart: shutdown then start the same app instance.
    app.shutdown().unwrap();
    app.start().unwrap();
    let second = app.ownership.current().unwrap().clone();

    // Same holder (same pid + instance_id) should be restored, not duplicated.
    assert_eq!(second.generation, first.generation);
    assert_eq!(second.instance_id, first.instance_id);
    assert_eq!(second.owner_pid, first.owner_pid);
    assert_eq!(second.socket_path, first.socket_path);
}
