//! Mirrors Python `test/test_cli_management_install.py`.

use ccb_cli::management_runtime::install::{
    build_unix_installer_env, resolve_installer_paths, resolve_managed_install_dir, run_installer,
};
use std::path::PathBuf;
use std::process::Command;

/// Serialize tests that mutate process-global env vars.
static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn resolve_installer_paths_uses_live_source_repo_with_managed_prefix() {
    let _guard = ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let source_dir = tmp.path().join("source-install");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::write(source_dir.join("install.sh"), "#!/usr/bin/env bash\n").unwrap();
    std::fs::create_dir(source_dir.join(".git")).unwrap();

    let managed_prefix = tmp.path().join("managed-install");
    std::env::set_var(
        "CODEX_INSTALL_PREFIX",
        managed_prefix.to_string_lossy().as_ref(),
    );

    let (source_root, install_dir) = resolve_installer_paths("install", &source_dir);
    assert_eq!(source_root, source_dir);
    assert_eq!(install_dir, managed_prefix);

    std::env::remove_var("CODEX_INSTALL_PREFIX");
}

#[test]
fn resolve_managed_install_dir_uses_managed_prefix_for_source_repo() {
    let _guard = ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let source_dir = tmp.path().join("source-install");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::write(source_dir.join("install.sh"), "#!/usr/bin/env bash\n").unwrap();
    std::fs::create_dir(source_dir.join(".git")).unwrap();

    let managed_prefix = tmp.path().join("managed-install");
    std::env::set_var(
        "CODEX_INSTALL_PREFIX",
        managed_prefix.to_string_lossy().as_ref(),
    );

    let install_dir = resolve_managed_install_dir(&source_dir);
    assert_eq!(install_dir, managed_prefix);

    std::env::remove_var("CODEX_INSTALL_PREFIX");
}

#[test]
fn build_unix_installer_env_marks_source_repo_root() {
    let _guard = ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let source_dir = tmp.path().join("source-install");
    std::fs::create_dir(&source_dir).unwrap();
    let install_dir = tmp.path().join("managed-install");

    // Skip if git is not available in the test environment.
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }

    let init = Command::new("git")
        .arg("-C")
        .arg(&source_dir)
        .arg("init")
        .output()
        .unwrap();
    assert!(init.status.success(), "git init failed");

    Command::new("git")
        .arg("-C")
        .arg(&source_dir)
        .args(["config", "user.email", "test@example.com"])
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&source_dir)
        .args(["config", "user.name", "Test"])
        .status()
        .unwrap();

    std::fs::write(source_dir.join("marker.txt"), "x").unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&source_dir)
        .args(["add", "."])
        .status()
        .unwrap();
    Command::new("git")
        .arg("-C")
        .arg(&source_dir)
        .args(["commit", "-m", "initial"])
        .status()
        .unwrap();

    for key in [
        "CCB_SOURCE_KIND",
        "CCB_SOURCE_ROOT",
        "CCB_GIT_COMMIT",
        "CCB_GIT_DATE",
    ] {
        std::env::remove_var(key);
    }

    let env = build_unix_installer_env(&install_dir, &source_dir);
    assert_eq!(
        env.get("CODEX_INSTALL_PREFIX").unwrap(),
        &install_dir.to_string_lossy().to_string()
    );
    assert_eq!(env.get("CCB_SOURCE_KIND").unwrap(), "source");
    assert_eq!(
        env.get("CCB_SOURCE_ROOT").unwrap(),
        &source_dir.to_string_lossy().to_string()
    );
    let commit = env
        .get("CCB_GIT_COMMIT")
        .expect("CCB_GIT_COMMIT should be set");
    assert!(!commit.is_empty(), "CCB_GIT_COMMIT should not be empty");
    let date = env
        .get("CCB_GIT_DATE")
        .expect("CCB_GIT_DATE should be set");
    assert_eq!(date.len(), 10, "CCB_GIT_DATE should be YYYY-MM-DD");
}

#[test]
fn run_installer_stages_and_normalizes_crlf_checkout() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source_dir = tmp.path().join("source-install");
    std::fs::create_dir(&source_dir).unwrap();
    let install_sh = source_dir.join("install.sh");
    let marker_path = source_dir.join("ran.txt");

    std::fs::write(
        &install_sh,
        b"#!/usr/bin/env bash\r\n\
          set -euo pipefail\r\n\
          printf '%s\\n' \"$0\" > \"$CODEX_INSTALL_PREFIX/ran.txt\"\r\n",
    )
    .unwrap();

    let code = run_installer("install", &source_dir);
    assert_eq!(code, 0, "installer should exit 0");

    let ran_from = std::fs::read_to_string(&marker_path)
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(
        PathBuf::from(&ran_from),
        install_sh,
        "installer should run from staged copy, not original checkout"
    );
    assert!(
        ran_from.contains("ccb-installer-"),
        "staged path should contain ccb-installer- prefix: {ran_from}"
    );
}
