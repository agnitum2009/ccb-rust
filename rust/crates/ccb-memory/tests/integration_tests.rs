use ccb_memory::{
    agent_private_memory_path, auto_source_candidates, materialize_runtime_memory_bundle,
    ContextFormatter, ContextTransfer, ConversationDeduper, ConversationEntry, ProjectMemorySource,
    TransferContext,
};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Deduper tests
// ---------------------------------------------------------------------------

#[test]
fn test_deduper_strips_protocol_markers() {
    let deduper = ConversationDeduper::new();
    let text = "Hello\nCCB_REQ_ID: 20240101-120000-000-1-1\nWorld";
    assert_eq!(deduper.strip_protocol_markers(text), "Hello\nWorld");
}

#[test]
fn test_deduper_strips_system_noise() {
    let deduper = ConversationDeduper::new();
    let text = "Hello\n<system-reminder>ignore me</system-reminder>\nWorld";
    assert_eq!(deduper.strip_system_noise(text), "Hello\n\nWorld");
}

#[test]
fn test_deduper_clean_content() {
    let deduper = ConversationDeduper::new();
    let text = "  Hello  \nCCB_CALLER=claude\n\n\n<env>x</env>\nWorld  ";
    assert_eq!(deduper.clean_content(text), "Hello  \n\nWorld");
}

#[test]
fn test_deduper_dedupe_messages() {
    let deduper = ConversationDeduper::new();
    let entries = vec![
        ConversationEntry {
            role: "user".to_string(),
            content: "hello".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "user".to_string(),
            content: "hello".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "assistant".to_string(),
            content: "hi".to_string(),
            ..Default::default()
        },
    ];
    let result = deduper.dedupe_messages(&entries);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_deduper_collapse_tool_calls() {
    let deduper = ConversationDeduper::new();
    let entries = vec![ConversationEntry {
        role: "assistant".to_string(),
        content: "I will read files".to_string(),
        tool_calls: vec![serde_json::json!({
            "name": "Read",
            "input": {"file_path": "/tmp/foo.txt"}
        })],
        ..Default::default()
    }];
    let result = deduper.collapse_tool_calls(&entries);
    assert!(result[0].content.contains("[Tools:"));
    assert!(result[0].content.contains("foo.txt"));
    assert!(result[0].tool_calls.is_empty());
}

// ---------------------------------------------------------------------------
// Formatter tests
// ---------------------------------------------------------------------------

fn sample_transfer_context() -> TransferContext {
    TransferContext {
        conversations: vec![
            ("What is 2+2?".to_string(), "2+2 equals 4.".to_string()),
            ("Write a file.".to_string(), "I wrote the file.".to_string()),
        ],
        source_session_id: "sess-123".to_string(),
        token_estimate: 100,
        metadata: serde_json::json!({"provider": "claude"}),
        stats: None,
        source_provider: "claude".to_string(),
    }
}

#[test]
fn test_formatter_markdown_contains_sections() {
    let formatter = ContextFormatter::new(8000);
    let ctx = sample_transfer_context();
    let out = formatter.format_markdown(&ctx, false);
    assert!(out.contains("## Context Transfer from Claude Session"));
    assert!(out.contains("**Source Session**: sess-123"));
    assert!(out.contains("#### Turn 1"));
    assert!(out.contains("**User**: What is 2+2?"));
}

#[test]
fn test_formatter_plain_contains_turns() {
    let formatter = ContextFormatter::new(8000);
    let ctx = sample_transfer_context();
    let out = formatter.format_plain(&ctx);
    assert!(out.contains("=== Context Transfer from Claude ==="));
    assert!(out.contains("--- Turn 1 ---"));
}

#[test]
fn test_formatter_json_roundtrip() {
    let formatter = ContextFormatter::new(8000);
    let ctx = sample_transfer_context();
    let out = formatter.format_json(&ctx);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid json");
    assert_eq!(parsed["source_provider"], "claude");
    assert_eq!(parsed["source_session_id"], "sess-123");
    assert!(parsed["conversations"].as_array().unwrap().len() == 2);
}

#[test]
fn test_formatter_truncate_to_limit() {
    let formatter = ContextFormatter::new(10);
    let conversations = vec![
        ("a".repeat(100), "b".repeat(100)),
        ("c".repeat(100), "d".repeat(100)),
    ];
    let result = formatter.truncate_to_limit(&conversations, None);
    assert!(result.len() <= 1);
}

#[test]
fn test_formatter_stats_section() {
    use ccb_memory::SessionStats;
    let mut stats = SessionStats::default();
    stats.tool_calls.insert("Read".to_string(), 3);
    stats.tool_calls.insert("Write".to_string(), 1);
    stats.files_written.push("/tmp/a.txt".to_string());
    stats.tasks_created = 2;
    stats.tasks_completed = 1;

    let lines = ccb_memory::formatter::format_stats_section(&stats, false);
    let joined = lines.join("\n");
    assert!(joined.contains("Read: 3"));
    assert!(joined.contains("Write: 1"));
    assert!(joined.contains("`/tmp/a.txt`"));
    assert!(joined.contains("1/2 completed"));
}

// ---------------------------------------------------------------------------
// Session parser tests
// ---------------------------------------------------------------------------

fn write_temp_session(content: &str) -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    std::fs::write(&path, content).unwrap();
    // Prevent the temp dir from being deleted while the path is in use.
    std::mem::forget(dir);
    path
}

