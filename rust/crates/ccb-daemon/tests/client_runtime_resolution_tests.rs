//! Mirrors Python `test/test_ccbd_client_resolution.py`.

use ccb_daemon::client_runtime::resolution::{resolve_work_dir, resolve_work_dir_with_registry};
use ccb_provider_core::runtime_specs::{CLAUDE_CLIENT_SPEC, CODEX_CLIENT_SPEC};
use std::path::PathBuf;

fn write_file(path: PathBuf, text: &str) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn test_resolve_work_dir_uses_project_root_for_ccb_session_file() {
    let tmp = tempfile::tempdir().unwrap();
    let session_file = write_file(tmp.path().join(".ccb").join(".claude-session"), "{}");
    let (work_dir, resolved) = resolve_work_dir(
        &CLAUDE_CLIENT_SPEC,
        Some(session_file.to_str().unwrap()),
        None,
        Some(tmp.path()),
    )
    .unwrap();

    assert_eq!(work_dir, tmp.path());
    assert_eq!(resolved.unwrap(), session_file.canonicalize().unwrap());
}

#[test]
fn test_resolve_work_dir_rejects_relative_session_file_in_claude_code() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(tmp.path().join(".claude-session"), "{}");
    std::env::set_var("CLAUDECODE", "1");
    let result = resolve_work_dir(
        &CLAUDE_CLIENT_SPEC,
        Some(".claude-session"),
        None,
        Some(tmp.path()),
    );
    std::env::remove_var("CLAUDECODE");

    let err = result.unwrap_err();
    assert!(err.contains("absolute path"), "{err}");
}

#[test]
fn test_resolve_work_dir_rejects_wrong_filename() {
    let tmp = tempfile::tempdir().unwrap();
    let wrong = write_file(tmp.path().join(".wrong-session"), "{}");
    let result = resolve_work_dir(
        &CODEX_CLIENT_SPEC,
        Some(wrong.to_str().unwrap()),
        None,
        Some(tmp.path()),
    );

    let err = result.unwrap_err();
    assert!(err.contains("expected filename"), "{err}");
}

#[test]
fn test_resolve_work_dir_with_registry_finds_project_session_file() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    let workspace = project_root.join(".ccb").join("workspaces").join("agent1");
    std::fs::create_dir_all(&workspace).unwrap();
    let session_file = write_file(project_root.join(".ccb").join(".codex-session"), "{}");

    let (work_dir, resolved) = resolve_work_dir_with_registry(
        &CODEX_CLIENT_SPEC,
        "codex",
        None,
        None,
        Some(&workspace),
    )
    .unwrap();

    assert_eq!(work_dir, workspace);
    assert_eq!(resolved.unwrap(), session_file);
}

#[test]
fn test_resolve_work_dir_with_registry_rejects_registry_only_mode_without_binding() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("CCB_REGISTRY_ONLY", "1");
    let result = resolve_work_dir_with_registry(
        &CODEX_CLIENT_SPEC,
        "codex",
        None,
        None,
        Some(tmp.path()),
    );
    std::env::remove_var("CCB_REGISTRY_ONLY");

    let err = result.unwrap_err();
    assert!(err.contains("no longer supported"), "{err}");
}
