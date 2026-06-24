//! Mirrors Python `test/test_provider_activity_hook_script.py`.

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn hook_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ccbr-provider-activity-hook"))
}

fn run_hook(
    runtime_dir: &std::path::Path,
    provider: &str,
    agent_name: &str,
    payload: Value,
    env: HashMap<String, String>,
) -> std::process::Output {
    let mut child = Command::new(hook_bin())
        .arg("--provider")
        .arg(provider)
        .arg("--project-id")
        .arg("project-1")
        .arg("--agent-name")
        .arg(agent_name)
        .arg("--runtime-dir")
        .arg(runtime_dir)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("hook binary can spawn");

    let stdin = child.stdin.take().expect("stdin is piped");
    let payload_text = serde_json::to_string(&payload).unwrap();
    std::thread::spawn(move || {
        let mut stdin = stdin;
        let _ = std::io::Write::write_all(&mut stdin, payload_text.as_bytes());
    });

    child.wait_with_output().expect("hook binary can finish")
}

#[test]
fn test_provider_activity_hook_writes_codex_active_snapshot() {
    let tmp = tempfile::TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let mut env = HashMap::new();
    env.insert("CCB_CALLER_ACTOR".into(), "agent2".into());
    env.insert(
        "CCB_CALLER_RUNTIME_DIR".into(),
        runtime_dir.to_string_lossy().into_owned(),
    );
    env.insert("CCB_SESSION_ID".into(), "ccbr-agent2-1".into());
    env.insert("TMUX_PANE".into(), "%42".into());

    let payload = serde_json::json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "codex-session-1",
        "turn_id": "turn-1",
        "prompt": "do not store me",
    });

    let output = run_hook(&runtime_dir, "codex", "agent2", payload, env);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activity_path = runtime_dir.join("activity.json");
    let activity: Value = serde_json::from_slice(&std::fs::read(&activity_path).unwrap()).unwrap();

    assert_eq!(activity["state"], "active");
    assert_eq!(activity["event_name"], "UserPromptSubmit");
    assert_eq!(activity["agent_name"], "agent2");
    assert_eq!(activity["ccbr_session_id"], "ccbr-agent2-1");
    assert_eq!(activity["pane_id"], "%42");
    assert_eq!(activity["provider_session_id"], "codex-session-1");
    assert_eq!(activity["provider_turn_id"], "turn-1");
    assert!(!activity.as_object().unwrap().contains_key("prompt"));
}

#[test]
fn test_provider_activity_hook_maps_claude_waiting_notification() {
    let tmp = tempfile::TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");

    let mut env = HashMap::new();
    env.insert("CCB_CALLER_ACTOR".into(), "agent3".into());
    env.insert(
        "CCB_CALLER_RUNTIME_DIR".into(),
        runtime_dir.to_string_lossy().into_owned(),
    );

    let payload = serde_json::json!({
        "hook_event_name": "Notification",
        "message": "Waiting for permission approval",
    });

    let output = run_hook(&runtime_dir, "claude", "agent3", payload, env);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activity: Value =
        serde_json::from_slice(&std::fs::read(runtime_dir.join("activity.json")).unwrap()).unwrap();
    assert_eq!(activity["state"], "pending");
    assert_eq!(activity["event_name"], "Notification");
}

#[test]
fn test_provider_activity_hook_exits_zero_without_writing_on_malformed_payload() {
    let tmp = tempfile::TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");

    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("CCB_CALLER_ACTOR".into(), "agent2".into());
    env.insert(
        "CCB_CALLER_RUNTIME_DIR".into(),
        runtime_dir.to_string_lossy().into_owned(),
    );

    let mut child = Command::new(hook_bin())
        .arg("--provider")
        .arg("codex")
        .arg("--project-id")
        .arg("project-1")
        .arg("--agent-name")
        .arg("agent2")
        .arg("--runtime-dir")
        .arg(&runtime_dir)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stdin = child.stdin.take().unwrap();
    std::thread::spawn(move || {
        let mut stdin = stdin;
        let _ = std::io::Write::write_all(&mut stdin, b"{not-json");
    });

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!runtime_dir.join("activity.json").exists());
}

#[test]
fn test_provider_activity_hook_maps_error_payload_to_failed_without_secret() {
    let tmp = tempfile::TempDir::new().unwrap();
    let runtime_dir = tmp.path().join("runtime");

    let mut env = HashMap::new();
    env.insert("CCB_CALLER_ACTOR".into(), "agent2".into());
    env.insert(
        "CCB_CALLER_RUNTIME_DIR".into(),
        runtime_dir.to_string_lossy().into_owned(),
    );

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "error": {
            "type": "provider_api_error",
            "code": "model_not_found",
            "message": "model unavailable",
            "api_key": "must-not-leak",
        },
    });

    let output = run_hook(&runtime_dir, "codex", "agent2", payload, env);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let activity: Value =
        serde_json::from_slice(&std::fs::read(runtime_dir.join("activity.json")).unwrap()).unwrap();
    assert_eq!(activity["state"], "failed");
    assert_eq!(activity["diagnostics"]["error_type"], "provider_api_error");
    assert_eq!(activity["diagnostics"]["error_code"], "model_not_found");
    assert!(!activity["diagnostics"]
        .as_object()
        .unwrap()
        .contains_key("api_key"));
}
