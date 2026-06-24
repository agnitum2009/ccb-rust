use ccbr_providers::session_paths::{find_project_ccbr_dir, session_file_for_runtime_dir};
use serde_json::json;
use tempfile::TempDir;

const PROVIDERS: &[&str] = &["codex", "claude", "gemini"];

fn write_runtime_root_marker(
    relocated_root: &std::path::Path,
    project_root: &std::path::Path,
    runtime_root_path: &std::path::Path,
) {
    let marker = relocated_root.join("runtime-root.json");
    std::fs::create_dir_all(relocated_root).unwrap();
    std::fs::write(
        &marker,
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "record_type": "ccbr_runtime_root",
            "project_id": "proj-1",
            "project_root": project_root.to_str().unwrap(),
            "anchor_path": project_root.join(".ccbr").to_str().unwrap(),
            "runtime_root_path": runtime_root_path.to_str().unwrap(),
            "created_at": "2026-05-07T00:00:00Z",
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn test_session_file_for_runtime_dir_follows_relocated_runtime_anchor() {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-relocated-session-path");
    let anchor = project_root.join(".ccbr");
    std::fs::create_dir_all(&anchor).unwrap();
    let relocated_root = tmp.path().join("state-root");

    write_runtime_root_marker(&relocated_root, &project_root, &relocated_root);

    for provider in PROVIDERS {
        let runtime_dir = relocated_root
            .join("agents")
            .join("reviewer")
            .join("provider-runtime")
            .join(provider);
        std::fs::create_dir_all(&runtime_dir).unwrap();

        assert_eq!(
            find_project_ccbr_dir(&runtime_dir),
            Some(anchor.clone()),
            "provider={provider}"
        );
        assert_eq!(
            session_file_for_runtime_dir(provider, &runtime_dir),
            Some(anchor.join(format!(".{provider}-reviewer-session"))),
            "provider={provider}"
        );
    }
}

#[test]
fn test_session_file_for_runtime_dir_rejects_invalid_runtime_marker() {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-invalid-relocated-session-path");
    let anchor = project_root.join(".ccbr");
    std::fs::create_dir_all(&anchor).unwrap();
    let relocated_root = tmp.path().join("state-root-invalid");
    let different_root = tmp.path().join("different-root");

    write_runtime_root_marker(&relocated_root, &project_root, &different_root);

    for provider in PROVIDERS {
        let runtime_dir = relocated_root
            .join("agents")
            .join("reviewer")
            .join("provider-runtime")
            .join(provider);
        std::fs::create_dir_all(&runtime_dir).unwrap();

        assert!(
            find_project_ccbr_dir(&runtime_dir).is_none(),
            "provider={provider}"
        );
        assert!(
            session_file_for_runtime_dir(provider, &runtime_dir).is_none(),
            "provider={provider}"
        );
    }
}

#[test]
fn test_session_file_for_runtime_dir_finds_local_ccbr_first() {
    let tmp = TempDir::new().unwrap();
    let project_root = tmp.path().join("repo");
    let local_ccb = project_root.join(".ccbr");
    std::fs::create_dir_all(&local_ccb).unwrap();
    let runtime_dir = local_ccb
        .join("agents")
        .join("reviewer")
        .join("provider-runtime")
        .join("claude");
    std::fs::create_dir_all(&runtime_dir).unwrap();

    assert_eq!(find_project_ccbr_dir(&runtime_dir), Some(local_ccb.clone()));
    assert_eq!(
        session_file_for_runtime_dir("claude", &runtime_dir),
        Some(local_ccb.join(".claude-reviewer-session"))
    );
}
