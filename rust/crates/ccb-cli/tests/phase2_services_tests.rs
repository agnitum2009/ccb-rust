//! End-to-end tests for the phase2 dispatch path using a fake daemon socket.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ccb_cli::ccbd::CcbdClient;
use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::{
    ParsedCommand, ParsedKillCommand, ParsedLogsCommand, ParsedMaintenanceCommand,
    ParsedPingCommand, ParsedPsCommand, ParsedReloadCommand, ParsedRestartCommand,
    ParsedStartCommand, ParsedWaitCommand,
};
use ccb_cli::phase2_runtime::dispatch::dispatch;
use ccb_cli::phase2_services::DaemonPhase2Services;
use serde_json::{json, Value};
use tempfile::TempDir;

fn make_project() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo");
    let ccb_dir = project_root.join(".ccb");
    std::fs::create_dir_all(&ccb_dir).unwrap();
    std::fs::write(ccb_dir.join("ccb.config"), "agent1:codex\n").unwrap();
    (tmp, project_root)
}

fn build_context(project_root: PathBuf, command: ParsedCommand) -> CliContext {
    CliContextBuilder::new(command)
        .cwd(project_root)
        .build()
        .expect("build context")
}

fn fake_daemon_server<P: AsRef<Path>>(
    socket_path: P,
    responses: HashMap<String, Value>,
) -> (JoinHandle<()>, mpsc::Receiver<()>) {
    let listener = UnixListener::bind(socket_path.as_ref()).unwrap();
    listener.set_nonblocking(true).expect("set_nonblocking");
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let start = Instant::now();
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut reader = BufReader::new(&stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let req: Value = serde_json::from_str(&line).unwrap_or_default();
                        let op = req.get("op").and_then(|v| v.as_str()).unwrap_or("");
                        let payload = responses.get(op).cloned().unwrap_or_else(
                            || json!({"status": "ok", "note": format!("canned response for {op}")}),
                        );
                        let response = json!({"ok": true, "payload": payload});
                        let _ = stream.write_all(response.to_string().as_bytes());
                        let _ = stream.write_all(b"\n");
                    }
                    let _ = tx.send(());
                    return;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > Duration::from_secs(2) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    return;
                }
            }
        }
    });
    (handle, rx)
}

fn run_phase2(
    project_root: PathBuf,
    command: Value,
    responses: HashMap<String, Value>,
) -> (i32, String) {
    let socket_path = project_root.join("ccbd.sock");
    let (server, server_done) = fake_daemon_server(&socket_path, responses);

    let client = CcbdClient::new(&socket_path);
    let services = DaemonPhase2Services::with_client(client);
    let parsed = match command.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
        "ps" => ParsedCommand::Ps(ParsedPsCommand::new(None)),
        "ping" => ParsedCommand::Ping(ParsedPingCommand::new(
            None,
            command
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("ccbd")
                .into(),
        )),
        "kill" => ParsedCommand::Kill(ParsedKillCommand {
            project: None,
            force: false,
            kind: "kill".into(),
        }),
        "start" => ParsedCommand::Start(ParsedStartCommand::new(
            None,
            vec!["agent1".into()],
            false,
            false,
        )),
        "restart" => ParsedCommand::Restart(ParsedRestartCommand::new(None, "agent1".into())),
        "logs" => ParsedCommand::Logs(ParsedLogsCommand::new(None, "agent1".into())),
        "maintenance" => ParsedCommand::Maintenance(ParsedMaintenanceCommand {
            project: None,
            action: "status".into(),
            args: vec![],
            kind: "maintenance".into(),
        }),
        "reload" => ParsedCommand::Reload(ParsedReloadCommand {
            project: None,
            dry_run: false,
            kind: "reload".into(),
        }),
        "wait" => ParsedCommand::Wait(ParsedWaitCommand::new(None, "wait".into(), "ccbd".into())),
        _ => ParsedCommand::Ps(ParsedPsCommand::new(None)),
    };
    let context = build_context(project_root, parsed);
    let mut out = Vec::new();
    let code = dispatch(&context, &command, &mut out, &services);
    let output = String::from_utf8(out).unwrap();

    let _ = server_done.recv_timeout(Duration::from_secs(3));
    let _ = server.join();
    (code, output)
}

