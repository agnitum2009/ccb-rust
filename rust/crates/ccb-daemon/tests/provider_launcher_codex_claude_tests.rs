//! Shape tests for new provider launch branches.
//!
//! These tests use `ProviderLauncher::build_plan` so they do not require a
//! running tmux server.

use std::path::PathBuf;

use ccb_daemon::provider_launcher::{LaunchContext, ProviderLauncher};

fn minimal_context<'a>(
    provider: &'a str,
    agent_name: &'a str,
    project_root: &'a str,
    workspace: &'a str,
) -> LaunchContext<'a> {
    LaunchContext {
        provider,
        agent_name,
        project_id: "proj-1",
        project_root,
        workspace_path: workspace,
        pane_id: "%42",
        socket_path: "/tmp/ccb-test.sock",
        restore: false,
        command_template: None,
        startup_args: &[],
        auto_permission: false,
        spec: None,
    }
}

#[test]
fn test_codex_launch_command_and_session_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let ws = root.clone();
    let launcher = ProviderLauncher::new();
    let ctx = minimal_context("codex", "agent1", &root, &ws);

    let result = launcher.build_plan(&ctx).unwrap();

    assert!(result.command.contains("codex"));
    assert!(result.session_payload.is_some());
    let session_path = result.session_path.expect("codex session path");
    assert_eq!(
        session_path,
        PathBuf::from(&root)
            .join(".ccbr")
            .join(".codex-agent1-session")
    );
}

#[test]
fn test_claude_launch_command_and_session_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let ws = root.clone();
    let launcher = ProviderLauncher::new();
    let ctx = minimal_context("claude", "reviewer", &root, &ws);

    let result = launcher.build_plan(&ctx).unwrap();

    assert!(result.command.contains("claude"));
    assert!(result.session_payload.is_some());
    let session_path = result.session_path.expect("claude session path");
    assert_eq!(
        session_path,
        PathBuf::from(&root)
            .join(".ccbr")
            .join(".claude-reviewer-session")
    );
}

#[test]
fn test_gemini_launch_command_and_session_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let ws = root.clone();
    let launcher = ProviderLauncher::new();
    let ctx = minimal_context("gemini", "gemini", &root, &ws);

    let result = launcher.build_plan(&ctx).unwrap();

    assert!(result.command.contains("gemini"));
    assert!(result.session_payload.is_some());
    let session_path = result.session_path.expect("gemini session path");
    assert_eq!(
        session_path,
        PathBuf::from(&root)
            .join(".ccbr")
            .join(".gemini-gemini-session")
    );
}

#[test]
fn test_agy_launch_command_and_session_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let ws = root.clone();
    let launcher = ProviderLauncher::new();
    let ctx = minimal_context("agy", "agy", &root, &ws);

    let result = launcher.build_plan(&ctx).unwrap();

    assert!(result.command.contains("agy"));
    assert!(result.session_payload.is_some());
    let session_path = result.session_path.expect("agy session path");
    assert_eq!(
        session_path,
        PathBuf::from(&root).join(".ccbr").join(".agy-agy-session")
    );
}

#[test]
fn test_droid_launch_command_and_session_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_string_lossy().to_string();
    let ws = root.clone();
    let launcher = ProviderLauncher::new();
    let ctx = minimal_context("droid", "droid", &root, &ws);

    let result = launcher.build_plan(&ctx).unwrap();

    assert!(result.command.contains("droid"));
    assert!(result.session_payload.is_some());
    let session_path = result.session_path.expect("droid session path");
    assert_eq!(
        session_path,
        PathBuf::from(&root)
            .join(".ccbr")
            .join(".droid-droid-session")
    );
}
