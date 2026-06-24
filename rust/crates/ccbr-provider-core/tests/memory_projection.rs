use ccbr_provider_core::memory_projection::{
    memory_projection_result, record_memory_projection_event, same_memory_projection_signature,
    text_file_sha256, write_projection_event_and_marker,
};
use serde_json::json;

fn sha256_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

#[test]
fn test_memory_projection_result_normalizes_warning_and_error_fields() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = memory_projection_result(
        "failed",
        "missing_project_context",
        &tmp.path().join("AGENTS.md"),
        None,
        None,
        Some(&["warn".to_string(), "".to_string(), "also-warn".to_string()]),
        None,
    );

    assert_eq!(result.status, "failed");
    assert_eq!(result.reason, "missing_project_context");
    assert_eq!(result.path, tmp.path().join("AGENTS.md").to_string_lossy());
    assert_eq!(result.warnings, vec!["warn", "also-warn"]);
    assert_eq!(result.error_detail, "");
}

#[test]
fn test_record_memory_projection_event_uses_caller_provider_and_dedupes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let event_path = tmp.path().join("events.jsonl");
    let marker_path = tmp.path().join("projection-marker.json");
    let result = memory_projection_result(
        "ok",
        "written",
        &tmp.path().join("CLAUDE.md"),
        Some("abc123"),
        Some(2),
        Some(&["careful".to_string()]),
        None,
    );

    record_memory_projection_event(
        &result,
        "claude",
        Some(&event_path),
        Some(&marker_path),
        Some("agent1"),
    )
    .unwrap();
    record_memory_projection_event(
        &result,
        "claude",
        Some(&event_path),
        Some(&marker_path),
        Some("agent1"),
    )
    .unwrap();

    let events: Vec<serde_json::Value> = std::fs::read_to_string(&event_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "claude_memory_projection_ok");
    assert_eq!(events[0]["provider"], "claude");
    assert_eq!(events[0]["agent_name"], "agent1");
    assert_eq!(
        events[0]["projection_path"],
        tmp.path().join("CLAUDE.md").to_string_lossy().to_string()
    );
    assert_eq!(events[0]["sha256"], "abc123");
    assert_eq!(events[0]["source_count"], 2);
    assert_eq!(events[0]["warnings"], json!(["careful"]));

    let marker: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&marker_path).unwrap()).unwrap();
    assert_eq!(
        marker,
        json!({
            "status": "ok",
            "reason": "written",
            "path": tmp.path().join("CLAUDE.md").to_string_lossy().to_string(),
            "sha256": "abc123",
            "warnings": ["careful"],
        })
    );
}

#[test]
fn test_record_memory_projection_event_requires_provider_and_targets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = memory_projection_result(
        "ok",
        "written",
        &tmp.path().join("GEMINI.md"),
        None,
        None,
        None,
        None,
    );

    record_memory_projection_event(
        &result,
        "",
        Some(&tmp.path().join("events.jsonl")),
        Some(&tmp.path().join("marker.json")),
        Some("agent1"),
    )
    .unwrap();
    record_memory_projection_event(
        &result,
        "gemini",
        None,
        Some(&tmp.path().join("marker.json")),
        Some("agent1"),
    )
    .unwrap();
    record_memory_projection_event(
        &result,
        "gemini",
        Some(&tmp.path().join("events.jsonl")),
        None,
        Some("agent1"),
    )
    .unwrap();
    record_memory_projection_event(
        &result,
        "gemini",
        Some(&tmp.path().join("events.jsonl")),
        Some(&tmp.path().join("marker.json")),
        None,
    )
    .unwrap();

    assert!(!tmp.path().join("events.jsonl").exists());
    assert!(!tmp.path().join("marker.json").exists());
}

