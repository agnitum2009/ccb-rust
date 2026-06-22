//! Mirrors Python `test/test_provider_finish_hook_script.py`.

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn hook_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ccb-provider-finish-hook"))
}

fn run_hook(
    completion_dir: &std::path::Path,
    provider: &str,
    agent_name: &str,
    workspace: &std::path::Path,
    payload: Value,
) -> std::process::Output {
    let mut child = Command::new(hook_bin())
        .arg("--provider")
        .arg(provider)
        .arg("--completion-dir")
        .arg(completion_dir)
        .arg("--agent-name")
        .arg(agent_name)
        .arg("--workspace")
        .arg(workspace)
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
fn test_provider_finish_hook_writes_claude_completion_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let transcript = tmp.path().join("transcript.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"user","message":{"content":"CCB_REQ_ID: 20260331-130805-796-1333224-9"}}"#,
    )
    .unwrap();

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "transcript_path": transcript,
        "last_assistant_message": "A3_FIX_13_OK",
        "session_id": "claude-session-1",
    });

    let output = run_hook(&completion_dir, "claude", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event_path = completion_dir
        .join("events")
        .join("20260331-130805-796-1333224-9.json");
    let event: Value = serde_json::from_slice(&std::fs::read(&event_path).unwrap()).unwrap();
    assert_eq!(event["provider"], "claude");
    assert_eq!(event["agent_name"], "agent3");
    assert_eq!(event["reply"], "A3_FIX_13_OK");
    assert_eq!(event["status"], "completed");
}

#[test]
fn test_provider_finish_hook_marks_empty_claude_reply_incomplete() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let transcript = tmp.path().join("transcript.jsonl");
    let req_id = "job_emptyclaude123";
    std::fs::write(
        &transcript,
        serde_json::json!({
            "uuid": "old-user",
            "type": "user",
            "message": {"role": "user", "content": "CCB_REQ_ID: job_previous111\n\nPrevious task."},
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "uuid": "old-assistant",
                "parentUuid": "old-user",
                "type": "assistant",
                "message": {"role": "assistant", "content": [{"type": "text", "text": "previous done"}]},
            })
            .to_string()
            + "\n"
            + &serde_json::json!({
                "uuid": "current-user",
                "type": "user",
                "message": {"role": "user", "content": format!("CCB_REQ_ID: {req_id}\n\nRun the task.")},
            })
            .to_string()
            + "\n",
    )
    .unwrap();

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "transcript_path": transcript,
        "last_assistant_message": "",
        "session_id": "claude-session-1",
    });

    let output = run_hook(&completion_dir, "claude", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event: Value = serde_json::from_slice(
        &std::fs::read(completion_dir.join("events").join(format!("{req_id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(event["provider"], "claude");
    assert_eq!(event["reply"], "");
    assert_eq!(event["status"], "incomplete");
    assert_eq!(event["diagnostics"]["reason"], "hook_stop_empty_reply");
    assert_eq!(event["diagnostics"]["empty_reply"], true);
    assert_eq!(event["diagnostics"]["error_type"], "empty_provider_reply");
}

#[test]
fn test_provider_finish_hook_uses_outer_claude_req_id_when_body_mentions_old_req_id() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let transcript = tmp.path().join("transcript.jsonl");
    let current_req_id = "job_current123abc";
    let embedded_old_req_id = "job_old456def";
    std::fs::write(
        &transcript,
        serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": format!(
                    "CCB_REQ_ID: {current_req_id}\n\nCCB_REQ_ID: {embedded_old_req_id}\n\nForwarded review context that contains an older request id."
                ),
            },
        })
        .to_string()
            + "\n",
    )
    .unwrap();

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "transcript_path": transcript,
        "last_assistant_message": "review completed",
        "session_id": "claude-session-1",
    });

    let output = run_hook(&completion_dir, "claude", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event_path = completion_dir
        .join("events")
        .join(format!("{current_req_id}.json"));
    let old_event_path = completion_dir
        .join("events")
        .join(format!("{embedded_old_req_id}.json"));
    assert!(event_path.exists());
    assert!(!old_event_path.exists());
    let event: Value = serde_json::from_slice(&std::fs::read(&event_path).unwrap()).unwrap();
    assert_eq!(event["req_id"], current_req_id);
    assert_eq!(event["reply"], "review completed");
    assert_eq!(event["status"], "completed");
}

#[test]
fn test_provider_finish_hook_ignores_later_claude_tool_result_req_id() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let transcript = tmp.path().join("transcript.jsonl");
    let current_req_id = "job_currentabc123";
    let tool_result_req_id = "job_toolresult999";
    std::fs::write(
        &transcript,
        serde_json::json!({
            "type": "user",
            "message": {"role": "user", "content": format!("CCB_REQ_ID: {current_req_id}\n\nReview this package.")},
        })
        .to_string()
            + "\n"
            + &serde_json::json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{"type": "tool_result", "tool_use_id": "tooluse_1", "content": format!("Command output mentioned CCB_REQ_ID: {tool_result_req_id}"), "is_error": false}],
                },
            })
            .to_string()
            + "\n",
    )
    .unwrap();

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "transcript_path": transcript,
        "last_assistant_message": "done after tools",
        "session_id": "claude-session-1",
    });

    let output = run_hook(&completion_dir, "claude", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event_path = completion_dir
        .join("events")
        .join(format!("{current_req_id}.json"));
    let tool_result_event_path = completion_dir
        .join("events")
        .join(format!("{tool_result_req_id}.json"));
    assert!(event_path.exists());
    assert!(!tool_result_event_path.exists());
    let event: Value = serde_json::from_slice(&std::fs::read(&event_path).unwrap()).unwrap();
    assert_eq!(event["req_id"], current_req_id);
    assert_eq!(event["reply"], "done after tools");
    assert_eq!(event["status"], "completed");
}

