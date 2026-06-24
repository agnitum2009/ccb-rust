use camino::Utf8Path;
use ccbr_provider_hooks::{activity, artifacts, notifications, settings};
use serde_json::json;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn activity_write_and_read_round_trip() {
    let dir = TempDir::new().unwrap();
    let runtime = Utf8Path::from_path(dir.path()).unwrap().join("runtime");

    let path = activity::write_activity(
        "codex",
        "project-1",
        "agent2",
        &runtime,
        "tool",
        "codex_hook",
        Some("PreToolUse"),
        Some("ccbr-agent2-1"),
        Some("%42"),
        Some("/tmp/workspace"),
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:00Z"),
    )
    .unwrap();

    assert_eq!(path, runtime.join("activity.json"));
    let evidence = activity::read_activity_evidence(
        &runtime,
        "project-1",
        "agent2",
        "codex",
        Some("ccbr-agent2-1"),
        None,
        Some("%42"),
        Some("/tmp/workspace"),
        Some("2026-05-27T00:00:05Z"),
        30.0,
    )
    .unwrap();

    assert_eq!(evidence.state, "active");
    assert_eq!(evidence.source, "codex_hook");
    assert_eq!(evidence.reason, "provider_PreToolUse");
    assert_eq!(evidence.event_name.as_deref(), Some("PreToolUse"));
}

#[test]
fn activity_diagnostics_filter_secrets() {
    let dir = TempDir::new().unwrap();
    let runtime = Utf8Path::from_path(dir.path()).unwrap().join("runtime");

    let mut diagnostics = HashMap::new();
    diagnostics.insert("tool_name".into(), json!("shell"));
    diagnostics.insert("api_key".into(), json!("secret-value"));

    activity::write_activity(
        "claude",
        "project-1",
        "agent1",
        &runtime,
        "failed",
        "claude_hook",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&diagnostics),
        Some("2026-05-27T00:00:00Z"),
    )
    .unwrap();

    let payload = activity::load_activity(&runtime).unwrap();
    let stored = payload["diagnostics"].as_object().unwrap();
    assert!(stored.contains_key("tool_name"));
    assert!(!stored.contains_key("api_key"));
}

#[test]
fn failed_activity_is_sticky_for_idle() {
    let dir = TempDir::new().unwrap();
    let runtime = Utf8Path::from_path(dir.path()).unwrap().join("runtime");

    activity::write_activity(
        "codex",
        "project-1",
        "agent2",
        &runtime,
        "failed",
        "codex_hook",
        None,
        Some("ccbr-1"),
        Some("%1"),
        None,
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:00Z"),
    )
    .unwrap();

    activity::write_activity(
        "codex",
        "project-1",
        "agent2",
        &runtime,
        "idle",
        "codex_hook",
        None,
        Some("ccbr-1"),
        Some("%1"),
        None,
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:01Z"),
    )
    .unwrap();

    let payload = activity::load_activity(&runtime).unwrap();
    assert_eq!(payload["state"], "failed");

    activity::write_activity(
        "codex",
        "project-1",
        "agent2",
        &runtime,
        "active",
        "codex_hook",
        None,
        Some("ccbr-1"),
        Some("%1"),
        None,
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:02Z"),
    )
    .unwrap();

    let payload = activity::load_activity(&runtime).unwrap();
    assert_eq!(payload["state"], "active");
}

#[test]
fn artifact_event_write_and_load() {
    let dir = TempDir::new().unwrap();
    let completion = Utf8Path::from_path(dir.path()).unwrap().join("completion");

    let mut diagnostics = HashMap::new();
    diagnostics.insert("hook_event_name".into(), json!("Stop"));

    let path = artifacts::write_event(
        "claude",
        &completion,
        "agent1",
        "/tmp/workspace",
        "job-abc123",
        "completed",
        "hello world",
        Some("session-1"),
        Some("Stop"),
        None,
        Some(&diagnostics),
    )
    .unwrap();

    assert_eq!(path, completion.join("events").join("job-abc123.json"));
    let loaded = artifacts::load_event(&completion, "job-abc123").unwrap();
    assert_eq!(loaded["provider"], "claude");
    assert_eq!(loaded["status"], "completed");
    assert_eq!(loaded["reply"], "hello world");
    assert_eq!(loaded["req_id"], "job-abc123");
}

#[test]
fn transcript_req_id_extraction_prefers_outer_marker() {
    let content = format!(
        "{}\n{}\n",
        json!({"type": "user", "message": {"role": "user", "content": "CCB_REQ_ID: job_current123\n\nReview this transcript:\nCCB_REQ_ID: job_old456\n```text\nCCB_REQ_ID: job_code789\n```"}}),
        json!({"type": "assistant", "message": {"role": "assistant", "content": "Working."}}),
    );

    assert_eq!(
        artifacts::latest_user_req_id_from_transcript_text(&content),
        Some("job_current123".into())
    );
    assert_eq!(
        artifacts::latest_req_id_from_transcript_text(&content),
        Some("job_current123".into())
    );
}

