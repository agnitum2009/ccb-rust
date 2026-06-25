use ccb_daemon::app::CcbdApp;
use ccb_daemon::models::lifecycle::{CcbdShutdownReport, CcbdStartupReport};
use ccb_daemon::start_flow::service::StartFlowService;
use ccb_daemon::stop_flow::service::StopFlowService;
use tempfile::TempDir;

const TRIGGER_START_COMMAND: &str = "start_command";
const TRIGGER_SHUTDOWN: &str = "shutdown";
const STATUS_OK: &str = "ok";
const ACTION_DAEMON_STARTED: &str = "daemon_started";
const REASON_SHUTDOWN: &str = "shutdown";

fn stub_app(dir: &TempDir) -> CcbdApp {
    CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    )
}

#[test]
fn test_start_acquires_ownership_and_writes_startup_report() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    app.start().unwrap();

    let record = app
        .ownership
        .current()
        .expect("ownership should be held after start");
    assert_eq!(record.owner_pid, std::process::id());
    assert_eq!(record.instance_id, app.daemon_instance_id());
    assert_eq!(record.generation, 1);

    let report_path = app.layout.ccbd_startup_report_path();
    let loaded: CcbdStartupReport = ccb_storage::json::JsonStore::new()
        .load(&report_path)
        .expect("startup report should be readable");

    assert_eq!(loaded.project_id, app.project_id());
    assert_eq!(loaded.trigger, TRIGGER_START_COMMAND);
    assert_eq!(loaded.status, STATUS_OK);
    assert!(loaded
        .actions_taken
        .contains(&ACTION_DAEMON_STARTED.to_string()));
    assert_eq!(
        loaded.api_version,
        ccb_daemon::models::api_models::common::API_VERSION
    );

    let lifecycle = app
        .lifecycle
        .load()
        .expect("lifecycle should be persisted after start");
    assert_eq!(lifecycle.project_id, app.project_id());
    assert_eq!(lifecycle.phase, "mounted");
    assert_eq!(lifecycle.desired_state, "running");
    assert_eq!(lifecycle.generation, 1);
    assert_eq!(
        lifecycle.socket_path.as_deref(),
        Some(app.socket_path().as_str())
    );
}

#[test]
fn test_shutdown_writes_shutdown_report_and_releases_ownership() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    app.start().unwrap();
    app.shutdown().unwrap();

    assert!(
        app.ownership.current().is_none(),
        "ownership should be released after shutdown"
    );

    let report_path = app.layout.ccbd_shutdown_report_path();
    let loaded: CcbdShutdownReport = ccb_storage::json::JsonStore::new()
        .load(&report_path)
        .expect("shutdown report should be readable");

    let last = app
        .last_shutdown_report
        .as_ref()
        .expect("last_shutdown_report should be set");
    assert_eq!(loaded.project_id, app.project_id());
    assert_eq!(loaded.trigger, TRIGGER_SHUTDOWN);
    assert_eq!(loaded.status, last.status);
    assert_eq!(loaded.daemon_generation, Some(1));
    assert_eq!(loaded.reason, Some(REASON_SHUTDOWN.to_string()));

    let lifecycle = app
        .lifecycle
        .load()
        .expect("lifecycle should be persisted after shutdown");
    assert_eq!(lifecycle.project_id, app.project_id());
    assert_eq!(lifecycle.phase, "unmounted");
    assert_eq!(lifecycle.desired_state, "stopped");
    assert!(lifecycle.owner_pid.is_none());
}

#[test]
fn test_start_bumps_generation_on_reacquire() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    app.start().unwrap();
    let first = app.ownership.current().unwrap().clone();
    assert_eq!(first.generation, 1);

    app.ownership.release();
    assert!(app.ownership.current().is_none());

    app.start().unwrap();
    let second = app.ownership.current().unwrap();
    assert_eq!(second.generation, 2);
    assert_eq!(second.instance_id, app.daemon_instance_id());
    assert_eq!(second.instance_id, first.instance_id);
    assert_eq!(second.owner_pid, std::process::id());
}

#[test]
fn test_startup_report_roundtrips() {
    let dir = TempDir::new().unwrap();
    let mut app = stub_app(&dir);

    app.start().unwrap();

    let report_path = app.layout.ccbd_startup_report_path();
    let loaded: CcbdStartupReport = ccb_storage::json::JsonStore::new()
        .load(&report_path)
        .expect("startup report should be readable");

    let last = app
        .last_startup_report
        .as_ref()
        .expect("last_startup_report should be set");
    assert_eq!(loaded.project_id, last.project_id);
    assert_eq!(loaded.trigger, last.trigger);
    assert_eq!(loaded.status, last.status);
    assert_eq!(loaded.actions_taken, last.actions_taken);
    assert_eq!(loaded.agent_results.len(), last.agent_results.len());
    assert_eq!(loaded.failure_reason, last.failure_reason);
    assert_eq!(loaded.api_version, last.api_version);
}
