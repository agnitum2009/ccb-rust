//! End-to-end smoke test for the Rust CLI.
//!
//! This test exercises the full CLI flow without requiring a real provider CLI.
//! It uses the real tmux backend but points the claude provider at a shell via
//! the `CLAUDE_START_CMD` environment variable, so it can run in CI as long as
//! tmux is installed.

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
        StartFlowService::with_tmux(),
        StopFlowService::with_tmux(),
    );
    let socket_path = app.socket_path();
    let app = Arc::new(Mutex::new(app));
    let server = SocketServer::new(&socket_path);
    let handle = server.listen(app, || {}).expect("server can listen");
    // Give the server a moment to bind before the CLI connects.
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
fn test_cli_start_ask_kill_smoke() {
    // Skip the test when tmux is not available (e.g., minimal CI images).
    if std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("tmux not available; skipping smoke test");
        return;
    }

    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);
    let project = dir.path().to_str().unwrap();

    // Point the claude provider at a shell so the test does not require the
    // real claude binary to be installed.
    std::env::set_var("CLAUDE_START_CMD", "sh");

    // Start the claude provider.
    let start_code = run(&["--project", project, "start", "claude"]);
    assert_eq!(start_code, 0, "start claude should succeed");

    // Submit a message to the claude provider.
    let ask_code = run(&[
        "--project",
        project,
        "ask",
        "claude",
        "--from",
        "user",
        "hello",
    ]);
    assert_eq!(ask_code, 0, "ask claude should succeed");

    // Verify project status reports the claude agent.
    let status_code = run(&["--project", project, "status"]);
    assert_eq!(status_code, 0, "status should succeed");

    // Stop / kill the project.
    let kill_code = run(&["--project", project, "kill", "--force"]);
    assert_eq!(kill_code, 0, "kill should succeed");

    // Status should still succeed after kill.
    let status_after = run(&["--project", project, "status"]);
    assert_eq!(status_after, 0, "status after kill should succeed");

    // Clean up the tmux server created by the daemon.
    let tmux_sock = dir.path().join(".ccb").join("ccbd").join("tmux.sock");
    if tmux_sock.exists() {
        let _ = std::process::Command::new("tmux")
            .args(["-S", tmux_sock.to_str().unwrap(), "kill-server"])
            .output();
    }

    std::env::remove_var("CLAUDE_START_CMD");

    server.shutdown();
    handle.join().unwrap();
}
