use camino::Utf8PathBuf;
use ccbr_storage::path_helpers::{
    runtime_project_anchor_from_path, runtime_state_root_from_anchor_ref,
};
use ccbr_storage::paths::PathLayout;
use std::fs;

fn tmp_path(tmp: &tempfile::TempDir, tail: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(tmp.path().join(tail)).unwrap()
}

#[test]
fn test_path_layout_uses_project_scoped_locations() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo"));

    assert_eq!(layout.ccbr_dir(), tmp_path(&tmp, "repo/.ccbr"));
    assert_eq!(layout.project_anchor_dir(), layout.ccbr_dir());
    assert_eq!(layout.runtime_state_root(), layout.ccbr_dir());
    assert_eq!(
        layout.runtime_state_placement().root_kind.as_str(),
        "project"
    );
    assert_eq!(layout.runtime_marker_status(), "not_required");
    assert_eq!(
        layout.config_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbr.config")
    );
    assert_eq!(
        layout.ccbrd_lifecycle_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/lifecycle.json")
    );
    assert_eq!(
        layout.ccbrd_lease_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/lease.json")
    );
    let socket_path = layout.ccbrd_socket_path();
    let socket_name = socket_path.file_name().unwrap();
    assert!(socket_name == "ccbrd.sock" || socket_name.starts_with("ccbrd-"));
    assert!(layout.ccbrd_socket_path().as_str().len() <= 100);
    assert_eq!(
        layout.ccbrd_state_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/state.json")
    );
    assert_eq!(
        layout.ccbrd_start_policy_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/start-policy.json")
    );
    assert_eq!(
        layout.ccbrd_startup_report_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/startup-report.json")
    );
    assert_eq!(
        layout.ccbrd_shutdown_report_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/shutdown-report.json")
    );
    let tmux_socket_path = layout.ccbrd_tmux_socket_path();
    let tmux_socket_name = tmux_socket_path.file_name().unwrap();
    assert!(tmux_socket_name == "tmux.sock" || tmux_socket_name.starts_with("tmux-"));
    assert!(layout.ccbrd_tmux_socket_path().as_str().len() <= 100);
    assert_eq!(
        layout.ccbrd_tmux_session_name(),
        format!("ccbr-{}", layout.project_slug())
    );
    assert_eq!(
        layout.ccbrd_lifecycle_log_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/lifecycle.jsonl")
    );
    assert_eq!(
        layout.ccbrd_support_dir(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/support")
    );
    assert_eq!(
        layout.ccbrd_keeper_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/keeper.json")
    );
    assert_eq!(
        layout.ccbrd_shutdown_intent_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/shutdown-intent.json")
    );
    assert_eq!(
        layout.agent_mailbox_path("Agent1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/mailboxes/agent1/mailbox.json")
    );
    assert_eq!(
        layout.agent_inbox_path("Agent1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/mailboxes/agent1/inbox.jsonl")
    );
    assert_eq!(
        layout.ccbrd_messages_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/messages/messages.jsonl")
    );
    assert_eq!(
        layout.ccbrd_attempts_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/attempts/attempts.jsonl")
    );
    assert_eq!(
        layout.ccbrd_replies_path(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/replies/replies.jsonl")
    );
    assert_eq!(
        layout.mailbox_lease_path("Agent1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/leases/agent1.json")
    );
    assert_eq!(
        layout.provider_health_path("job-1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/provider-health/job-1.jsonl")
    );
    assert_eq!(
        layout.agent_runtime_path("Agent1"),
        tmp_path(&tmp, "repo/.ccbr/agents/agent1/runtime.json")
    );
    assert_eq!(
        layout.agent_provider_state_dir("Agent1", "CoDeX"),
        tmp_path(&tmp, "repo/.ccbr/agents/agent1/provider-state/codex")
    );
    assert_eq!(
        layout.snapshot_path("job-1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/snapshots/job-1.json")
    );
    assert_eq!(
        layout.cursor_path("job-1"),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/cursors/job-1.json")
    );
    assert_eq!(
        layout.support_bundle_path("bundle-1").unwrap(),
        tmp_path(&tmp, "repo/.ccbr/ccbrd/support/bundle-1.tar.gz")
    );
    assert_eq!(
        layout.workspace_path("Agent1", None),
        tmp_path(&tmp, "repo/.ccbr/workspaces/agent1")
    );
    assert_eq!(
        layout.provider_profiles_dir(),
        tmp_path(&tmp, "repo/.ccbr/provider-profiles")
    );
}

#[test]
fn test_path_layout_supports_external_workspace_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo"));
    let external = tmp.path().join("external-workspaces");
    let workspace = layout.workspace_path("agent1", Some(external.to_str().unwrap()));
    assert_eq!(
        workspace,
        tmp_path(&tmp, "external-workspaces")
            .join(layout.project_slug())
            .join("agent1")
    );
    assert_eq!(
        layout
            .workspace_binding_path("agent1", Some(external.to_str().unwrap()))
            .file_name(),
        Some(".ccbr-workspace.json")
    );
}

