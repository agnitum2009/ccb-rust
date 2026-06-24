use camino::Utf8PathBuf;
use std::process::Command;

fn repo_root() -> Utf8PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Utf8PathBuf::from(manifest)
}

fn bin_path() -> std::path::PathBuf {
    std::env::var_os("CARGO_BIN_EXE_ccbr-release-checker")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            repo_root()
                .join("target/debug/ccbr-release-checker")
                .into_std_path_buf()
        })
}

#[test]
fn test_cli_help() {
    let output = Command::new(bin_path())
        .arg("--help")
        .output()
        .expect("binary exists");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    assert!(stdout.contains("ccbr-release-checker"));
    assert!(stdout.contains("check-release-state"));
    assert!(stdout.contains("check-assets"));
    assert!(stdout.contains("check-github"));
    assert!(stdout.contains("check-local"));
    assert!(stdout.contains("check-markdown"));
    assert!(stdout.contains("check-workflows"));
}

#[test]
fn test_markdown_helpers() {
    let body = r#"# Title
## v1.0.0 (2024-01-01)
Fixed bug.
## v0.9.0
Old.
"#;
    assert_eq!(
        ccbr_release_checker::markdown::markdown_section(body, "v1.0.0 (2024-01-01)"),
        Some("Fixed bug.".to_string())
    );
    let readme = r#"<details>
<summary><b>v1.0.0</b> - Release</summary>
- Fixed bug
</details>
"#;
    assert_eq!(
        ccbr_release_checker::markdown::readme_release_block(readme, "v1.0.0"),
        Some("- Fixed bug".to_string())
    );
    assert!(ccbr_release_checker::markdown::has_substantive_release_text(Some("- Fixed bug")));
    let versions = ccbr_release_checker::markdown::release_note_versions(readme);
    assert_eq!(versions, vec!["v1.0.0"]);
}

#[test]
fn test_local_checks_on_temp_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    // Initialize a git repo.
    let _ = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root.as_std_path())
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(root.as_std_path())
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(root.as_std_path())
        .output();

    // Write release files.
    std::fs::write(root.join("VERSION"), "1.0.0\n").unwrap();
    std::fs::write(root.join("ccb"), r#"VERSION = "1.0.0""#).unwrap();
    std::fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## v1.0.0 (2024-01-01)\n\n- Fixed bug\n",
    )
    .unwrap();
    std::fs::write(
        root.join("README.md"),
        r#"![version](version-1.0.0-orange.svg)
.ccbr/ccbr_memory.md
<details>
<summary><b>v1.0.0</b> - Release</summary>
- Fixed bug
</details>
## How to Install
```bash
git clone https://github.com/SeemSeam/claude_codex_bridge.git
```
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("README_zh.md"),
        r#"![version](version-1.0.0-orange.svg)
.ccbr/ccbr_memory.md
<details>
<summary><b>v1.0.0</b> - Release</summary>
- Fixed bug
</details>
## 如何安装
```bash
git clone https://github.com/SeemSeam/claude_codex_bridge.git
```
"#,
    )
    .unwrap();

    let mut report = ccbr_release_checker::Report::default();
    ccbr_release_checker::local::check_local_files(
        &root,
        "v1.0.0",
        "SeemSeam/claude_codex_bridge",
        &mut report,
    );
    assert!(!report.has_issues(), "{:#?}", report.issues);
}

#[test]
fn test_workflow_wait_status() {
    use ccbr_release_checker::workflows::format_workflow_wait_status;
    use serde_json::Value;
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert(
        "Tests".to_string(),
        Value::Object(
            vec![
                ("status".to_string(), Value::String("completed".to_string())),
                (
                    "conclusion".to_string(),
                    Value::String("success".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
        ),
    );
    let required = vec!["Tests".to_string(), "Missing".to_string()];
    assert_eq!(
        format_workflow_wait_status(&map, &required),
        "Missing=missing, Tests=completed/success"
    );
}
