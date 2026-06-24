use camino::Utf8Path;
use ccbr_provider_hooks::{
    activity_path, build_activity_hook_command, build_hook_command,
    completion_dir_from_session_data, completion_status_label, completion_status_marker,
    default_reply_for_status, event_path, extract_req_id, install_workspace_activity_hooks,
    install_workspace_completion_hooks, latest_req_id_from_transcript, load_activity, load_event,
    normalize_activity_state, normalize_completion_status, read_activity_evidence, write_activity,
    write_event, ProviderActivityEvidence, COMPLETION_STATUS_CANCELLED,
    COMPLETION_STATUS_COMPLETED, COMPLETION_STATUS_FAILED, COMPLETION_STATUS_INCOMPLETE,
};
use serde_json::Map;
use std::collections::HashMap;
use tempfile::TempDir;

/// Compile- and run-time check that every item in Python
/// `provider_hooks.__init__.__all__` is reachable from the crate root.
#[test]
fn crate_root_reexports_are_reachable() {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let runtime = root.join("runtime");
    let completion = root.join("completion");

    let _ = COMPLETION_STATUS_CANCELLED;
    let _ = COMPLETION_STATUS_COMPLETED;
    let _ = COMPLETION_STATUS_FAILED;
    let _ = COMPLETION_STATUS_INCOMPLETE;

    assert_eq!(normalize_activity_state("tool"), Some("active"));
    assert_eq!(activity_path(&runtime), runtime.join("activity.json"));
    assert_eq!(
        write_activity(
            "codex",
            "project-1",
            "agent2",
            &runtime,
            "tool",
            "codex_hook",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("2026-05-27T00:00:00Z"),
        )
        .unwrap(),
        runtime.join("activity.json")
    );
    assert!(load_activity(&runtime).is_some());
    assert!(read_activity_evidence(
        &runtime,
        "project-1",
        "agent2",
        "codex",
        None,
        None,
        None,
        None,
        Some("2026-05-27T00:00:05Z"),
        30.0,
    )
    .is_some());

    let _ = ProviderActivityEvidence {
        state: "active".into(),
        source: "test".into(),
        reason: "test".into(),
        updated_at: "2026-05-27T00:00:00Z".into(),
        event_name: None,
        provider_session_id: None,
        provider_turn_id: None,
        model: None,
        diagnostics: Some(Map::new()),
    };

    assert_eq!(
        normalize_completion_status(Some("unknown"), true),
        "completed"
    );
    assert_eq!(completion_status_label(Some("failed"), true), "Failed");
    assert_eq!(
        completion_status_marker(Some("failed"), true),
        "[CCB_TASK_FAILED]"
    );
    assert_eq!(
        default_reply_for_status(Some("cancelled"), true),
        "Task cancelled or timed out before completion."
    );

    assert_eq!(
        event_path(&completion, "job-123"),
        completion.join("events").join("job-123.json")
    );
    assert_eq!(
        extract_req_id("CCB_REQ_ID: job_current123"),
        Some("job_current123".into())
    );
    assert!(latest_req_id_from_transcript(None).is_none());

    let mut session_data = HashMap::new();
    session_data.insert(
        "completion_artifact_dir".into(),
        serde_json::Value::String(completion.to_string()),
    );
    assert_eq!(
        completion_dir_from_session_data(&session_data),
        Some(completion)
    );

    assert_eq!(
        write_event(
            "claude",
            root.join("completion2"),
            "agent1",
            "/tmp/workspace",
            "job-123",
            "completed",
            "hello",
            None,
            None,
            None,
            None,
        )
        .unwrap(),
        root.join("completion2").join("events").join("job-123.json")
    );
    assert!(load_event(root.join("completion2"), "job-123").is_some());

    let _ = build_hook_command(
        "claude",
        Utf8Path::new("/tmp/bin/ccbr-provider-finish-hook"),
        "/usr/bin/python3",
        Utf8Path::new("/tmp/completion"),
        "agent1",
        Utf8Path::new("/tmp/workspace"),
    );
    let _ = build_activity_hook_command(
        "claude",
        Utf8Path::new("/tmp/bin/ccbr-provider-activity-hook"),
        "/usr/bin/python3",
        "project-1",
        "agent1",
        Utf8Path::new("/tmp/runtime"),
        Utf8Path::new("/tmp/workspace"),
    );

    let home_root = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let command = "/usr/bin/python3 /tmp/ccbr-provider-finish-hook --provider claude";
    let _ = install_workspace_completion_hooks("claude", &workspace, Some(&home_root), command);
    let _ = install_workspace_activity_hooks("claude", &workspace, Some(&home_root), command);
}