#[test]
fn transcript_current_turn_follows_parent_chain() {
    let content = format!(
        "{}\n{}\n{}\n{}\n",
        json!({"uuid": "u1", "type": "user", "message": {"role": "user", "content": "CCB_REQ_ID: job_current123\n\nRun a tool."}}),
        json!({"uuid": "a1", "parentUuid": "u1", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "tool_use", "name": "Read"}]}}),
        json!({"uuid": "u2", "parentUuid": "a1", "type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "content": "ok"}]}, "toolUseResult": {"type": "text"}}),
        json!({"uuid": "a2", "parentUuid": "u2", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "done"}]}}),
    );

    assert_eq!(
        artifacts::current_turn_req_id_from_transcript_text(&content, Some("done")),
        Some("job_current123".into())
    );
}

#[test]
fn settings_install_claude_completion_hooks() {
    let dir = TempDir::new().unwrap();
    let home_root = Utf8Path::from_path(dir.path()).unwrap().join("claude-home");
    let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let command = "/usr/bin/python3 /tmp/ccbr-provider-finish-hook --provider claude";

    let settings_path = settings::install_workspace_completion_hooks(
        "claude",
        &workspace,
        Some(&home_root),
        command,
    )
    .unwrap();

    assert_eq!(
        settings_path,
        home_root.join(".claude").join("settings.json")
    );
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(data["hooks"]["Stop"][0]["hooks"][0]["command"], command);

    let trust_path = home_root.join(".claude.json");
    let trust: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&trust_path).unwrap()).unwrap();
    assert_eq!(
        trust[workspace.canonicalize().unwrap().to_str().unwrap()]["hasTrustDialogAccepted"],
        true
    );
}

#[test]
fn settings_install_gemini_completion_hooks() {
    let dir = TempDir::new().unwrap();
    let home_root = Utf8Path::from_path(dir.path()).unwrap().join("gemini-home");
    let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
    let command = "/usr/bin/python3 /tmp/ccbr-provider-finish-hook --provider gemini";

    let settings_path = settings::install_workspace_completion_hooks(
        "gemini",
        &workspace,
        Some(&home_root),
        command,
    )
    .unwrap();

    assert_eq!(
        settings_path,
        home_root.join(".gemini").join("settings.json")
    );
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(data["hooks"]["AfterAgent"][0]["matcher"], "*");
    assert_eq!(
        data["hooks"]["AfterAgent"][0]["hooks"][0]["command"],
        command
    );
}

