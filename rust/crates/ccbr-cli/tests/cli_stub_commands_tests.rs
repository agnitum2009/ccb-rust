use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ccbr_cli::entry::run_cli;
use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::socket_server::SocketServer;
use ccbr_daemon::start_flow::service::StartFlowService;
use ccbr_daemon::stop_flow::service::StopFlowService;
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
fn test_cli_doctor_config_validate_and_pend() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);
    let project = dir.path().to_str().unwrap();

    run(&["--project", project, "start", "claude"]);

    assert_eq!(
        run(&["--project", project, "doctor"]),
        0,
        "doctor should succeed"
    );
    assert_eq!(
        run(&["--project", project, "config", "validate"]),
        0,
        "config validate should succeed"
    );
    assert_eq!(
        run(&["--project", project, "pend", "--queue", "claude"]),
        0,
        "pend --queue should succeed"
    );
    assert_eq!(
        run(&["--project", project, "wait-any", "--timeout=0.01", "msg-1"]),
        0,
        "wait-any should use mailbox wait"
    );
    assert_eq!(
        run(&["--project", project, "wait-all", "--timeout=0.01", "msg-1"]),
        0,
        "wait-all should use mailbox wait"
    );
    assert_eq!(
        run(&[
            "--project",
            project,
            "wait-quorum",
            "--timeout=0.01",
            "1",
            "msg-1",
        ]),
        0,
        "wait-quorum should use mailbox wait"
    );

    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_fault_repair_tools_and_roles() {
    let dir = TempDir::new().unwrap();
    let (server, handle, _socket) = spawn_daemon(&dir);
    let project = dir.path().to_str().unwrap();

    // roles install/update/sync delegate to the external `agent-roles` CLI.
    // Inject a mock so the real code path runs against a deterministic fake.
    let mock = dir.path().join("mock-agent-roles");
    std::fs::write(
        &mock,
        "#!/bin/sh\nprintf '{\"path\":\"/tmp/mock-role\",\"status\":\"ok\",\"roles\":[]}\\n'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&mock).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&mock, perms).unwrap();
    }
    std::env::set_var("AGENT_ROLES_CLI", &mock);

    run(&["--project", project, "start", "claude"]);

    assert_eq!(
        run(&["--project", project, "fault", "list"]),
        0,
        "fault list should succeed"
    );
    assert_eq!(
        run(&[
            "--project",
            project,
            "fault",
            "arm",
            "claude",
            "--task-id",
            "task-1",
            "--reason",
            "api_error",
            "--count",
            "2",
        ]),
        0,
        "fault arm should succeed"
    );
    assert_eq!(
        run(&["--project", project, "fault", "clear", "all"]),
        0,
        "fault clear should succeed"
    );
    assert_eq!(
        run(&["--project", project, "repair", "ack", "claude", "evt-1"]),
        0,
        "repair ack should succeed"
    );
    assert_ne!(
        run(&["--project", project, "repair", "retry", "claude"]),
        0,
        "repair retry should reject an unknown concrete target"
    );
    assert_ne!(
        run(&["--project", project, "repair", "resubmit", "msg-1"]),
        0,
        "repair resubmit should reject an unknown message id"
    );
    assert_eq!(
        run(&["--project", project, "tools", "doctor", "neovim"]),
        0,
        "tools doctor should succeed"
    );
    assert_eq!(
        run(&["--project", project, "tools", "install", "neovim"]),
        0,
        "tools install should succeed"
    );
    assert_eq!(
        run(&["--project", project, "roles", "list"]),
        0,
        "roles list should succeed"
    );
    // `roles update`/`install`/`sync` delegate to the external `agent-roles`
    // CLI (mocked above); `roles add` requires an installed role + project
    // config and is covered by the ccbr-agents rolepack unit tests.
    assert_eq!(
        run(&["--project", project, "roles", "update", "agentroles.archi"]),
        0,
        "roles update should succeed"
    );

    std::env::remove_var("AGENT_ROLES_CLI");
    server.shutdown();
    handle.join().unwrap();
}

#[test]
fn test_cli_version_update_uninstall_reinstall() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().to_str().unwrap();

    assert_eq!(
        run(&["--project", project, "version"]),
        0,
        "version should succeed"
    );
    assert_eq!(
        run(&["--project", project, "update"]),
        0,
        "update should succeed"
    );
    assert_eq!(
        run(&["--project", project, "uninstall"]),
        0,
        "uninstall should succeed"
    );
    assert_eq!(
        run(&["--project", project, "reinstall"]),
        0,
        "reinstall should succeed"
    );
}

#[test]
fn test_cli_mobile_parser_receipts() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().to_str().unwrap();

    assert_eq!(
        run(&[
            "--project",
            project,
            "mobile",
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--public-url",
            "https://example.test",
            "--route-provider",
            "relay",
        ]),
        0,
        "mobile serve should parse as mobile, not start an agent"
    );
    assert_eq!(
        run(&["--project", project, "mobile", "devices"]),
        0,
        "mobile devices should parse"
    );
    assert_eq!(
        run(&["--project", project, "mobile", "revoke", "device-1"]),
        0,
        "mobile revoke should parse"
    );
    assert_eq!(
        run(&["--project", project, "mobile"]),
        2,
        "mobile without action should fail like Python"
    );
    assert_eq!(
        run(&[
            "--project",
            project,
            "mobile",
            "serve",
            "--route-provider",
            "bad",
        ]),
        2,
        "mobile serve rejects unknown route providers"
    );
}
