use std::fs;
use std::path::PathBuf;

use ccbr_release_builder::{
    create_tarball, is_excluded_part, normalize_arch, normalize_release_platform,
    release_artifact_basename, release_artifact_name, release_build_arch, resolve_version,
    run_verify, write_release_metadata, write_sha256, BuildInfo, VerifyOptions,
};
use tempfile::TempDir;

fn make_dummy_repo(temp: &TempDir) -> PathBuf {
    let root = temp.path().to_path_buf();
    fs::write(root.join("VERSION"), "7.4.3-test\n").unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::create_dir_all(root.join("rust")).unwrap();
    fs::write(root.join("rust").join("Cargo.toml"), "[workspace]\n").unwrap();
    root
}

#[test]
fn test_normalize_arch() {
    assert_eq!(normalize_arch("x86_64"), "x86_64");
    assert_eq!(normalize_arch("amd64"), "x86_64");
    assert_eq!(normalize_arch("aarch64"), "aarch64");
    assert_eq!(normalize_arch("arm64"), "aarch64");
    assert_eq!(normalize_arch("  ARM64  "), "aarch64");
    assert_eq!(normalize_arch("unknown"), "unknown");
}

#[test]
fn test_normalize_release_platform() {
    assert_eq!(normalize_release_platform("Linux"), Some("linux".into()));
    assert_eq!(normalize_release_platform("linux"), Some("linux".into()));
    assert_eq!(normalize_release_platform("Darwin"), Some("macos".into()));
    assert_eq!(normalize_release_platform("macos"), Some("macos".into()));
    assert_eq!(normalize_release_platform("Windows"), None);
}

#[test]
fn test_release_artifact_basename() {
    assert_eq!(
        release_artifact_basename("linux", "x86_64"),
        Some("ccbr-linux-x86_64".into())
    );
    assert_eq!(
        release_artifact_basename("linux", "arm64"),
        Some("ccbr-linux-aarch64".into())
    );
    assert_eq!(
        release_artifact_basename("macos", "x86_64"),
        Some("ccbr-macos-universal".into())
    );
    assert_eq!(release_artifact_basename("windows", "x86_64"), None);
}

#[test]
fn test_release_artifact_name() {
    assert_eq!(
        release_artifact_name("linux", "x86_64"),
        Some("ccbr-linux-x86_64.tar.gz".into())
    );
}

#[test]
fn test_release_build_arch() {
    assert_eq!(release_build_arch("linux", "x86_64"), Some("x86_64".into()));
    assert_eq!(
        release_build_arch("macos", "x86_64"),
        Some("universal".into())
    );
}

#[test]
fn test_excluded_parts() {
    assert!(is_excluded_part(".git"));
    assert!(is_excluded_part("target"));
    assert!(is_excluded_part("__pycache__"));
    assert!(is_excluded_part(".tmp_test_env_foo"));
    assert!(!is_excluded_part("src"));
}

#[test]
fn test_resolve_version() {
    let temp = TempDir::new().unwrap();
    let root = make_dummy_repo(&temp);
    assert_eq!(resolve_version(&root).unwrap(), "7.4.3-test");

    fs::remove_file(root.join("VERSION")).unwrap();
    assert!(resolve_version(&root).is_err());
}

#[test]
fn test_write_release_metadata() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().to_path_buf();
    let build_info = BuildInfo {
        version: "7.4.3".into(),
        commit: Some("abc1234".into()),
        date: Some("2026-06-13".into()),
        build_time: "2026-06-13T00:00:00Z".into(),
        platform: "linux".into(),
        arch: "x86_64".into(),
        channel: "stable".into(),
        source_kind: "release".into(),
        install_mode: "release".into(),
    };
    write_release_metadata(&root, &build_info).unwrap();

    let version = fs::read_to_string(root.join("VERSION")).unwrap();
    assert_eq!(version.trim(), "7.4.3");

    let json = fs::read_to_string(root.join("BUILD_INFO.json")).unwrap();
    assert!(json.contains("\"version\": \"7.4.3\""));
    assert!(json.contains("\"arch\": \"x86_64\""));
}

