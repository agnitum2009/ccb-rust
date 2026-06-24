use ccbr_cli::context::{CliContext, CliContextBuilder};
use ccbr_cli::models::{ParsedCommand, ParsedWaitCommand};
use ccbr_cli::services::wait_runtime::service::wait_for_replies;
use serde_json::{json, Value};
use tempfile::TempDir;

fn build_context(project_root: std::path::PathBuf) -> CliContext {
    let ccbr_dir = project_root.join(".ccbr");
    std::fs::create_dir_all(&ccbr_dir).unwrap();
    std::fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();
    CliContextBuilder::new(ParsedCommand::Wait(ParsedWaitCommand::new(
        None,
        "any".into(),
        "msg_1".into(),
    )))
    .cwd(project_root)
    .build()
    .expect("build context")
}

#[derive(Debug)]
struct FakeTraceClient {
    payloads: std::sync::Mutex<Vec<Value>>,
}

impl ccbr_cli::services::wait_runtime::service::TraceClient for FakeTraceClient {
    fn trace(&self, target: &str) -> Result<Value, String> {
        assert_eq!(target, "msg_1");
        let mut payloads = self.payloads.lock().unwrap();
        if payloads.is_empty() {
            return Err("no more payloads".into());
        }
        Ok(payloads.remove(0))
    }
}

#[test]
fn test_wait_for_replies_any_polls_until_reply_arrives() {
    let tmp = TempDir::new().unwrap();
    let context = build_context(tmp.path().join("repo-wait-any"));
    let command = ParsedWaitCommand {
        project: None,
        mode: "any".into(),
        target: "msg_1".into(),
        quorum: None,
        timeout_s: Some(1.0),
        kind: "wait".into(),
    };

    let payloads = vec![
        json!({
            "resolved_kind": "message",
            "attempts": [
                {"attempt_id": "att_1", "message_id": "msg_1", "agent_name": "codex", "retry_index": 0, "updated_at": "2026-03-30T00:00:01Z"}
            ],
            "replies": [],
        }),
        json!({
            "resolved_kind": "message",
            "attempts": [
                {"attempt_id": "att_1", "message_id": "msg_1", "agent_name": "codex", "retry_index": 0, "updated_at": "2026-03-30T00:00:02Z"}
            ],
            "replies": [
                {"reply_id": "rep_1", "message_id": "msg_1", "attempt_id": "att_1", "agent_name": "codex",
                 "terminal_status": "completed", "reason": "task_complete", "finished_at": "2026-03-30T00:00:10Z", "reply": "done"}
            ],
        }),
    ];
    let client = FakeTraceClient {
        payloads: std::sync::Mutex::new(payloads),
    };

    let summary = wait_for_replies(
        &context,
        &command,
        &client,
        no_sleep,
        std::time::Instant::now,
    );

    assert_eq!(summary.mode, "any");
    assert_eq!(summary.target, "msg_1");
    assert_eq!(summary.resolved_kind, "message");
    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.received_count, 1);
    assert_eq!(summary.terminal_count, 1);
    assert_eq!(summary.notice_count, 0);
    assert_eq!(summary.wait_status, "satisfied");
    assert_eq!(summary.replies[0]["reply_id"], "rep_1");
    assert_eq!(summary.replies[0]["reply"], "done");
}

#[test]
fn test_wait_for_replies_quorum_uses_latest_attempt_per_agent() {
    let tmp = TempDir::new().unwrap();
    let context = build_context(tmp.path().join("repo-wait-quorum"));
    let command = ParsedWaitCommand {
        project: None,
        mode: "quorum".into(),
        target: "msg_1".into(),
        quorum: Some(1),
        timeout_s: Some(1.0),
        kind: "wait".into(),
    };

    let payload = json!({
        "resolved_kind": "message",
        "attempts": [
            {"attempt_id": "att_old", "message_id": "msg_1", "agent_name": "codex", "retry_index": 0, "updated_at": "2026-03-30T00:00:02Z"},
            {"attempt_id": "att_new", "message_id": "msg_1", "agent_name": "codex", "retry_index": 1, "updated_at": "2026-03-30T00:00:03Z"},
        ],
        "replies": [
            {"reply_id": "rep_old", "message_id": "msg_1", "attempt_id": "att_old", "agent_name": "codex",
             "terminal_status": "incomplete", "reason": "need_retry", "finished_at": "2026-03-30T00:00:04Z", "reply": "retry me"},
            {"reply_id": "rep_new", "message_id": "msg_1", "attempt_id": "att_new", "agent_name": "codex",
             "terminal_status": "completed", "reason": "task_complete", "finished_at": "2026-03-30T00:00:05Z", "reply": "final answer"},
        ],
    });
    let client = FakeTraceClient {
        payloads: std::sync::Mutex::new(vec![payload]),
    };

    let summary = wait_for_replies(
        &context,
        &command,
        &client,
        no_sleep,
        std::time::Instant::now,
    );

    assert_eq!(summary.mode, "quorum");
    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.received_count, 1);
    assert_eq!(summary.terminal_count, 1);
    assert_eq!(summary.notice_count, 0);
    assert_eq!(summary.wait_status, "satisfied");
    assert_eq!(summary.replies[0]["reply_id"], "rep_new");
    assert_eq!(summary.replies[0]["reply"], "final answer");
}

#[test]
fn test_wait_for_replies_returns_notice_when_heartbeat_arrives_first() {
    let tmp = TempDir::new().unwrap();
    let context = build_context(tmp.path().join("repo-wait-heartbeat"));
    let command = ParsedWaitCommand {
        project: None,
        mode: "any".into(),
        target: "msg_1".into(),
        quorum: None,
        timeout_s: Some(1.0),
        kind: "wait".into(),
    };

    let payload = json!({
        "resolved_kind": "message",
        "attempts": [
            {"attempt_id": "att_1", "message_id": "msg_1", "agent_name": "codex", "job_id": "job_1", "retry_index": 0, "updated_at": "2026-03-30T00:10:00Z"}
        ],
        "replies": [
            {"reply_id": "rep_heartbeat", "message_id": "msg_1", "attempt_id": "att_1", "agent_name": "codex",
             "terminal_status": "incomplete", "notice": true, "notice_kind": "heartbeat",
             "last_progress_at": "2026-03-30T00:00:00Z", "heartbeat_silence_seconds": 600.0,
             "reason": Value::Null, "finished_at": "2026-03-30T00:10:00Z", "reply": "task still running"}
        ],
    });
    let client = FakeTraceClient {
        payloads: std::sync::Mutex::new(vec![payload]),
    };

    let summary = wait_for_replies(
        &context,
        &command,
        &client,
        no_sleep,
        std::time::Instant::now,
    );

    assert_eq!(summary.wait_status, "notice");
    assert_eq!(summary.received_count, 1);
    assert_eq!(summary.terminal_count, 0);
    assert_eq!(summary.notice_count, 1);
    assert_eq!(summary.replies[0]["notice"], true);
    assert_eq!(summary.replies[0]["notice_kind"], "heartbeat");
    assert_eq!(summary.replies[0]["job_id"], "job_1");
}

fn no_sleep(_: std::time::Duration) {}
