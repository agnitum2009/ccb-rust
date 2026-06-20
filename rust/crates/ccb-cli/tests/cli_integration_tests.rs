use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ccb_cli::entry::run_cli;
use ccb_cli::services::DaemonClient;
use ccb_daemon::app::CcbdApp;
use ccb_daemon::socket_server::SocketServer;
use ccb_daemon::start_flow::service::StartFlowService;
use ccb_daemon::stop_flow::service::StopFlowService;
use tempfile::TempDir;

use ccb_provider_core::protocol::{self, BEGIN_PREFIX, REQ_ID_PREFIX};

fn spawn_daemon(dir: &TempDir) -> (SocketServer, thread::JoinHandle<()>, PathBuf) {
    spawn_daemon_with_backend(
        dir,
        StartFlowService::with_stub(),
        StopFlowService::with_stub(),
    )
}

fn spawn_daemon_with_backend(
    dir: &TempDir,
    start_flow: StartFlowService,
    stop_flow: StopFlowService,
) -> (SocketServer, thread::JoinHandle<()>, PathBuf) {
    let app = CcbdApp::with_backend(dir.path(), start_flow, stop_flow);
    let socket_path = app.socket_path();
    let app = Arc::new(Mutex::new(app));
    let server = SocketServer::new(&socket_path);
    let handle = server.listen(app, || {}).expect("server can listen");
    // Give the server a moment to bind.
    thread::sleep(Duration::from_millis(50));
    (server, handle, PathBuf::from(socket_path))
}

fn spawn_daemon_real(dir: &TempDir) -> (SocketServer, thread::JoinHandle<()>, PathBuf) {
    spawn_daemon_with_backend(
        dir,
        StartFlowService::with_tmux(),
        StopFlowService::with_tmux(),
    )
}

fn run(args: &[&str]) -> i32 {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    // Integration tests run the CLI from the source checkout against temp
    // project directories, so bypass the source-runtime guard.
    std::env::set_var("CCB_SOURCE_RUNTIME_OK", "1");
    run_cli(&owned)
}

