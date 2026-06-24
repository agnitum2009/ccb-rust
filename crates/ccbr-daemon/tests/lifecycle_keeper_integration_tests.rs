use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::models::lifecycle::{CcbdShutdownReport, CcbdStartupReport};
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use tempfile::TempDir;

const TRIGGER_START_COMMAND: &str = "start_command";
const TRIGGER_SHUTDOWN: &str = "shutdown";
const STATUS_OK: &str = "ok";
const ACTION_DAEMON_STARTED: &str = "daemon_started";
const EVENT_STARTED: &str = "started";
const EVENT_STOPPED: &str = "stopped";
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

    let report_path = app.layout.ccbrd_startup_report_path();
    let loaded: CcbdStartupReport = ccbr_storage::json::JsonStore::new()
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
        ccbr_daemon::models::api_models::common::API_VERSION
    );

    let recent = app.lifecycle.recent_reports(1);
    assert!(
        !recent.is_empty(),
        "lifecycle should record a started event"
    );
    let event = recent
        .iter()
        .find(|r| r.event == EVENT_STARTED)
        .expect("started event");
    assert_eq!(event.project_id, app.project_id());
    assert_eq!(event.details["generation"], 1);
    assert_eq!(event.details["socket_path"], app.socket_path());
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

    let report_path = app.layout.ccbrd_shutdown_report_path();
    let loaded: CcbdShutdownReport = ccbr_storage::json::JsonStore::new()
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

    let recent = app.lifecycle.recent_reports(2);
    assert!(
        recent.iter().any(|r| r.event == EVENT_STOPPED),
        "lifecycle should record a stopped event"
    );
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

    let report_path = app.layout.ccbrd_startup_report_path();
    let loaded: CcbdStartupReport = ccbr_storage::json::JsonStore::new()
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
