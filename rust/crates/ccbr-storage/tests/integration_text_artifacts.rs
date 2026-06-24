use camino::Utf8PathBuf;
use ccbr_storage::paths::PathLayout;
use ccbr_storage::text_artifacts::{
    maybe_spill_text, read_text_artifact, sweep_expired_text_artifacts, validate_text_artifact_ref,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn tmp_path(tmp: &tempfile::TempDir, tail: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(tmp.path().join(tail)).unwrap()
}

#[test]
fn test_maybe_spill_text_keeps_small_text_inline() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo-inline"));

    let (body, artifact) = maybe_spill_text(
        &layout,
        "short body",
        "ask-request",
        "agent1",
        "large body",
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(body, "short body");
    assert!(artifact.is_none());
    assert!(!layout.ccbd_text_artifacts_dir().exists());
}

#[test]
fn test_maybe_spill_text_writes_large_text_artifact() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo-spill"));
    let text = "x".repeat(5000);

    let (body, artifact) = maybe_spill_text(
        &layout,
        &text,
        "ask-request",
        "agent1",
        "large body",
        None,
        None,
        Some("2026-05-22T00:00:00Z"),
    )
    .unwrap();

    assert!(artifact.is_some());
    assert!(body.len() <= 4096);
    assert!(body.contains("large body"));
    let artifact = artifact.unwrap();
    let path = Utf8PathBuf::from(&artifact.path);
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    assert_eq!(read_text_artifact(&layout, &artifact).unwrap(), text);
}

#[test]
fn test_validate_text_artifact_ref_rejects_path_escape() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo-escape"));
    let outside = tmp_path(&tmp, "outside.txt");
    fs::write(&outside, "secret").unwrap();

    let artifact = ccbr_storage::text_artifacts::TextArtifact {
        schema_version: 1,
        kind: "text".into(),
        artifact_id: "id".into(),
        path: outside.to_string(),
        bytes: 6,
        sha256: "bad".into(),
        encoding: "utf-8".into(),
        preview: "".into(),
        created_at: "2026-05-22T00:00:00Z".into(),
        expires_at: "2026-05-23T00:00:00Z".into(),
    };

    let result = validate_text_artifact_ref(&layout, Some(&artifact));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("escapes"));
}

#[test]
fn test_validate_text_artifact_ref_rejects_sha_mismatch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo-sha"));
    let (_, artifact) = maybe_spill_text(
        &layout,
        &"x".repeat(5000),
        "reply",
        "job1",
        "large reply",
        None,
        None,
        None,
    )
    .unwrap();
    assert!(artifact.is_some());
    let mut artifact = artifact.unwrap();
    artifact.sha256 = "0".repeat(64);

    let result = validate_text_artifact_ref(&layout, Some(&artifact));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("sha256"));
}

#[test]
fn test_sweep_expired_text_artifacts_removes_old_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let layout = PathLayout::new(tmp_path(&tmp, "repo-sweep"));
    let (_, artifact) = maybe_spill_text(
        &layout,
        &"x".repeat(5000),
        "reply",
        "job1",
        "large reply",
        None,
        None,
        None,
    )
    .unwrap();
    assert!(artifact.is_some());
    let path = Utf8PathBuf::from(&artifact.unwrap().path);
    let old = SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 60 * 60);
    let old_secs = old.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let times = filetime::FileTime::from_unix_time(old_secs, 0);
    filetime::set_file_mtime(&path, times).unwrap();

    let removed = sweep_expired_text_artifacts(&layout, None).unwrap();

    assert!(removed.contains(&path));
    assert!(!path.exists());
}
