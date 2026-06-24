//! Mirrors Python `test/test_v2_runtime_launch.py` workspace-preparation subset.

use camino::Utf8PathBuf;
use ccbr_providers::workspace_preparation::{provider_hook_home_root, resolve_gemini_home_root};
use ccbr_storage::paths::PathLayout;

fn tmp_layout() -> (tempfile::TempDir, PathLayout) {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().join("repo")).unwrap();
    let layout = PathLayout::new(root);
    (tmp, layout)
}

#[test]
fn test_provider_hook_home_root_returns_none_for_unknown_provider() {
    let (_tmp, layout) = tmp_layout();
    let runtime_dir = layout.agent_provider_runtime_dir("agent1", "unknown");
    assert!(provider_hook_home_root(&layout, "unknown", "agent1", &runtime_dir, None).is_none());
}

#[test]
fn test_resolve_gemini_home_root() {
    let (_tmp, layout) = tmp_layout();
    let path = resolve_gemini_home_root(&layout, "agent1");
    assert_eq!(
        path,
        layout
            .agent_provider_state_dir("agent1", "gemini")
            .join("home")
    );
}

#[test]
fn test_provider_hook_home_root_returns_claude_home_root() {
    let (_tmp, layout) = tmp_layout();
    let runtime_dir = layout.agent_provider_runtime_dir("agent1", "claude");
    let home_root = provider_hook_home_root(&layout, "claude", "agent1", &runtime_dir, None);
    assert!(home_root.is_some());
    let home_root = home_root.unwrap();
    assert!(home_root.as_str().contains("provider-state"));
    assert!(home_root.as_str().ends_with("/home"));
}
