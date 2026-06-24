use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ccbr_cli::entry::run_cli;
use ccbr_cli::services::socket_path_for_project;
use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::socket_server::SocketServer;
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
use serde_json::{json, Value};
use tempfile::TempDir;

fn spawn_daemon(dir: &TempDir) -> (SocketServer, thread::JoinHandle<()>, PathBuf) {
    let app = CcbdApp::with_backend(
        dir.path(),
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    );
    let socket_path = app.socket_path();
    let app = Arc::new(Mutex::new(app));
    let server = SocketServer::new(&socket_path);
    let handle = server.listen(app, || {}).expect("server can listen");
    thread::sleep(Duration::from_millis(50));
    (server, handle, PathBuf::from(socket_path))
}

fn run(args: &[&str]) -> i32 {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    // Integration tests run the CLI from the source checkout against temp
    // project directories, so bypass the source-runtime guard.
    std::env::set_var("CCBR_SOURCE_RUNTIME_OK", "1");
    run_cli(&owned)
}

#[test]
fn test_cli_maintenance_tick() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    let code = run(&["--project", project, "maintenance", "tick"]);
    assert_eq!(code, 0, "maintenance tick should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_logs_no_session() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    let code = run(&["--project", project, "logs", "claude"]);
    assert_eq!(code, 0, "logs should succeed even without session file");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_logs_with_session() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "codex"]);

    // Write a synthetic codex session log so the handler has something to tail.
    let session_path = dir.path().join("codex-session.jsonl");
    std::fs::write(&session_path, "first line\nsecond line\nthird line\n").unwrap();

    let code = run(&["--project", project, "logs", "codex"]);
    assert_eq!(code, 0, "logs should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_cleanup() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    let code = run(&["--project", project, "cleanup"]);
    assert_eq!(code, 0, "cleanup should succeed");

    server.shutdown();
    handle.join().unwrap();
}

fn write_maintenance_config(dir: &TempDir) {
    let ccbr_dir = dir.path().join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(
        ccbr_dir.join("ccbr.config"),
        "demo:codex\n\n[maintenance.heartbeat]\nenabled = true\nassessor = \"demo\"\ninterval_s = 900\nmin_interval_s = 90\nstartup_ensure = true\n",
    )
    .unwrap();
}

#[test]
fn test_cli_maintenance_status() {
    let dir = TempDir::new().unwrap();
    write_maintenance_config(&dir);
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let code = run(&["--project", project, "maintenance", "status"]);
    assert_eq!(code, 0, "maintenance status should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_maintenance_tick_healthy_writes_status() {
    let dir = TempDir::new().unwrap();
    write_maintenance_config(&dir);
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let code = run(&["--project", project, "maintenance", "tick"]);
    assert_eq!(code, 0, "maintenance tick should succeed");

    let layout = ccbr_storage::paths::PathLayout::new(project);
    let project_id = layout.project_id().to_string();
    let store = ccbr_heartbeat::MaintenanceHeartbeatStore::new(layout, &project_id).unwrap();
    assert_eq!(
        store.load_status().state,
        ccbr_heartbeat::store::ReadState::Ok
    );

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_maintenance_schedule() {
    let dir = TempDir::new().unwrap();
    write_maintenance_config(&dir);
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let code = run(&[
        "--project",
        project,
        "maintenance",
        "schedule",
        "--after",
        "120",
        "--reason",
        "test_schedule",
    ]);
    assert_eq!(code, 0, "maintenance schedule should succeed");

    let layout = ccbr_storage::paths::PathLayout::new(project);
    let project_id = layout.project_id().to_string();
    let store = ccbr_heartbeat::MaintenanceHeartbeatStore::new(layout, &project_id).unwrap();
    assert_eq!(
        store.load_schedule().state,
        ccbr_heartbeat::store::ReadState::Ok
    );

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_maintenance_runner_due_tick() {
    let dir = TempDir::new().unwrap();
    write_maintenance_config(&dir);
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(project);
    let project_id = layout.project_id().to_string();

    // Write a schedule that is already due so the runner ticks immediately.
    let schedule_path = layout.ccbrd_maintenance_heartbeat_schedule_path();
    std::fs::create_dir_all(schedule_path.parent().unwrap()).unwrap();
    std::fs::write(
        &schedule_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 1,
            "record_type": "maintenance_heartbeat_schedule",
            "project_id": project_id,
            "next_run_at": "2020-01-01T00:00:00Z",
            "reason": "test_due",
            "updated_at": "2020-01-01T00:00:00Z",
            "updated_by": "test",
        }))
        .unwrap(),
    )
    .unwrap();

    let runner_code = run(&[
        "--project",
        project,
        "maintenance",
        "runner",
        "--max-iterations",
        "1",
        "--sleep-cap",
        "0s",
        "--no-dispatch",
    ]);
    assert_eq!(runner_code, 0, "maintenance runner should succeed");

    let store = ccbr_heartbeat::MaintenanceHeartbeatStore::new(layout, &project_id).unwrap();
    assert_eq!(
        store.load_status().state,
        ccbr_heartbeat::store::ReadState::Ok
    );

    server.shutdown();
    handle.join().unwrap();
}