#[test]
fn settings_install_claude_completion_hooks_preserves_existing_entries() {
    let dir = TempDir::new().unwrap();
    let home_root = Utf8Path::from_path(dir.path()).unwrap().join("claude-home");
    let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let command = "/usr/bin/python3 /tmp/ccbr-provider-finish-hook --provider claude";
    let settings_path = home_root.join(".claude").join("settings.json");
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "hooks": {
                "Stop": [
                    {"hooks": [{"type": "command", "command": "echo existing"}]},
                    {"hooks": [{"type": "command", "command": command}]},
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    settings::install_workspace_completion_hooks("claude", &workspace, Some(&home_root), command)
        .unwrap();

    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert_eq!(data["hooks"]["Stop"].as_array().unwrap().len(), 2);
    assert!(!workspace.join(".claude").exists());
}

#[test]
fn settings_install_claude_activity_hooks_prunes_stale_activity_hooks() {
    let dir = TempDir::new().unwrap();
    let home_root = Utf8Path::from_path(dir.path()).unwrap().join("claude-home");
    let workspace = Utf8Path::from_path(dir.path()).unwrap().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let activity_command =
        "/usr/bin/python3 /current/bin/ccbr-provider-activity-hook --provider claude";
    let stale_activity_command =
        "/usr/bin/python3 /old/bin/ccbr-provider-activity-hook --provider claude";
    let finish_command = "/usr/bin/python3 /old/bin/ccbr-provider-finish-hook --provider claude";
    let settings_path = home_root.join(".claude").join("settings.json");
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "hooks": {
                "Stop": [
                    {"hooks": [{"type": "command", "command": finish_command}]},
                    {"hooks": [{"type": "command", "command": stale_activity_command}]},
                ],
                "PostToolUse": [
                    {"hooks": [{"type": "command", "command": "echo existing"}]},
                    {"hooks": [{"type": "command", "command": stale_activity_command}]},
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    settings::install_workspace_activity_hooks(
        "claude",
        &workspace,
        Some(&home_root),
        activity_command,
    )
    .unwrap();

    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    let stop_commands: Vec<String> = data["hooks"]["Stop"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["hooks"].as_array().unwrap().iter())
        .filter_map(|hook| hook["command"].as_str().map(|s| s.to_string()))
        .collect();
    let post_tool_commands: Vec<String> = data["hooks"]["PostToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["hooks"].as_array().unwrap().iter())
        .filter_map(|hook| hook["command"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(stop_commands.contains(&finish_command.to_string()));
    assert!(stop_commands.contains(&activity_command.to_string()));
    assert!(!stop_commands.contains(&stale_activity_command.to_string()));
    assert_eq!(post_tool_commands, vec!["echo existing", activity_command]);
    assert!(!workspace.join(".claude").exists());
}

#[test]
fn notifications_status_helpers() {
    assert_eq!(
        notifications::normalize_completion_status(Some("failed"), true),
        notifications::COMPLETION_STATUS_FAILED
    );
    assert_eq!(
        notifications::completion_status_label(Some("failed"), true),
        "Failed"
    );
    assert_eq!(
        notifications::completion_status_marker(Some("failed"), true),
        "[CCB_TASK_FAILED]"
    );
    assert_eq!(
        notifications::default_reply_for_status(Some("cancelled"), true),
        "Task cancelled or timed out before completion."
    );
}

#[test]
fn test_crate_root_reexports_reachable() {
    use camino::Utf8Path;
    use serde_json::Map;
    use std::collections::HashMap;

    let _: Option<ccbr_provider_hooks::ProviderActivityEvidence> = None;

    let _ = ccbr_provider_hooks::COMPLETION_STATUS_CANCELLED;
    let _ = ccbr_provider_hooks::COMPLETION_STATUS_COMPLETED;
    let _ = ccbr_provider_hooks::COMPLETION_STATUS_FAILED;
    let _ = ccbr_provider_hooks::COMPLETION_STATUS_INCOMPLETE;

    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let runtime = root.join("runtime");
    let completion = root.join("completion");
    let workspace = root.join("workspace");
    let home_root = root.join("home");

    // activity
    let _ = ccbr_provider_hooks::activity_path(&runtime);
    let _ = ccbr_provider_hooks::load_activity(&runtime);
    let _ = ccbr_provider_hooks::normalize_activity_state("tool");
    let _ = ccbr_provider_hooks::read_activity_evidence(
        &runtime, "p", "a", "claude", None, None, None, None, None, 30.0,
    );
    let _ = ccbr_provider_hooks::write_activity(
        "claude",
        "p",
        "a",
        &runtime,
        "idle",
        "",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:00Z"),
    );

    // artifacts
    let _ = ccbr_provider_hooks::completion_dir_from_session_data(&HashMap::new());
    let _ = ccbr_provider_hooks::event_path(&completion, "id");
    let _ = ccbr_provider_hooks::extract_req_id("CCB_REQ_ID: job_1");
    let _ = ccbr_provider_hooks::latest_req_id_from_transcript(None);
    let _ = ccbr_provider_hooks::load_event(&completion, "id");
    let _ = ccbr_provider_hooks::write_event(
        "claude",
        &completion,
        "a",
        workspace.as_str(),
        "id",
        "completed",
        "hello",
        None,
        None,
        None,
        None,
    );

    // notifications
    let _ = ccbr_provider_hooks::completion_status_label(Some("failed"), true);
    let _ = ccbr_provider_hooks::completion_status_marker(Some("failed"), true);
    let _ = ccbr_provider_hooks::default_reply_for_status(Some("failed"), true);
    let _ = ccbr_provider_hooks::normalize_completion_status(Some("failed"), true);

    // settings
    let _ = ccbr_provider_hooks::build_hook_command(
        "claude",
        Utf8Path::new("/tmp/script"),
        "/usr/bin/python3",
        Utf8Path::new("/tmp/comp"),
        "a",
        Utf8Path::new("/tmp/ws"),
    );
    let _ = ccbr_provider_hooks::build_activity_hook_command(
        "claude",
        Utf8Path::new("/tmp/script"),
        "/usr/bin/python3",
        "p",
        "a",
        Utf8Path::new("/tmp/runtime"),
        Utf8Path::new("/tmp/ws"),
    );
    let _ = ccbr_provider_hooks::install_workspace_activity_hooks(
        "claude",
        &workspace,
        Some(&home_root),
        "cmd",
    );
    let _ = ccbr_provider_hooks::install_workspace_completion_hooks(
        "claude",
        &workspace,
        Some(&home_root),
        "cmd",
    );

    // submodule helpers
    let _ = ccbr_provider_hooks::claude_hook_home_layout(&home_root);
    let _ = ccbr_provider_hooks::load_json(root.join("missing.json").as_ref());
    let _ = ccbr_provider_hooks::save_json(root.join("test.json").as_ref(), &Map::new());
    let _ = ccbr_provider_hooks::workspace_key(&workspace);
}