#[test]
fn test_path_layout_shortens_socket_paths_when_project_path_is_too_long() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp
        .path()
        .join("very-long-project-name-".repeat(4))
        .join("nested-segment-".repeat(4))
        .join("repo");
    let layout = PathLayout::new(Utf8PathBuf::from_path_buf(project_root).unwrap());

    let socket_path = layout.ccbrd_socket_path();
    assert!(
        socket_path.file_name().unwrap().starts_with("ccbrd-"),
        "socket should be shortened"
    );
    let tmux_socket_path = layout.ccbrd_tmux_socket_path();
    assert!(
        tmux_socket_path.file_name().unwrap().starts_with("tmux-"),
        "tmux socket should be shortened"
    );
    assert!(!layout.ccbrd_socket_path().as_str().contains(".ccbr/ccbrd"));
    assert!(!layout
        .ccbrd_tmux_socket_path()
        .as_str()
        .contains(".ccbr/ccbrd"));
    assert!(layout.ccbrd_socket_path().as_str().len() <= 100);
    assert!(layout.ccbrd_tmux_socket_path().as_str().len() <= 100);
}

#[test]
fn test_path_layout_uses_anchor_ref_for_relocated_runtime() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp_path(&tmp, "repo-ref");
    let layout = PathLayout::new(project_root.clone());
    let relocated_root = Utf8PathBuf::from("/r");
    fs::create_dir_all(layout.ccbr_dir()).unwrap();
    fs::write(
        layout.runtime_root_ref_path(),
        format!(
            "{{\"schema_version\":1,\"record_type\":\"ccbr_runtime_root_ref\",\"project_id\":\"{}\",\"runtime_state_root\":\"{}\",\"created_at\":\"2026-05-07T00:00:00Z\"}}",
            layout.project_id(),
            relocated_root
        ),
    )
    .unwrap();

    let relocated = PathLayout::new(project_root);
    assert_eq!(
        relocated.runtime_state_placement().root_kind.as_str(),
        "relocated"
    );
    assert_eq!(
        relocated
            .runtime_state_placement()
            .relocation_reason
            .as_deref(),
        Some("runtime_root_ref")
    );
    assert_eq!(relocated.runtime_state_root(), relocated_root);
    assert_eq!(relocated.ccbrd_dir(), relocated_root.join("ccbrd"));
    assert_eq!(relocated.agents_dir(), relocated_root.join("agents"));
    assert_eq!(
        relocated.ccbrd_socket_path(),
        relocated_root.join("ccbrd/ccbrd.sock")
    );
    assert_eq!(
        relocated.ccbrd_tmux_socket_path(),
        relocated_root.join("ccbrd/tmux.sock")
    );
    assert_eq!(relocated.runtime_marker_status(), "missing");
}

#[test]
fn test_runtime_state_root_from_anchor_ref_rejects_invalid_payloads() {
    let tmp = tempfile::TempDir::new().unwrap();
    let anchor = tmp_path(&tmp, "repo/.ccbr");
    fs::create_dir_all(&anchor).unwrap();
    let ref_path = anchor.join("runtime-root-ref.json");

    fs::write(
        &ref_path,
        r#"{"schema_version":1,"record_type":"wrong","project_id":"proj-1","runtime_state_root":"/tmp/state"}"#,
    )
    .unwrap();
    assert!(runtime_state_root_from_anchor_ref(&anchor, Some("proj-1")).is_none());

    fs::write(
        &ref_path,
        r#"{"schema_version":1,"record_type":"ccbr_runtime_root_ref","project_id":"proj-1","runtime_state_root":"relative/state"}"#,
    )
    .unwrap();
    assert!(runtime_state_root_from_anchor_ref(&anchor, Some("proj-1")).is_none());
}

#[test]
fn test_runtime_project_anchor_from_path_rejects_invalid_marker_payloads() {
    let tmp = tempfile::TempDir::new().unwrap();
    let relocated_root = tmp_path(&tmp, "state-root");
    fs::create_dir_all(&relocated_root).unwrap();
    let marker_path = relocated_root.join("runtime-root.json");

    fs::write(
        &marker_path,
        format!(
            "{{\"schema_version\":1,\"record_type\":\"wrong\",\"project_id\":\"proj-1\",\"project_root\":\"/tmp/repo\",\"anchor_path\":\"/tmp/repo/.ccbr\",\"runtime_root_path\":\"{}\"}}",
            relocated_root
        ),
    )
    .unwrap();
    assert!(runtime_project_anchor_from_path(&relocated_root.join("agents")).is_none());

    fs::write(
        &marker_path,
        format!(
            "{{\"schema_version\":1,\"record_type\":\"ccbr_runtime_root\",\"project_id\":\"\",\"project_root\":\"/tmp/repo\",\"anchor_path\":\"/tmp/repo/.ccbr\",\"runtime_root_path\":\"{}\"}}",
            relocated_root
        ),
    )
    .unwrap();
    assert!(runtime_project_anchor_from_path(&relocated_root.join("agents")).is_none());

    fs::write(
        &marker_path,
        format!(
            "{{\"schema_version\":1,\"record_type\":\"ccbr_runtime_root\",\"project_id\":\"proj-1\",\"project_root\":\"/tmp/repo\",\"anchor_path\":\"/tmp/repo/.ccbr\",\"runtime_root_path\":\"{}\"}}",
            tmp_path(&tmp, "other-root")
        ),
    )
    .unwrap();
    assert!(runtime_project_anchor_from_path(&relocated_root.join("agents")).is_none());
}