#[test]
fn test_cli_ping_daemon() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let code = run(&["--project", dir.path().to_str().unwrap(), "ping", "ccbd"]);
    assert_eq!(code, 0, "ping ccbd should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_start_status_stop() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();

    let start_code = run(&["--project", project, "start", "claude", "gemini"]);
    assert_eq!(start_code, 0, "start should succeed");

    let status_code = run(&["--project", project, "status"]);
    assert_eq!(status_code, 0, "status should succeed");

    let stop_code = run(&["--project", project, "stop"]);
    assert_eq!(stop_code, 0, "stop should succeed");

    let status_after = run(&["--project", project, "status"]);
    assert_eq!(status_after, 0, "status after stop should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_ask_submission() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon_real(&dir);

    // Point the claude provider to a shell so the test does not require
    // the real claude binary to be installed.
    std::env::set_var("CLAUDE_START_CMD", "sh");

    let project = dir.path().to_str().unwrap();

    let start_code = run(&["--project", project, "start", "claude"]);
    assert_eq!(start_code, 0, "start should succeed");

    let ask_code = run(&[
        "--project",
        project,
        "ask",
        "claude",
        "--from",
        "user",
        "hello",
    ]);
    assert_eq!(ask_code, 0, "ask should succeed");

    // Clean up the real tmux server created by the daemon.
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

#[test]
fn test_cli_attach() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let code = run(&["--project", project, "attach", "external-agent"]);
    assert_eq!(code, 0, "attach should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_shutdown() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    let code = run(&["--project", project, "shutdown"]);
    assert_eq!(code, 0, "shutdown should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_stop_all_force() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude", "gemini"]);

    let code = run(&["--project", project, "stop-all", "--force"]);
    assert_eq!(code, 0, "stop-all --force should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_queue_trace_cancel() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    let queue_code = run(&["--project", project, "queue", "claude"]);
    assert_eq!(queue_code, 0, "queue should succeed");

    let trace_code = run(&["--project", project, "trace", "claude"]);
    assert_eq!(trace_code, 0, "trace should succeed");

    let cancel_code = run(&["--project", project, "cancel", "job-does-not-exist"]);
    assert_eq!(cancel_code, 0, "cancel should succeed");

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_inbox_ack_reload_restart_clear() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    assert_eq!(
        run(&["--project", project, "inbox", "claude"]),
        0,
        "inbox should succeed"
    );
    assert_eq!(
        run(&["--project", project, "ack", "claude", "evt-1"]),
        0,
        "ack should succeed"
    );
    assert_eq!(
        run(&["--project", project, "reload", "--dry-run"]),
        0,
        "reload should succeed"
    );
    assert_eq!(
        run(&["--project", project, "restart", "claude"]),
        0,
        "restart should succeed"
    );
    assert_eq!(
        run(&["--project", project, "clear", "claude"]),
        0,
        "clear should succeed"
    );

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_watch_wait_maintenance() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);

    let project = dir.path().to_str().unwrap();
    run(&["--project", project, "start", "claude"]);

    assert_eq!(
        run(&["--project", project, "watch", "claude"]),
        0,
        "watch should succeed"
    );
    assert_eq!(
        run(&["--project", project, "wait", "ccbd"]),
        0,
        "wait ccbd should succeed"
    );
    assert_eq!(
        run(&["--project", project, "maintenance", "status"]),
        0,
        "maintenance status should succeed"
    );

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_ask_drives_execution_service() {
    let dir = TempDir::new().unwrap();
    let ccb_dir = dir.path().join(".ccb");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    std::fs::write(
        ccb_dir.join("ccb.config"),
        r#"version = 2
default_agents = ["codex"]

[agents.codex]
provider = "codex"
target = "codex"

[windows]
main = "codex:codex"
"#,
    )
    .unwrap();

    std::env::set_var("CODEX_START_CMD", "sh");

    let (server, handle, socket) = spawn_daemon_real(&dir);
    let project = dir.path().to_str().unwrap();

    let start_code = run(&["--project", project, "start", "codex"]);
    assert_eq!(start_code, 0, "start should succeed");

    let ask_code = run(&["--project", project, "ask", "codex", "hello"]);
    assert_eq!(ask_code, 0, "ask should succeed");

    // Find the job id so we can synthesize a matching codex session log.
    let client = ccb_cli::services::UnixDaemonClient::new(socket.to_str().unwrap());
    let trace = client
        .call("trace", serde_json::json!({"target": "codex"}))
        .unwrap();
    let jobs = trace
        .get("jobs")
        .and_then(|v| v.as_array())
        .expect("trace should return jobs");
    assert!(!jobs.is_empty(), "codex should have at least one job");
    let job_id = jobs[0]
        .get("job_id")
        .and_then(|v| v.as_str())
        .expect("job should have an id")
        .to_string();
    assert_ne!(
        jobs[0]
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        "accepted",
        "job should be running after execution.start"
    );

    // Write a synthetic codex session log that makes the adapter emit a
    // terminal decision on the next heartbeat poll.
    let req_id = protocol::make_req_id(&job_id);
    let request_anchor = format!("{}{}>>", BEGIN_PREFIX, req_id);
    let user_text = format!("{} {}", REQ_ID_PREFIX, request_anchor);
    let session_path = dir.path().join("codex-session.jsonl");
    let session_content = format!(
        r#"{{"type":"event_msg","timestamp":"2026-06-13T09:06:34Z","payload":{{"type":"user_message","message":"{}"}}}}
{{"type":"event_msg","timestamp":"2026-06-13T09:06:35Z","payload":{{"type":"task_complete","last_agent_message":"done"}}}}
"#,
        user_text.replace('"', "\\\"")
    );
    std::fs::write(&session_path, &session_content).unwrap();

    // Wait for the daemon heartbeat to poll the execution service.
    thread::sleep(Duration::from_millis(1200));

    let result = client
        .call("trace", serde_json::json!({"target": "codex"}))
        .unwrap();
    let jobs = result
        .get("jobs")
        .and_then(|v| v.as_array())
        .expect("trace should return jobs");
    assert!(!jobs.is_empty(), "codex should have at least one job");
    let status = jobs[0]
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    assert_eq!(
        status, "completed",
        "execution poll should have completed the job, got {status}"
    );
    assert!(
        jobs[0].get("terminal_decision").is_some(),
        "completed job should carry a terminal decision"
    );

    // Clean up the real tmux server created by the daemon.
    let tmux_sock = dir.path().join(".ccb").join("ccbd").join("tmux.sock");
    if tmux_sock.exists() {
        let _ = std::process::Command::new("tmux")
            .args(["-S", tmux_sock.to_str().unwrap(), "kill-server"])
            .output();
    }

    std::env::remove_var("CODEX_START_CMD");

    server.shutdown();
    handle.join().unwrap();
}