/// Spawn a minimal fake ccbrd that responds to Unix socket RPCs in the format
/// used by `ccbr_cli::services::UnixDaemonClient` (`{"method":..., "params":...}`).
fn spawn_fake_daemon(
    socket_path: &std::path::Path,
    responses: HashMap<String, Value>,
) -> (thread::JoinHandle<()>, mpsc::Receiver<()>) {
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (tx, rx) = mpsc::channel();
    let path = socket_path.to_path_buf();
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let mut handled = 0;
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut reader = BufReader::new(&stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let req: Value = serde_json::from_str(&line).unwrap_or_default();
                        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
                        let result = responses
                            .get(method)
                            .cloned()
                            .unwrap_or_else(|| json!({"status": "ok"}));
                        let resp = json!({"ok": true, "result": result});
                        let _ = stream.write_all(resp.to_string().as_bytes());
                        let _ = stream.write_all(b"\n");
                    }
                    handled += 1;
                    // The tick path calls project_view then submit.
                    if handled >= 2 {
                        let _ = tx.send(());
                        return;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > Duration::from_secs(5) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return,
            }
        }
    });
    // Wait until the socket file exists before returning.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    (handle, rx)
}

#[test]
fn test_cli_maintenance_tick_concern_submits_activation() {
    let dir = TempDir::new().unwrap();
    write_maintenance_config(&dir);

    let project = dir.path().to_str().unwrap();
    let socket_path = socket_path_for_project(dir.path());
    std::fs::create_dir_all(std::path::Path::new(&socket_path).parent().unwrap()).unwrap();

    let mut responses = HashMap::new();
    // A mounted daemon with a degraded agent produces a concern evaluation.
    responses.insert(
        "project_view".to_string(),
        json!({
            "view": {
                "ccbrd": {"state": "mounted", "health": "healthy", "generation": 1},
                "agents": [
                    {"name": "demo", "activity_state": "offline"},
                ],
                "comms": [],
            },
            "cache": {"generated_at": "2026-06-20T00:00:00Z"},
        }),
    );
    responses.insert(
        "submit".to_string(),
        json!({"job_id": "maint-job-123", "status": "submitted"}),
    );

    let (handle, done) = spawn_fake_daemon(std::path::Path::new(&socket_path), responses);

    let code = run(&["--project", project, "maintenance", "tick", "--force"]);
    assert_eq!(code, 0, "maintenance tick should succeed");

    // Wait for the fake daemon to handle both RPCs, but do not block forever.
    let _ = done.recv_timeout(Duration::from_secs(5));
    handle.join().unwrap();

    let layout = ccbr_storage::paths::PathLayout::new(project);
    let project_id = layout.project_id().to_string();
    let store = ccbr_heartbeat::MaintenanceHeartbeatStore::new(layout, &project_id).unwrap();
    let status = store.load_status();
    assert_eq!(status.state, ccbr_heartbeat::store::ReadState::Ok);
    let record = status.value.expect("status record should be present");
    assert_eq!(record.last_tick_status.as_deref(), Some("concern"));
    assert_eq!(record.last_activation_status.as_deref(), Some("submitted"));
}