#[test]
fn test_parse_session_basic() {
    let parser = ccb_memory::ClaudeSessionParser::default();
    let path = write_temp_session(
        r#"{"type": "user", "message": {"content": "hello"}, "uuid": "u1"}
{"type": "assistant", "message": {"content": "hi"}, "uuid": "a1"}"#,
    );
    let entries = parser.parse_session(&path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].role, "user");
    assert_eq!(entries[1].role, "assistant");
}

#[test]
fn test_parse_session_content_blocks() {
    let parser = ccb_memory::ClaudeSessionParser::default();
    let path = write_temp_session(
        r#"{"type": "assistant", "message": {"content": [{"type": "text", "text": "line1"}, {"type": "text", "text": "line2"}]}}"#,
    );
    let entries = parser.parse_session(&path).unwrap();
    assert_eq!(entries[0].content, "line1\nline2");
}

#[test]
fn test_extract_session_stats() {
    let parser = ccb_memory::ClaudeSessionParser::default();
    let path = write_temp_session(
        r#"{"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Write", "input": {"file_path": "/tmp/x.txt"}}]}}
{"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "ok"}]}}"#,
    );
    let stats = parser.extract_session_stats(&path).unwrap();
    assert_eq!(stats.tool_calls.get("Write"), Some(&1));
    assert_eq!(stats.files_written, vec!["/tmp/x.txt"]);
    assert_eq!(stats.tool_executions.len(), 1);
}

#[test]
fn test_session_not_found() {
    let parser = ccb_memory::ClaudeSessionParser::default();
    let result = parser.parse_session(PathBuf::from("/nonexistent/path.jsonl").as_path());
    assert!(matches!(
        result,
        Err(ccb_memory::MemoryError::SessionNotFound(_))
    ));
}

// ---------------------------------------------------------------------------
// Transfer tests
// ---------------------------------------------------------------------------

#[test]
fn test_transfer_build_pairs() {
    let entries = vec![
        ConversationEntry {
            role: "user".to_string(),
            content: "u1".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "assistant".to_string(),
            content: "a1".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "user".to_string(),
            content: "u2".to_string(),
            ..Default::default()
        },
    ];
    let pairs = ccb_memory::transfer::build_pairs(&entries);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("u1".to_string(), "a1".to_string()));
}

#[test]
fn test_transfer_save_and_format() {
    let tmp = tempfile::tempdir().unwrap();
    let transfer = ContextTransfer::new(8000, tmp.path());
    let ctx = sample_transfer_context();
    let path = transfer
        .save_transfer(&ctx, "markdown", None, None)
        .unwrap();
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Context Transfer from Claude Session"));
}

#[test]
fn test_transfer_extract_from_claude_session_file() {
    let session_path = write_temp_session(
        r#"{"type": "user", "message": {"content": "hello"}}
{"type": "assistant", "message": {"content": "hi there"}}"#,
    );
    let tmp = tempfile::tempdir().unwrap();
    let transfer = ContextTransfer::new(8000, tmp.path());
    let ctx = transfer
        .extract_conversations(Some(&session_path), 3, true, "claude", None, None)
        .unwrap();
    assert_eq!(ctx.conversations.len(), 1);
    assert_eq!(ctx.source_provider, "claude");
    assert!(ctx.token_estimate > 0);
}

// ---------------------------------------------------------------------------
// Project memory tests
// ---------------------------------------------------------------------------

#[test]
fn test_sha256_text() {
    let a = ccb_memory::sha256_text("hello");
    let b = ccb_memory::sha256_text("hello");
    let c = ccb_memory::sha256_text("world");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.len(), 64);
}

#[test]
fn test_memory_policy_for_provider() {
    let _claude = ccb_memory::memory_policy_for_provider("claude");
    assert!(!ccb_memory::should_include_source(
        "claude",
        "provider_native_project"
    ));
    assert!(ccb_memory::should_include_source(
        "gemini",
        "provider_native_project"
    ));
    let filters = ccb_memory::filters_for_source("claude", "provider_user_memory");
    assert!(filters.contains(&"ccb_install_blocks".to_string()));
}