#[test]
fn test_provider_finish_hook_ignores_claude_scheduled_task_after_stale_ccb_prompt() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let transcript = tmp.path().join("transcript.jsonl");
    let stale_req_id = "job_stale123abc";
    let scheduled_reply = "当前进度：已完成第9次，正在执行第10次。";
    let records = [
        serde_json::json!({"uuid": "u1", "type": "user", "message": {"role": "user", "content": format!("CCB_REQ_ID: {stale_req_id}\n\nRun a long task.")}}),
        serde_json::json!({"uuid": "u2", "parentUuid": "u1", "type": "user", "message": {"role": "user", "content": [{"type": "text", "text": "[Request interrupted by user]"}]}}),
        serde_json::json!({"uuid": "s1", "parentUuid": "u2", "type": "system", "subtype": "scheduled_task_fire", "content": "Running scheduled task"}),
        serde_json::json!({"uuid": "u3", "parentUuid": "s1", "type": "user", "message": {"role": "user", "content": "循环计数，共50次"}, "isMeta": true}),
        serde_json::json!({"uuid": "a1", "parentUuid": "u3", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": scheduled_reply}]}}),
        serde_json::json!({"type": "last-prompt", "lastPrompt": format!("CCB_REQ_ID: {stale_req_id}\n\nRun a long task.")}),
    ];
    std::fs::write(
        &transcript,
        records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();

    let payload = serde_json::json!({
        "hook_event_name": "Stop",
        "transcript_path": transcript,
        "last_assistant_message": scheduled_reply,
        "session_id": "claude-session-1",
    });

    let output = run_hook(&completion_dir, "claude", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!completion_dir
        .join("events")
        .join(format!("{stale_req_id}.json"))
        .exists());
}

#[test]
fn test_provider_finish_hook_writes_gemini_failed_event_for_login_required_response() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let req_id = "20260331-130805-796-1333224-10";
    let payload = serde_json::json!({
        "hook_event_name": "AfterAgent",
        "prompt": format!("CCB_REQ_ID: {req_id} Execute the full request from @/tmp/request.md and reply directly."),
        "prompt_response": "Code Assist login required.\nAttempting to open authentication page in your browser.\nOtherwise navigate to:\nhttps://accounts.google.com/o/oauth2/v2/auth?... \n",
        "session_id": "gemini-session-1",
        "finishReason": "STOP",
    });

    let output = run_hook(&completion_dir, "gemini", "agent3", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event: Value = serde_json::from_slice(
        &std::fs::read(completion_dir.join("events").join(format!("{req_id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(event["provider"], "gemini");
    assert_eq!(event["agent_name"], "agent3");
    assert_eq!(event["status"], "failed");
    assert!(event["reply"]
        .as_str()
        .unwrap()
        .starts_with("Code Assist login required."));
    assert_eq!(event["diagnostics"]["hook_event_name"], "AfterAgent");
    assert_eq!(event["diagnostics"]["finish_reason"], "STOP");
    assert_eq!(event["diagnostics"]["error_type"], "provider_api_error");
    assert_eq!(event["diagnostics"]["error_code"], "LoginRequired");
    assert!(event["diagnostics"]["error_message"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("login required"));
}

#[test]
fn test_provider_finish_hook_accepts_job_id_anchor_from_prompt() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let req_id = "job_06188b28c1db";
    let payload = serde_json::json!({
        "hook_event_name": "AfterAgent",
        "prompt": format!("CCB_REQ_ID: {req_id} Execute the full request from @/tmp/request.md and reply directly."),
        "prompt_response": "job-based reply",
        "session_id": "gemini-session-1",
        "finishReason": "STOP",
    });

    let output = run_hook(&completion_dir, "gemini", "agent2", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event: Value = serde_json::from_slice(
        &std::fs::read(completion_dir.join("events").join(format!("{req_id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(event["req_id"], req_id);
    assert_eq!(event["reply"], "job-based reply");
}

#[test]
fn test_provider_finish_hook_marks_empty_gemini_reply_incomplete() {
    let tmp = tempfile::TempDir::new().unwrap();
    let completion_dir = tmp.path().join("completion");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let req_id = "job_7c1f6ab28cde";
    let payload = serde_json::json!({
        "hook_event_name": "AfterAgent",
        "prompt": format!("CCB_REQ_ID: {req_id} Execute the full request from @/tmp/request.md and reply directly."),
        "prompt_response": "",
        "session_id": "gemini-session-1",
        "finishReason": "STOP",
    });

    let output = run_hook(&completion_dir, "gemini", "agent2", &workspace, payload);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event: Value = serde_json::from_slice(
        &std::fs::read(completion_dir.join("events").join(format!("{req_id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(event["req_id"], req_id);
    assert_eq!(event["reply"], "");
    assert_eq!(event["status"], "incomplete");
    assert_eq!(
        event["diagnostics"]["reason"],
        "hook_after_agent_incomplete"
    );
    assert_eq!(event["diagnostics"]["empty_reply"], true);
}
