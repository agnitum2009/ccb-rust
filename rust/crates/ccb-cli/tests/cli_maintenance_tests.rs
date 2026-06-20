use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ccb_cli::entry::run_cli;
use ccb_daemon::app::CcbdApp;
use ccb_daemon::socket_server::SocketServer;
use ccb_daemon::start_flow::service::StartFlowService;
use ccb_daemon::stop_flow::service::StopFlowService;
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
    std::env::set_var("CCB_SOURCE_RUNTIME_OK", "1");
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