#[test]
fn test_filter_install_blocks() {
    let source = ProjectMemorySource::new(
        "provider_user_memory",
        "Memory",
        PathBuf::from("/tmp/mem.md"),
        "start\n<!-- CCB_CONFIG_START -->\ninstall\n<!-- CCB_CONFIG_END -->\nend",
        true,
    );
    let filtered = ccb_memory::filter_memory_source(&source, &["ccb_install_blocks".to_string()]);
    assert!(filtered.filtered);
    assert!(filtered.content.contains("start"));
    assert!(!filtered.content.contains("CCB_CONFIG"));
}

#[test]
fn test_render_memory_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let source = ProjectMemorySource::new(
        "ccb_shared",
        "CCB Shared Project Memory",
        tmp.path().join("ccb_memory.md"),
        "Shared content",
        true,
    );
    let rendered =
        ccb_memory::render_memory_bundle(tmp.path(), "agent1", "claude", &[source], None);
    assert!(rendered.contains("# CCB Managed Agent Memory"));
    assert!(rendered.contains("## CCB Shared Project Memory"));
    assert!(rendered.contains("Shared content"));
}

#[test]
fn test_materialize_runtime_memory_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".ccb")).unwrap();
    let result = materialize_runtime_memory_bundle(
        tmp.path(),
        "Agent1",
        "claude",
        None,
        Some("2024-01-01T00:00:00Z"),
    )
    .unwrap();
    assert!(result.path.exists());
    assert!(result.written);
    assert!(!result.unchanged);

    // Second materialization should be unchanged.
    let result2 = materialize_runtime_memory_bundle(
        tmp.path(),
        "Agent1",
        "claude",
        None,
        Some("2024-01-01T00:00:00Z"),
    )
    .unwrap();
    assert!(result2.unchanged);
    assert!(!result2.written);
}

#[test]
fn test_agent_private_memory_path() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".ccb")).unwrap();
    let layout = ccb_storage::paths::PathLayout::new(tmp.path().to_string_lossy().to_string());
    let path = agent_private_memory_path(&layout, "Agent1");
    assert!(path.to_string_lossy().contains("agent1"));
    assert!(path.to_string_lossy().contains("memory.md"));
}

fn write_workspace_binding(workspace: &std::path::Path, target_project: &std::path::Path) {
    std::fs::write(
        workspace.join(".ccb-workspace.json"),
        serde_json::json!({
            "schema_version": 2,
            "record_type": "workspace_binding",
            "target_project": target_project,
            "project_id": "demo-project",
            "agent_name": "agent4",
            "workspace_mode": "linked",
            "workspace_path": workspace,
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn test_load_session_data_uses_workspace_binding_named_agent() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let workspace = tmp.path().join("workspace-agent4");
    std::fs::create_dir(&workspace).unwrap();
    write_workspace_binding(&workspace, &project_root);

    let session_file = project_root.join(".ccb").join(".codex-agent4-session");
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    std::fs::write(&session_file, r#"{"codex_session_id":"sid-1"}"#).unwrap();

    let (resolved, data) = ccb_memory::transfer::load_session_data(&workspace, "codex");
    assert_eq!(resolved, Some(session_file));
    assert_eq!(
        data.get("codex_session_id").unwrap().as_str().unwrap(),
        "sid-1"
    );
}

#[test]
fn test_auto_source_candidates_prefers_bound_agent_session() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("project");
    std::fs::create_dir(&project_root).unwrap();
    let workspace = tmp.path().join("workspace-agent4");
    std::fs::create_dir(&workspace).unwrap();
    write_workspace_binding(&workspace, &project_root);

    let codex_session = project_root.join(".ccb").join(".codex-agent4-session");
    let gemini_session = project_root.join(".ccb").join(".gemini-agent4-session");
    std::fs::create_dir_all(codex_session.parent().unwrap()).unwrap();
    std::fs::write(&codex_session, "{}").unwrap();
    std::fs::write(&gemini_session, "{}").unwrap();

    let now = std::time::SystemTime::now();
    let older = now - std::time::Duration::from_secs(10);
    filetime::set_file_mtime(&codex_session, filetime::FileTime::from_system_time(older)).unwrap();
    filetime::set_file_mtime(&gemini_session, filetime::FileTime::from_system_time(now)).unwrap();

    let mut source_session_files = HashMap::new();
    source_session_files.insert("codex".to_string(), ".codex-session".to_string());
    source_session_files.insert("gemini".to_string(), ".gemini-session".to_string());

    let ordered = auto_source_candidates(&workspace, &["codex", "gemini"], &source_session_files);
    assert_eq!(ordered[..2], ["gemini", "codex"]);
}