#[test]
fn test_phase2_ping_via_daemon_services() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "ping".to_string(),
        json!({"pong": true, "target": "ccbd", "status": "ok"}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "ping", "target": "ccbd"}),
        responses,
    );
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
    let (code, output) = run_phase2(project_root, json!({"kind": "ps"}), HashMap::new());
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("project_id:"),
        "output should contain project_id: {}",
        output
    );
}

#[test]
fn test_phase2_kill_forwards_stop_all_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "stop-all".to_string(),
        json!({"status": "ok", "killed": ["agent1"]}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "kill", "force": false}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("status") || output.contains("killed"),
        "output should reflect kill result: {}",
        output
    );
}

#[test]
fn test_phase2_start_forwards_start_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "start".to_string(),
        json!({"status": "ok", "started": ["agent1"], "actions": ["launch"] }),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "start", "agent_names": ["agent1"], "restore": false}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("agent1") || output.contains("launch"),
        "output should reflect start result: {}",
        output
    );
}

#[test]
fn test_phase2_restart_forwards_project_restart_agent_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "project_restart_agent".to_string(),
        json!({"restart_status": "ok", "agent_name": "agent1"}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "restart", "agent_name": "agent1"}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("agent1"),
        "output should mention agent name: {}",
        output
    );
}

#[test]
fn test_phase2_logs_forwards_logs_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "logs".to_string(),
        json!({
            "agent_name": "agent1",
            "entries": [
                {"source": "stdout", "path": "agent1.log", "lines": ["hello world"]}
            ]
        }),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "logs", "agent_name": "agent1"}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("hello world"),
        "output should contain log line: {}",
        output
    );
}

#[test]
fn test_phase2_maintenance_forwards_maintenance_tick_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "maintenance_tick".to_string(),
        json!({"maintenance_status": "ok", "concerns": []}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "maintenance", "action": "status"}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("ok") || output.contains("maintenance"),
        "output should reflect maintenance status: {}",
        output
    );
}

#[test]
fn test_phase2_reload_forwards_project_reload_config_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "project_reload_config".to_string(),
        json!({"status": "ok", "changes": ["agent1"]}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "reload", "dry_run": false}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("ok") || output.contains("agent1"),
        "output should reflect reload result: {}",
        output
    );
}

#[test]
fn test_phase2_wait_forwards_watch_rpc() {
    let (_tmp, project_root) = make_project();
    let mut responses = HashMap::new();
    responses.insert(
        "watch".to_string(),
        json!({"target": "ccbd", "cursor": 1, "terminal": true, "lines": []}),
    );
    let (code, output) = run_phase2(
        project_root,
        json!({"kind": "wait", "target": "ccbd"}),
        responses,
    );
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
}

#[test]
fn test_phase2_doctor_bundle_uses_local_service() {
    let (_tmp, project_root) = make_project();
    // Diagnostic bundle only needs local context; no daemon RPC.
    let context = build_context(
        project_root.clone(),
        ParsedCommand::Doctor(ccb_cli::models::ParsedDoctorCommand {
            project: None,
            bundle: true,
            output_path: None,
            storage: false,
            json_output: false,
            kind: "doctor".into(),
        }),
    );
    let services = DaemonPhase2Services::from_context(&context);
    let command = json!({"kind": "doctor", "bundle": true, "output_path": null});
    let mut out = Vec::new();
    let code = dispatch(&context, &command, &mut out, &services);
    let output = String::from_utf8(out).unwrap();
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("bundle_path") || output.contains("ccb-support"),
        "output should reflect bundle creation: {}",
        output
    );
}

#[test]
fn test_phase2_config_validate_uses_local_config_loader() {
    let (_tmp, project_root) = make_project();
    let context = build_context(
        project_root.clone(),
        ParsedCommand::ConfigValidate(ccb_cli::models::ParsedConfigValidateCommand {
            project: None,
            kind: "config-validate".into(),
        }),
    );
    let services = DaemonPhase2Services::from_context(&context);
    let command = json!({"kind": "config-validate"});
    let mut out = Vec::new();
    let code = dispatch(&context, &command, &mut out, &services);
    let output = String::from_utf8(out).unwrap();
    assert_eq!(code, 0, "exit code should be 0, output: {}", output);
    assert!(
        output.contains("ok") || output.contains("agent1"),
        "output should reflect valid config: {}",
        output
    );
}
