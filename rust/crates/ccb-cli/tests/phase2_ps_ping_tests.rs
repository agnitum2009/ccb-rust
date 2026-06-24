use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use ccb_cli::ccbd::CcbdClient;
use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::{ParsedCommand, ParsedPingCommand, ParsedPsCommand};
use ccb_cli::phase2_runtime::dispatch::dispatch;
use ccb_cli::phase2_services::DaemonPhase2Services;
use serde_json::json;
use tempfile::TempDir;

fn build_context(project_root: PathBuf, command: ParsedCommand) -> CliContext {
    CliContextBuilder::new(command)
        .cwd(project_root)
        .build()
        .expect("build context")
}

fn make_project() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo");
    let ccb_dir = project_root.join(".ccbr");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    std::fs::write(ccb_dir.join("ccbr.config"), "agent1:codex\n").unwrap();
    (tmp, project_root)
}

#[test]
fn test_phase2_ping_via_daemon_services() {
    let (_tmp, project_root) = make_project();
    let socket_path = project_root.join("ccbd.sock");

    // Start a tiny Unix socket server that returns a canned ping response.
    let listener = UnixListener::bind(&socket_path).unwrap();
    let server_thread = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let response = r#"{"ok":true,"payload":{"pong":true,"target":"ccbd","status":"ok"}}"#;
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });

    let client = CcbdClient::new(&socket_path);
    let services = DaemonPhase2Services::with_client(client);
    let context = build_context(
        project_root,
        ParsedCommand::Ping(ParsedPingCommand::new(None, "ccbd".into())),
    );
    let command = json!({ "kind": "ping", "target": "ccbd" });
    let mut out = Vec::new();
    let code = dispatch(&context, &command, &mut out, &services);
    let output = String::from_utf8(out).unwrap();

    server_thread.join().unwrap();

    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("pong"),
        "output should contain pong: {}",
        output
    );
}

#[test]
fn test_phase2_ps_via_local_services() {
    let (_tmp, project_root) = make_project();
    let context = build_context(
        project_root.clone(),
        ParsedCommand::Ps(ParsedPsCommand::new(None)),
    );
    let services = DaemonPhase2Services::from_context(&context);
    let command = json!({ "kind": "ps" });
    let mut out = Vec::new();
    let code = dispatch(&context, &command, &mut out, &services);
    let output = String::from_utf8(out).unwrap();

    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("project_id:"),
        "output should contain project_id: {}",
        output
    );
    assert!(
        output.contains("agent:"),
        "output should contain agent: {}",
        output
    );
}