#[test]
fn test_create_tarball_and_sha256_and_verify() {
    let temp = TempDir::new().unwrap();
    let stage_root = temp.path().join("stage");
    let artifact_root = stage_root.join("ccbr-linux-x86_64");
    fs::create_dir_all(&artifact_root).unwrap();
    fs::write(artifact_root.join("BUILD_INFO.json"), "{}\n").unwrap();

    // Placeholder required binaries so verification passes.
    let bin_dir = artifact_root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    for name in ccbr_release_builder::REQUIRED_BINARIES {
        let dest = if *name == "ccbr" {
            artifact_root.join(name)
        } else {
            bin_dir.join(name)
        };
        fs::write(&dest, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest, perms).unwrap();
        }
    }

    let output_dir = temp.path().join("dist");
    fs::create_dir_all(&output_dir).unwrap();
    let artifact_path = output_dir.join("ccbr-linux-x86_64.tar.gz");

    create_tarball(&stage_root, &artifact_root, &artifact_path).unwrap();
    assert!(artifact_path.is_file());

    let sha_path = output_dir.join("SHA256SUMS");
    write_sha256(&artifact_path, &sha_path).unwrap();
    assert!(sha_path.is_file());

    let sha_contents = fs::read_to_string(&sha_path).unwrap();
    assert!(sha_contents.contains("ccbr-linux-x86_64.tar.gz"));

    run_verify(&VerifyOptions {
        artifact_path: artifact_path.clone(),
    })
    .unwrap();
}

#[test]
fn test_copy_repo_tree_respects_exclusions() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("VERSION"), "1.0.0\n").unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\n").unwrap();

    let excluded_dir = root.join("target");
    fs::create_dir_all(&excluded_dir).unwrap();
    fs::write(excluded_dir.join("artifact"), "should not be copied").unwrap();

    let pycache = root.join("__pycache__");
    fs::create_dir_all(&pycache).unwrap();
    fs::write(pycache.join("cache.pyc"), "should not be copied").unwrap();

    let nested_src = root.join("src");
    fs::create_dir_all(&nested_src).unwrap();
    fs::write(nested_src.join("main.rs"), "fn main() {}\n").unwrap();

    let destination = temp.path().join("copy");
    ccbr_release_builder::copy_repo_tree(&root, &destination, &[]).unwrap();

    assert!(destination.join("VERSION").is_file());
    assert!(destination.join("Cargo.toml").is_file());
    assert!(destination.join("src").join("main.rs").is_file());
    assert!(!destination.join("target").exists());
    assert!(!destination.join("__pycache__").exists());
}

#[test]
fn test_export_release_tree_allow_dirty_copies_worktree() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("VERSION"), "1.0.0\n").unwrap();
    fs::write(root.join("file.txt"), "hello\n").unwrap();

    let destination = temp.path().join("exported");
    ccbr_release_builder::export_release_tree(&root, &destination, "HEAD", true, &[]).unwrap();

    assert!(destination.join("VERSION").is_file());
    assert!(destination.join("file.txt").is_file());
}

#[test]
fn test_verify_rejects_missing_build_info() {
    let temp = TempDir::new().unwrap();
    let stage_root = temp.path().join("stage");
    let artifact_root = stage_root.join("ccbr-linux-x86_64");
    fs::create_dir_all(&artifact_root).unwrap();
    fs::write(artifact_root.join("VERSION"), "1.0.0\n").unwrap();

    let output_dir = temp.path().join("dist");
    fs::create_dir_all(&output_dir).unwrap();
    let artifact_path = output_dir.join("ccbr-linux-x86_64.tar.gz");

    create_tarball(&stage_root, &artifact_root, &artifact_path).unwrap();
    assert!(run_verify(&VerifyOptions { artifact_path }).is_err());
}