#[test]
fn test_same_memory_projection_signature_requires_sha_for_unchanged_fast_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let marker = tmp.path().join("marker.json");
    std::fs::write(
        &marker,
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "reason": "written",
            "path": tmp.path().join("AGENTS.md").to_string_lossy().to_string(),
            "sha256": "",
            "warnings": [],
        }))
        .unwrap()
            + "\n",
    )
    .unwrap();

    assert!(!same_memory_projection_signature(
        &marker,
        &json!({
            "status": "skipped",
            "reason": "unchanged",
            "path": tmp.path().join("AGENTS.md").to_string_lossy().to_string(),
            "sha256": "",
            "warnings": [],
        })
    ));
}

#[test]
fn test_same_memory_projection_signature_allows_extra_fields_exact_match() {
    let tmp = tempfile::TempDir::new().unwrap();
    let marker = tmp.path().join("opencode-marker.json");
    let signature = json!({
        "status": "ok",
        "reason": "written",
        "path": tmp.path().join("bundle.md").to_string_lossy().to_string(),
        "config_path": tmp.path().join("opencode.json").to_string_lossy().to_string(),
        "bundle_path": tmp.path().join("bundle.md").to_string_lossy().to_string(),
        "sha256": "bundle-sha",
        "config_sha256": "config-sha",
        "warnings": [],
        "config_merge_status": "merged",
        "config_merge_reason": "merged_project_opencode_json",
    });
    std::fs::write(
        &marker,
        serde_json::to_string_pretty(&signature).unwrap() + "\n",
    )
    .unwrap();

    assert!(same_memory_projection_signature(&marker, &signature));
    let mut changed = signature.clone();
    changed["config_sha256"] = json!("other");
    assert!(!same_memory_projection_signature(&marker, &changed));
}

#[test]
fn test_same_memory_projection_signature_skipped_fast_path_uses_base_fields() {
    let tmp = tempfile::TempDir::new().unwrap();
    let marker = tmp.path().join("opencode-marker.json");
    std::fs::write(
        &marker,
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "reason": "written",
            "path": tmp.path().join("bundle.md").to_string_lossy().to_string(),
            "config_path": tmp.path().join("opencode.json").to_string_lossy().to_string(),
            "sha256": "bundle-sha",
            "config_sha256": "old-config-sha",
            "warnings": [],
        }))
        .unwrap()
            + "\n",
    )
    .unwrap();

    assert!(same_memory_projection_signature(
        &marker,
        &json!({
            "status": "skipped",
            "reason": "unchanged",
            "path": tmp.path().join("bundle.md").to_string_lossy().to_string(),
            "config_path": tmp.path().join("opencode.json").to_string_lossy().to_string(),
            "sha256": "bundle-sha",
            "config_sha256": "new-config-sha",
            "warnings": [],
        })
    ));
}

#[test]
fn test_write_projection_event_and_marker_appends_event_and_writes_signature() {
    let tmp = tempfile::TempDir::new().unwrap();
    let event_path = tmp.path().join("events.jsonl");
    let marker_path = tmp.path().join("projection-marker.json");
    let event = json!({
        "record_type": "agent_event",
        "event_type": "opencode_memory_projection_ok",
        "provider": "opencode",
    });
    let signature = json!({
        "status": "ok",
        "reason": "written",
        "path": tmp.path().join("memory.md").to_string_lossy().to_string(),
        "sha256": "abc123",
        "warnings": [],
    });

    write_projection_event_and_marker(&event, &signature, &event_path, &marker_path).unwrap();

    let events: Vec<serde_json::Value> = std::fs::read_to_string(&event_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events, vec![event]);

    let marker: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&marker_path).unwrap()).unwrap();
    assert_eq!(marker, signature);
}

#[test]
fn test_text_file_sha256_hashes_existing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("memory.md");
    std::fs::write(&path, "memory bundle\n").unwrap();

    assert_eq!(text_file_sha256(&path), sha256_text("memory bundle\n"));
}

#[test]
fn test_text_file_sha256_returns_empty_for_missing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert_eq!(text_file_sha256(&tmp.path().join("missing.md")), "");
}
