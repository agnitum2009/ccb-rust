//! Mirrors Python `test/test_provider_helper_cleanup.py`.

use ccbr_agents::models::AgentState;
use ccbr_providers::helper_cleanup::{
    cleanup_stale_runtime_helper, terminate_helper_manifest_path,
};
use ccbr_providers::helper_manifest::ProviderRuntimeView;
use ccbr_storage::paths::PathLayout;

fn write_helper(path: &camino::Utf8Path, runtime_generation: i64, leader_pid: i64, pgid: i64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = format!(
        r#"{{"schema_version":1,"record_type":"provider_helper_manifest","agent_name":"agent1","runtime_generation":{},"helper_kind":"codex_bridge","leader_pid":{},"pgid":{},"started_at":"2026-04-22T00:00:00Z","owner_daemon_generation":5,"state":"running"}}"#,
        runtime_generation, leader_pid, pgid
    );
    std::fs::write(path, text).unwrap();
}

fn runtime_view(
    agent_name: &str,
    provider: &str,
    runtime_generation: Option<i64>,
    state: AgentState,
    runtime_root: &str,
) -> ProviderRuntimeView {
    ProviderRuntimeView {
        agent_name: agent_name.to_string(),
        provider: provider.to_string(),
        state: Some(state),
        runtime_root: runtime_root.to_string(),
        runtime_generation,
        started_at: None,
        last_seen_at: None,
        daemon_generation: None,
    }
}

#[test]
fn test_cleanup_stale_runtime_helper_reaps_superseded_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("repo")).unwrap();
    let layout = PathLayout::new(root);
    let helper_path = layout.agent_helper_path("agent1");
    // Use very large PIDs that are extremely unlikely to exist.
    write_helper(&helper_path, 1, 777_777, 888_888);

    let runtime = runtime_view(
        "agent1",
        "codex",
        Some(2),
        AgentState::Idle,
        "/tmp/runtime-new",
    );
    let reaped = cleanup_stale_runtime_helper(&layout, &runtime);

    assert!(reaped);
    assert!(!helper_path.exists());
}

#[test]
fn test_cleanup_stale_runtime_helper_keeps_current_owner_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("repo")).unwrap();
    let layout = PathLayout::new(root);
    let helper_path = layout.agent_helper_path("agent1");
    write_helper(&helper_path, 3, 777_777, 888_888);

    let runtime = runtime_view(
        "agent1",
        "codex",
        Some(3),
        AgentState::Idle,
        "/tmp/runtime-current",
    );
    let reaped = cleanup_stale_runtime_helper(&layout, &runtime);

    assert!(!reaped);
    assert!(helper_path.exists());
}

#[test]
fn test_cleanup_stale_runtime_helper_requires_canonical_runtime_generation() {
    let tmp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("repo")).unwrap();
    let layout = PathLayout::new(root);
    let helper_path = layout.agent_helper_path("agent1");
    write_helper(&helper_path, 3, 777_777, 888_888);

    // runtime_generation None means the runtime cannot prove ownership, so the
    // stale manifest is reaped.
    let runtime = runtime_view(
        "agent1",
        "codex",
        None,
        AgentState::Idle,
        "/tmp/runtime-current",
    );
    let reaped = cleanup_stale_runtime_helper(&layout, &runtime);

    assert!(reaped);
    assert!(!helper_path.exists());
}

#[test]
fn test_terminate_helper_manifest_path_clears_file_when_leader_is_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("repo")).unwrap();
    let layout = PathLayout::new(root);
    let helper_path = layout.agent_helper_path("agent1");
    write_helper(&helper_path, 1, 501_501, 601_601);

    let terminated = terminate_helper_manifest_path(&helper_path);

    assert!(terminated);
    assert!(!helper_path.exists());
}
