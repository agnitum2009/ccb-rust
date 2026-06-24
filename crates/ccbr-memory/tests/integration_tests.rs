use ccbr_memory::{
    agent_private_memory_path, auto_source_candidates, auto_transfer::clear_seen,
    auto_transfer::maybe_auto_transfer_with, ensure_project_memory, load_memory_sources,
    materialize_runtime_memory_bundle, project_memory_path, runtime_memory_bundle_path,
    seed_metadata_path, ContextFormatter, ContextTransfer, ConversationDeduper, ConversationEntry,
    ProjectMemorySource, TransferContext,
};
use ccbr_provider_profiles::codex_home_config::materialize_codex_home_config;
use ccbr_provider_profiles::models::{ProviderProfileSpec, ResolvedProviderProfile};
use ccbr_providers::claude::launcher_runtime::home::materialize_claude_home_config;
use ccbr_providers::opencode::launcher::materialize_opencode_memory_config;
use ccbr_storage::paths::PathLayout;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

type StartedRecord = (
    String,
    PathBuf,
    Option<PathBuf>,
    Option<String>,
    Option<String>,
);

// ---------------------------------------------------------------------------
// Deduper tests
// ---------------------------------------------------------------------------

#[test]
fn test_deduper_strips_protocol_markers() {
    let deduper = ConversationDeduper::new();
    let text = "Hello\nCCBR_REQ_ID: 20240101-120000-000-1-1\nWorld";
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
    let text = "  Hello  \nCCBR_CALLER=claude\n\n\n<env>x</env>\nWorld  ";
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

#[test]
fn test_deduper_strips_various_protocol_markers() {
    let deduper = ConversationDeduper::new();
    let text = "Hello\nCCBR_REQ_ID: 20260202-123456-001-1-1\nCCBR_BEGIN: 20260202-123456-001-1-1\nCCBR_DONE: 20260202-123456-001-1-1\n[CCBR_ASYNC_SUBMITTED provider=codex]\nWorld";
    let result = deduper.strip_protocol_markers(text);
    assert!(!result.contains("CCBR_REQ_ID"));
    assert!(!result.contains("CCBR_BEGIN"));
    assert!(!result.contains("CCBR_DONE"));
    assert!(!result.contains("CCBR_ASYNC_SUBMITTED"));
    assert!(result.contains("Hello"));
    assert!(result.contains("World"));
}

#[test]
fn test_deduper_collapse_multiple_tool_kinds() {
    let deduper = ConversationDeduper::new();
    let entries = vec![ConversationEntry {
        role: "assistant".to_string(),
        content: "".to_string(),
        tool_calls: vec![
            serde_json::json!({"name": "Read", "input": {"file_path": "/a.py"}}),
            serde_json::json!({"name": "Read", "input": {"file_path": "/b.py"}}),
            serde_json::json!({"name": "Bash", "input": {"command": "ls"}}),
        ],
        ..Default::default()
    }];
    let result = deduper.collapse_tool_calls(&entries);
    assert!(result[0].content.contains("Read 2 file(s)"));
    assert!(result[0].content.contains("Bash 1 command(s)"));
}

#[test]
fn test_deduper_dedupe_messages_keeps_different() {
    let deduper = ConversationDeduper::new();
    let entries = vec![
        ConversationEntry {
            role: "user".to_string(),
            content: "Hello".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "assistant".to_string(),
            content: "Hi".to_string(),
            ..Default::default()
        },
        ConversationEntry {
            role: "user".to_string(),
            content: "How are you?".to_string(),
            ..Default::default()
        },
    ];
    let result = deduper.dedupe_messages(&entries);
    assert_eq!(result.len(), 3);
}

#[test]
fn test_deduper_dedupe_messages_empty() {
    let deduper = ConversationDeduper::new();
    let result = deduper.dedupe_messages(&[]);
    assert!(result.is_empty());
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
fn test_formatter_estimate_tokens() {
    let formatter = ContextFormatter::new(1000);
    let text = "a".repeat(400);
    assert_eq!(formatter.estimate_tokens(&text), 100);
}

#[test]
fn test_formatter_truncate_keeps_newest_pairs() {
    let formatter = ContextFormatter::new(500);
    let conversations = vec![
        ("a".repeat(400), "b".repeat(400)),
        ("c".repeat(400), "d".repeat(400)),
        ("e".repeat(400), "f".repeat(400)),
    ];
    let result = formatter.truncate_to_limit(&conversations, Some(500));
    assert_eq!(result.len(), 2);
    assert_eq!(result.last().unwrap().0, "e".repeat(400));
}

#[test]
fn test_formatter_stats_section() {
    use ccbr_memory::SessionStats;
    let mut stats = SessionStats::default();
    stats.tool_calls.insert("Read".to_string(), 3);
    stats.tool_calls.insert("Write".to_string(), 1);
    stats.files_written.push("/tmp/a.txt".to_string());
    stats.tasks_created = 2;
    stats.tasks_completed = 1;

    let lines = ccbr_memory::formatter::format_stats_section(&stats, false);
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
    let parser = ccbr_memory::ClaudeSessionParser::default();
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
    let parser = ccbr_memory::ClaudeSessionParser::default();
    let path = write_temp_session(
        r#"{"type": "assistant", "message": {"content": [{"type": "text", "text": "line1"}, {"type": "text", "text": "line2"}]}}"#,
    );
    let entries = parser.parse_session(&path).unwrap();
    assert_eq!(entries[0].content, "line1\nline2");
}

#[test]
fn test_extract_session_stats() {
    let parser = ccbr_memory::ClaudeSessionParser::default();
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
    let parser = ccbr_memory::ClaudeSessionParser::default();
    let result = parser.parse_session(PathBuf::from("/nonexistent/path.jsonl").as_path());
    assert!(matches!(
        result,
        Err(ccbr_memory::MemoryError::SessionNotFound(_))
    ));
}

#[test]
fn test_parse_session_tolerates_corrupted_lines() {
    let parser = ccbr_memory::ClaudeSessionParser::default();
    let path = write_temp_session(
        r#"{"type": "user", "message": {"content": "Hello"}}
invalid json
{"type": "assistant", "message": {"content": "Hi"}}"#,
    );
    let entries = parser.parse_session(&path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].role, "user");
    assert_eq!(entries[1].role, "assistant");
}

#[test]
fn test_get_session_info_uses_file_stem() {
    let parser = ccbr_memory::ClaudeSessionParser::default();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("my-session.jsonl");
    std::fs::write(&path, "{}\n").unwrap();
    let info = parser.get_session_info(&path).unwrap();
    assert_eq!(info.session_id, "my-session");
    assert_eq!(info.session_path, path.to_string_lossy().to_string());
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
    let pairs = ccbr_memory::transfer::build_pairs(&entries);
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
    let a = ccbr_memory::sha256_text("hello");
    let b = ccbr_memory::sha256_text("hello");
    let c = ccbr_memory::sha256_text("world");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.len(), 64);
}

#[test]
fn test_memory_policy_for_provider() {
    let _claude = ccbr_memory::memory_policy_for_provider("claude");
    assert!(!ccbr_memory::should_include_source(
        "claude",
        "provider_native_project"
    ));
    assert!(ccbr_memory::should_include_source(
        "gemini",
        "provider_native_project"
    ));
    let filters = ccbr_memory::filters_for_source("claude", "provider_user_memory");
    assert!(filters.contains(&"ccbr_install_blocks".to_string()));
}

#[test]
fn test_filter_install_blocks() {
    let source = ProjectMemorySource::new(
        "provider_user_memory",
        "Memory",
        PathBuf::from("/tmp/mem.md"),
        "start\n<!-- CCBR_CONFIG_START -->\ninstall\n<!-- CCBR_CONFIG_END -->\nend",
        true,
    );
    let filtered = ccbr_memory::filter_memory_source(&source, &["ccbr_install_blocks".to_string()]);
    assert!(filtered.filtered);
    assert!(filtered.content.contains("start"));
    assert!(!filtered.content.contains("CCBR_CONFIG"));
}

#[test]
fn test_render_memory_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let source = ProjectMemorySource::new(
        "ccbr_shared",
        "CCBR Shared Project Memory",
        tmp.path().join("ccbr_memory.md"),
        "Shared content",
        true,
    );
    let rendered =
        ccbr_memory::render_memory_bundle(tmp.path(), "agent1", "claude", &[source], None);
    assert!(rendered.contains("# CCBR Managed Agent Memory"));
    assert!(rendered.contains("## CCBR Shared Project Memory"));
    assert!(rendered.contains("Shared content"));
}

#[test]
fn test_materialize_runtime_memory_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".ccbr")).unwrap();
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
    std::fs::create_dir_all(tmp.path().join(".ccbr")).unwrap();
    let layout = ccbr_storage::paths::PathLayout::new(tmp.path().to_string_lossy().to_string());
    let path = agent_private_memory_path(&layout, "Agent1");
    assert!(path.to_string_lossy().contains("agent1"));
    assert!(path.to_string_lossy().contains("memory.md"));
}

fn write_workspace_binding(workspace: &std::path::Path, target_project: &std::path::Path) {
    std::fs::write(
        workspace.join(".ccbr-workspace.json"),
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

    let session_file = project_root.join(".ccbr").join(".codex-agent4-session");
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    std::fs::write(&session_file, r#"{"codex_session_id":"sid-1"}"#).unwrap();

    let (resolved, data) = ccbr_memory::transfer::load_session_data(&workspace, "codex");
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

    let codex_session = project_root.join(".ccbr").join(".codex-agent4-session");
    let gemini_session = project_root.join(".ccbr").join(".gemini-agent4-session");
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

// ---------------------------------------------------------------------------
// Project memory filter tests (parity with test_project_memory_filters.py)
// ---------------------------------------------------------------------------

fn filter_source(text: &str, kind: &str) -> ProjectMemorySource {
    let source = ProjectMemorySource::new(
        kind,
        "Provider User Memory",
        PathBuf::from("/tmp/AGENTS.md"),
        text,
        true,
    );
    ccbr_memory::filter_memory_source(&source, &["ccbr_install_blocks".to_string()])
}

#[test]
fn filter_strips_complete_ccbr_config_block() {
    let result = filter_source(
        "before\n<!-- CCBR_CONFIG_START -->\nconfig\n<!-- CCBR_CONFIG_END -->\nafter\n",
        "provider_user_memory",
    );
    assert_eq!(result.content, "before\nafter\n");
    assert!(result.filtered);
    assert_eq!(result.filter_names, vec!["ccbr_install_blocks"]);
}

#[test]
fn filter_strips_complete_roles_and_rubrics_blocks() {
    let result = filter_source(
        "keep\n<!-- CCBR_ROLES_START -->roles<!-- CCBR_ROLES_END -->\n<!-- REVIEW_RUBRICS_START -->rubric<!-- REVIEW_RUBRICS_END -->\ntail\n",
        "provider_user_memory",
    );
    assert_eq!(result.content, "keep\ntail\n");
}

#[test]
fn filter_strips_review_and_gemini_inspiration_blocks() {
    let result = filter_source(
        "keep\n<!-- CODEX_REVIEW_START -->review<!-- CODEX_REVIEW_END -->\n<!-- GEMINI_INSPIRATION_START -->idea<!-- GEMINI_INSPIRATION_END -->\ntail\n",
        "provider_user_memory",
    );
    assert_eq!(result.content, "keep\ntail\n");
}

#[test]
fn filter_strips_legacy_collaboration_sections() {
    let result = filter_source(
        "intro\n## Codex Collaboration Rules\nold codex\n## Gemini Collaboration Rules\nold gemini\n## OpenCode Collaboration Rules\nold opencode\n",
        "provider_user_memory",
    );
    assert!(!result.content.contains("Collaboration Rules"));
    assert_eq!(result.content, "intro\n");
}

#[test]
fn filter_strips_legacy_chinese_collaboration_sections() {
    let result = filter_source(
        "intro\n## Codex 协作规则\nold codex\n## Gemini 协作规则\nold gemini\n## OpenCode 协作规则\nold opencode\n",
        "provider_user_memory",
    );
    assert!(!result.content.contains("协作规则"));
    assert_eq!(result.content, "intro\n");
}

#[test]
fn filter_preserves_user_paragraph_spacing_after_block_removal() {
    let result = filter_source(
        "first paragraph\n\nsecond paragraph\n<!-- CCBR_CONFIG_START -->\nold config\n<!-- CCBR_CONFIG_END -->\nthird paragraph\n",
        "provider_user_memory",
    );
    assert_eq!(
        result.content,
        "first paragraph\n\nsecond paragraph\nthird paragraph\n"
    );
}

#[test]
fn filter_preserves_isolated_marker() {
    let text = "before\n<!-- CCBR_CONFIG_START -->\nuser note without end marker\n";
    let result = filter_source(text, "provider_user_memory");
    assert_eq!(result.content, text);
    assert!(!result.filtered);
}

#[test]
fn filter_preserves_unrelated_user_text() {
    let text = "Use ask carefully, but this is user-authored and has no CCBR marker pair.\n";
    let result = filter_source(text, "provider_user_memory");
    assert_eq!(result.content, text);
    assert!(!result.filtered);
}

#[test]
fn filter_only_applies_to_provider_user_memory() {
    let text = "before\n<!-- CCBR_CONFIG_START -->\nconfig\n<!-- CCBR_CONFIG_END -->\nafter\n";
    let result = filter_source(text, "ccbr_shared");
    assert_eq!(result.content, text);
    assert!(!result.filtered);
}

#[test]
fn memory_provider_policy_excludes_native_project_for_duplicate_loading_providers() {
    assert!(!ccbr_memory::should_include_source(
        "claude",
        "provider_native_project"
    ));
    assert!(!ccbr_memory::should_include_source(
        "codex",
        "provider_native_project"
    ));
    assert!(!ccbr_memory::should_include_source(
        "opencode",
        "provider_native_project"
    ));
    assert!(ccbr_memory::should_include_source(
        "gemini",
        "provider_native_project"
    ));
}

// ---------------------------------------------------------------------------
// Project memory ensure/load/materialize tests (parity with test_project_memory.py)
// ---------------------------------------------------------------------------

fn legacy_v4_project_memory_template() -> &'static str {
    r#"# CCB Project Memory

This project uses CCB for visible multi-agent collaboration.

## Collaboration

- You are one agent in a CCB-managed project team.
- Use CCB `ask` for project-level collaboration with configured agents.
- Delegate with the goal, scope/files, assumptions, expected output, and verification needs.
- Reply concisely with findings, changes, verification, blockers, and risks when relevant.

## Ask Communication

Preferred form:

```text
/ask <agent> <message>
```

Shell fallback:

```bash
command ask "$TARGET" <<'EOF'
$MESSAGE
EOF
```

- Submit once, then stop. Do not wait, poll, or run `pend`/`watch`/`ping` unless diagnostics were requested.
- During an active CCB ask task, use `ask --callback` when a child result is needed; use `ask --silence` only for independent no-result-needed work.
- Plain nested `ask` from an active task is rejected by CCB.
"#
}

#[test]
fn ensure_project_memory_creates_template_and_seed() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());

    let result = ensure_project_memory(&layout, Some("2026-05-11T00:00:00+00:00")).unwrap();

    assert!(result.created);
    assert!(result.seed_written);
    assert!(result.warning.is_empty());
    let memory_path = project_memory_path(&layout);
    assert!(memory_path.is_file());
    let text = std::fs::read_to_string(&memory_path).unwrap();
    assert!(text.contains("This project uses CCBR for visible multi-agent collaboration."));
    assert!(text.contains("Use CCBR `ask` for project-level collaboration with configured agents."));
    assert!(!text.contains("Plain nested `ask` from an active task is"));
    assert!(!text.contains("command ask \"$TARGET\""));
    assert!(!text.contains("Do not wait, poll, or run `pend`/`watch`/`ping`"));
    assert!(!text.contains("ccbr -h"));

    let seed = std::fs::read_to_string(seed_metadata_path(&layout)).unwrap();
    let seed: serde_json::Value = serde_json::from_str(&seed).unwrap();
    assert_eq!(seed["record_type"], "ccbr_project_memory_seed");
    assert_eq!(seed["template_version"], 5);
    assert_eq!(
        seed["memory_path"],
        memory_path.to_string_lossy().to_string()
    );
    assert_eq!(seed["sha256"], result.sha256);
}

#[test]
fn ensure_project_memory_does_not_overwrite_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, "# Team Memory\n\nKeep this custom text.\n").unwrap();

    let result = ensure_project_memory(&layout, None).unwrap();

    assert!(!result.created);
    assert!(!result.seed_written);
    assert_eq!(
        std::fs::read_to_string(&memory_path).unwrap(),
        "# Team Memory\n\nKeep this custom text.\n"
    );
    assert!(!seed_metadata_path(&layout).exists());
}

#[test]
fn ensure_project_memory_ignores_legacy_root_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let legacy_path = project_root.join("CCB.md");
    std::fs::write(&legacy_path, "legacy shared memory\n").unwrap();

    let result = ensure_project_memory(&layout, None).unwrap();
    let memory_path = project_memory_path(&layout);

    assert!(result.created);
    assert!(result.seed_written);
    let text = std::fs::read_to_string(&memory_path).unwrap();
    assert!(text.contains("This project uses CCBR for visible multi-agent collaboration."));
    assert!(!text.contains("legacy shared memory"));
    assert_eq!(
        std::fs::read_to_string(&legacy_path).unwrap(),
        "legacy shared memory\n"
    );
}

#[test]
fn ensure_project_memory_backfills_missing_seed_for_unedited_template() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());

    let first = ensure_project_memory(&layout, Some("2026-05-11T00:00:00+00:00")).unwrap();
    std::fs::remove_file(seed_metadata_path(&layout)).unwrap();

    let second = ensure_project_memory(&layout, Some("2026-05-11T00:01:00+00:00")).unwrap();

    assert!(first.created);
    assert!(!second.created);
    assert!(second.seed_written);
    let seed = std::fs::read_to_string(seed_metadata_path(&layout)).unwrap();
    let seed: serde_json::Value = serde_json::from_str(&seed).unwrap();
    assert_eq!(seed["sha256"], second.sha256);
}

#[test]
fn ensure_project_memory_upgrades_unedited_seeded_old_template() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    let old_template = legacy_v4_project_memory_template();
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, old_template).unwrap();
    let seed_path = seed_metadata_path(&layout);
    std::fs::create_dir_all(seed_path.parent().unwrap()).unwrap();
    std::fs::write(
        &seed_path,
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccbr_project_memory_seed",
            "template_version": 4,
            "memory_path": memory_path.to_string_lossy().to_string(),
            "sha256": ccbr_memory::sha256_text(old_template),
            "created_at": "2026-06-01T00:00:00+00:00",
        })
        .to_string(),
    )
    .unwrap();

    let result = ensure_project_memory(&layout, Some("2026-06-07T00:00:00+00:00")).unwrap();

    assert!(!result.created);
    assert!(result.seed_written);
    assert!(result.warning.is_empty());
    let text = std::fs::read_to_string(&memory_path).unwrap();
    assert!(text.contains("This project uses CCBR for visible multi-agent collaboration."));
    assert!(!text.contains("command ask \"$TARGET\""));
    let seed = std::fs::read_to_string(&seed_path).unwrap();
    let seed: serde_json::Value = serde_json::from_str(&seed).unwrap();
    assert_eq!(seed["template_version"], 5);
    assert_eq!(seed["sha256"], result.sha256);
}

#[test]
fn ensure_project_memory_upgrades_unedited_legacy_template_without_seed() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    let old_template = legacy_v4_project_memory_template();
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, old_template).unwrap();

    let result = ensure_project_memory(&layout, Some("2026-06-07T00:00:00+00:00")).unwrap();

    assert!(!result.created);
    assert!(result.seed_written);
    let text = std::fs::read_to_string(&memory_path).unwrap();
    assert!(text.contains("This project uses CCBR for visible multi-agent collaboration."));
    let seed = std::fs::read_to_string(seed_metadata_path(&layout)).unwrap();
    let seed: serde_json::Value = serde_json::from_str(&seed).unwrap();
    assert_eq!(seed["template_version"], 5);
    assert_eq!(seed["sha256"], result.sha256);
}

#[test]
fn ensure_project_memory_does_not_upgrade_edited_old_seed() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    let seeded_text = "# CCB Project Memory\n\n## Ask Communication\nseeded\n";
    let edited_text = format!("{}\nUser edit.\n", seeded_text);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, &edited_text).unwrap();
    let seed_path = seed_metadata_path(&layout);
    std::fs::create_dir_all(seed_path.parent().unwrap()).unwrap();
    std::fs::write(
        &seed_path,
        serde_json::json!({
            "schema_version": 1,
            "record_type": "ccbr_project_memory_seed",
            "template_version": 4,
            "memory_path": memory_path.to_string_lossy().to_string(),
            "sha256": ccbr_memory::sha256_text(seeded_text),
            "created_at": "2026-06-01T00:00:00+00:00",
        })
        .to_string(),
    )
    .unwrap();

    let result = ensure_project_memory(&layout, None).unwrap();

    assert!(!result.created);
    assert!(!result.seed_written);
    assert_eq!(std::fs::read_to_string(&memory_path).unwrap(), edited_text);
}

#[test]
fn load_memory_sources_reads_from_project_root_not_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    let workspace = tmp.path().join("worktree");
    std::fs::create_dir(&project_root).unwrap();
    std::fs::create_dir(&workspace).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, "shared memory\n").unwrap();
    std::fs::write(project_root.join("GEMINI.md"), "project gemini memory\n").unwrap();
    std::fs::write(workspace.join("GEMINI.md"), "workspace-only memory\n").unwrap();
    let agent_private = agent_private_memory_path(&layout, "Agent3");
    std::fs::create_dir_all(agent_private.parent().unwrap()).unwrap();
    std::fs::write(&agent_private, "private memory\n").unwrap();

    let sources = load_memory_sources(&layout, "Agent3", "gemini", &[], true, None);

    let content_by_kind: HashMap<String, String> = sources
        .iter()
        .map(|s| (s.kind.clone(), s.content.clone()))
        .collect();
    assert_eq!(content_by_kind["ccbr_shared"], "shared memory\n");
    assert_eq!(
        content_by_kind["provider_native_project"],
        "project gemini memory\n"
    );
    assert_eq!(content_by_kind["agent_private"], "private memory\n");
    assert!(!content_by_kind
        .values()
        .any(|c| c.contains("workspace-only memory")));
}

#[test]
fn load_memory_sources_can_skip_provider_native_project_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, "shared memory\n").unwrap();
    std::fs::write(project_root.join("GEMINI.md"), "project gemini memory\n").unwrap();
    let agent_private = agent_private_memory_path(&layout, "Agent1");
    std::fs::create_dir_all(agent_private.parent().unwrap()).unwrap();
    std::fs::write(&agent_private, "private memory\n").unwrap();

    let default_sources = load_memory_sources(&layout, "Agent1", "gemini", &[], true, None);
    let skipped_sources = load_memory_sources(&layout, "Agent1", "gemini", &[], true, Some(false));

    assert_eq!(
        default_sources
            .iter()
            .map(|s| s.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["ccbr_shared", "provider_native_project", "agent_private"]
    );
    assert_eq!(
        skipped_sources
            .iter()
            .map(|s| s.kind.as_str())
            .collect::<Vec<_>>(),
        vec!["ccbr_shared", "agent_private"]
    );
    assert!(!skipped_sources
        .iter()
        .any(|s| s.content.contains("project gemini memory")));
}

#[test]
fn materialize_runtime_memory_bundle_writes_generated_bundle_with_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    let workspace = tmp.path().join("worktree");
    std::fs::create_dir(&project_root).unwrap();
    std::fs::create_dir(&workspace).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, "shared ask rules\n").unwrap();
    std::fs::write(project_root.join("CLAUDE.md"), "claude project rules\n").unwrap();
    let agent_private = agent_private_memory_path(&layout, "agent1");
    std::fs::create_dir_all(agent_private.parent().unwrap()).unwrap();
    std::fs::write(&agent_private, "agent private rules\n").unwrap();

    let result = materialize_runtime_memory_bundle(
        &project_root,
        "agent1",
        "claude",
        Some(&workspace),
        None,
    )
    .unwrap();

    assert!(result.written);
    assert!(result.warnings.is_empty());
    let bundle_path = runtime_memory_bundle_path(&layout, "agent1");
    assert_eq!(result.path, bundle_path);
    let text = std::fs::read_to_string(&bundle_path).unwrap();
    assert!(text.contains("# CCBR Managed Agent Memory"));
    assert!(text.contains("<!-- ccbr-memory-bundle schema_version=1"));
    assert!(text.contains("provider: claude"));
    assert!(text.contains(&format!(
        "workspace_path: {}",
        workspace.canonicalize().unwrap().display()
    )));
    assert!(text.contains("## CCBR Runtime Coordination Rules"));
    assert!(text.contains("CCBR `ask` is submit-only"));
    assert!(text.contains("Do not wait, poll, or run `pend`/`watch`/`ping`"));
    assert!(text.contains("## CCBR Shared Project Memory"));
    assert!(text.contains("shared ask rules"));
    let coord_pos = text.find("## CCBR Runtime Coordination Rules").unwrap();
    let shared_pos = text.find("## CCBR Shared Project Memory").unwrap();
    assert!(coord_pos < shared_pos);
    assert!(!text.contains("## Provider-Native Project Memory"));
    assert!(!text.contains("claude project rules"));
    assert!(text.contains("## Agent Private Memory"));
    assert!(text.contains("agent private rules"));
    let kinds: std::collections::HashSet<String> =
        result.sources.iter().map(|s| s.kind.clone()).collect();
    assert_eq!(
        kinds,
        std::collections::HashSet::from(["ccbr_shared".to_string(), "agent_private".to_string()])
    );
}

#[test]
fn materialize_runtime_memory_bundle_skips_unchanged_write() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();
    let layout = PathLayout::new(project_root.to_string_lossy().to_string());
    let memory_path = project_memory_path(&layout);
    std::fs::create_dir_all(memory_path.parent().unwrap()).unwrap();
    std::fs::write(&memory_path, "shared ask rules\n").unwrap();

    let first =
        materialize_runtime_memory_bundle(&project_root, "agent1", "opencode", None, None).unwrap();
    let mtime = std::fs::metadata(runtime_memory_bundle_path(&layout, "agent1"))
        .unwrap()
        .modified()
        .unwrap();
    let second =
        materialize_runtime_memory_bundle(&project_root, "agent1", "opencode", None, None).unwrap();

    assert!(first.written);
    assert!(!first.unchanged);
    assert!(!second.written);
    assert!(second.unchanged);
    assert_eq!(
        std::fs::metadata(runtime_memory_bundle_path(&layout, "agent1"))
            .unwrap()
            .modified()
            .unwrap(),
        mtime
    );
}

#[test]
fn materialize_runtime_memory_bundle_handles_invalid_agent_name() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("repo");
    std::fs::create_dir(&project_root).unwrap();

    let result =
        materialize_runtime_memory_bundle(&project_root, "bad/name", "claude", None, None).unwrap();

    assert!(!result.written);
    assert!(result.sources.is_empty());
    assert!(!result.warnings.is_empty());
}

// ---------------------------------------------------------------------------
// Auto-transfer tests (parity with test_memory_auto_transfer.py)
// ---------------------------------------------------------------------------

#[test]
fn maybe_auto_transfer_starts_once_for_same_key() {
    clear_seen();
    std::env::set_var("CCBR_CTX_TRANSFER_ON_SESSION_SWITCH", "1");
    let tmp = tempfile::tempdir().unwrap();
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let session_path = tmp.path().join("session.json");
    let mut started: Vec<StartedRecord> = Vec::new();
    let mut start = |provider: &str,
                     work_dir: &Path,
                     sp: Option<&Path>,
                     sid: Option<&str>,
                     pid: Option<&str>| {
        started.push((
            provider.to_string(),
            work_dir.to_path_buf(),
            sp.map(|p| p.to_path_buf()),
            sid.map(|s| s.to_string()),
            pid.map(|s| s.to_string()),
        ));
    };

    maybe_auto_transfer_with(
        "codex",
        tmp.path(),
        Some(&session_path),
        Some("session-1"),
        Some("proj-1"),
        &mut start,
    );
    maybe_auto_transfer_with(
        "codex",
        tmp.path(),
        Some(&session_path),
        Some("session-1"),
        Some("proj-1"),
        &mut start,
    );

    assert_eq!(started.len(), 1);
    assert_eq!(started[0].0, "codex");
    assert_eq!(started[0].1, tmp.path());
    std::env::set_current_dir(original_cwd).unwrap();
}

#[test]
fn maybe_auto_transfer_skips_foreign_work_dir() {
    clear_seen();
    std::env::set_var("CCBR_CTX_TRANSFER_ON_SESSION_SWITCH", "1");
    let tmp = tempfile::tempdir().unwrap();
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    let other = tmp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let mut started: Vec<StartedRecord> = Vec::new();
    let mut start = |provider: &str,
                     work_dir: &Path,
                     sp: Option<&Path>,
                     sid: Option<&str>,
                     pid: Option<&str>| {
        started.push((
            provider.to_string(),
            work_dir.to_path_buf(),
            sp.map(|p| p.to_path_buf()),
            sid.map(|s| s.to_string()),
            pid.map(|s| s.to_string()),
        ));
    };

    maybe_auto_transfer_with(
        "codex",
        &other,
        Some(Path::new("/tmp/session.json")),
        Some("session-1"),
        Some("proj-1"),
        &mut start,
    );

    assert!(started.is_empty());
    std::env::set_current_dir(original_cwd).unwrap();
}

// ---------------------------------------------------------------------------
// Provider transfer runtime tests (parity with test_memory_transfer_providers.py)
// ---------------------------------------------------------------------------

#[test]
fn test_extract_from_codex_falls_back_to_latest_log_path() {
    let tmp = tempfile::tempdir().unwrap();
    let log_path = tmp.path().join("latest.jsonl");
    std::fs::write(
        &log_path,
        r#"{"role": "user", "content": " user "}
{"role": "assistant", "content": " assistant "}
"#,
    )
    .unwrap();

    // Point the bound session file at a missing path so the fallback scanner runs.
    let session_file = tmp.path().join(".codex-session");
    std::fs::write(
        &session_file,
        r#"{"codex_session_path": "/nonexistent/missing.jsonl"}"#,
    )
    .unwrap();

    let deduper = ConversationDeduper::new();
    let formatter = ContextFormatter::new(200);
    let ctx = ccbr_memory::transfer_runtime::providers_runtime::codex::extract_from_codex(
        tmp.path(),
        &ccbr_memory::transfer::source_session_files(),
        &deduper,
        &formatter,
        200,
        4,
        0,
    )
    .unwrap();

    assert_eq!(ctx.source_provider, "codex");
    assert_eq!(ctx.source_session_id, "latest");
    assert_eq!(
        ctx.metadata.get("session_path").and_then(|v| v.as_str()),
        Some(log_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        ctx.conversations,
        vec![("user".to_string(), "assistant".to_string())]
    );
}

#[test]
fn test_extract_from_opencode_uses_captured_session_state() {
    let tmp = tempfile::tempdir().unwrap();
    let ccbr_dir = tmp.path().join(".ccbr");
    std::fs::create_dir(&ccbr_dir).unwrap();
    let session_path = tmp.path().join("session.json");
    std::fs::write(
        &session_path,
        r#"{"messages": [
    {"role": "user", "content": "question"},
    {"role": "assistant", "content": "answer"}
]}"#,
    )
    .unwrap();

    let session_file = ccbr_dir.join(".opencode-session");
    std::fs::write(&session_file, r#"{"opencode_project_id": "proj-9"}"#).unwrap();

    let deduper = ConversationDeduper::new();
    let formatter = ContextFormatter::new(200);
    let ctx = ccbr_memory::transfer_runtime::providers_runtime::opencode::extract_from_opencode(
        tmp.path(),
        &ccbr_memory::transfer::source_session_files(),
        &deduper,
        &formatter,
        200,
        2,
        0,
    )
    .unwrap();

    assert_eq!(ctx.source_provider, "opencode");
    assert_eq!(ctx.source_session_id, "session");
    assert_eq!(
        ctx.metadata.get("session_path").and_then(|v| v.as_str()),
        Some(session_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        ctx.metadata.get("project_id").and_then(|v| v.as_str()),
        Some("proj-9")
    );
    assert_eq!(
        ctx.conversations,
        vec![("question".to_string(), "answer".to_string())]
    );
}

#[test]
fn test_extract_from_opencode_raises_when_no_session_identity_exists() {
    let tmp = tempfile::tempdir().unwrap();
    // No session files at all.
    let deduper = ConversationDeduper::new();
    let formatter = ContextFormatter::new(200);
    let result = ccbr_memory::transfer_runtime::providers_runtime::opencode::extract_from_opencode(
        tmp.path(),
        &ccbr_memory::transfer::source_session_files(),
        &deduper,
        &formatter,
        200,
        2,
        0,
    );
    assert!(matches!(
        result,
        Err(ccbr_memory::MemoryError::SessionNotFound(_))
    ));
}

// ---------------------------------------------------------------------------
// Real-context provider memory materialization tests
// (parity with test_project_memory_real_context.py)
// ---------------------------------------------------------------------------

fn write_test_file(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn assert_single_runtime_coordination(text: &str) {
    assert_eq!(
        text.matches("## CCBR Runtime Coordination Rules").count(),
        1
    );
    assert_eq!(text.matches("command ask \"$TARGET\"").count(), 1);
}

#[test]
fn test_realistic_provider_memory_context_composes_each_provider_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path().join("external-real-project");
    let workspace_path = tmp.path().join("external-real-project-worktree");
    let claude_source_home = tmp.path().join("source-claude-home");
    let codex_source_home = tmp.path().join("source-codex-home");
    std::fs::create_dir(&project_root).unwrap();
    std::fs::create_dir(&workspace_path).unwrap();

    write_test_file(
        &project_root.join(".ccbr").join("ccbr_memory.md"),
        "# CCBR Project Memory\n\nSHARED-MEMORY-SENTINEL\n",
    );
    write_test_file(&project_root.join("CLAUDE.md"), "PROJECT-CLAUDE-SENTINEL\n");
    write_test_file(&project_root.join("AGENTS.md"), "PROJECT-AGENTS-SENTINEL\n");
    write_test_file(&project_root.join("GEMINI.md"), "PROJECT-GEMINI-SENTINEL\n");
    write_test_file(
        &project_root.join("opencode.json"),
        serde_json::json!({"instructions": ["AGENTS.md"], "model": "test-model"})
            .to_string()
            .as_str(),
    );

    write_test_file(
        &project_root
            .join(".ccbr")
            .join("agents")
            .join("reviewer")
            .join("memory.md"),
        "CLAUDE-PRIVATE-SENTINEL\n",
    );
    write_test_file(
        &project_root
            .join(".ccbr")
            .join("agents")
            .join("builder")
            .join("memory.md"),
        "CODEX-PRIVATE-SENTINEL\n",
    );
    write_test_file(
        &project_root
            .join(".ccbr")
            .join("agents")
            .join("designer")
            .join("memory.md"),
        "OPENCODE-PRIVATE-SENTINEL\n",
    );
    write_test_file(
        &project_root
            .join(".ccbr")
            .join("agents")
            .join("analyst")
            .join("memory.md"),
        "GEMINI-PRIVATE-SENTINEL\n",
    );

    write_test_file(
        &claude_source_home.join(".claude").join("CLAUDE.md"),
        "CLAUDE-USER-SENTINEL\n<!-- CCBR_CONFIG_START -->\nOLD-CLAUDE-INSTALL-BLOCK\n<!-- CCBR_CONFIG_END -->\n",
    );
    write_test_file(
        &codex_source_home.join("AGENTS.md"),
        "CODEX-USER-SENTINEL\n<!-- CCBR_ROLES_START -->\nOLD-CODEX-ROLES-BLOCK\n<!-- CCBR_ROLES_END -->\n",
    );

    let claude_home = project_root
        .join(".ccbr")
        .join("agents")
        .join("reviewer")
        .join("provider-state")
        .join("claude")
        .join("home");
    let claude_layout = materialize_claude_home_config(
        camino::Utf8Path::from_path(&claude_home).unwrap(),
        None,
        Some(camino::Utf8Path::from_path(&claude_source_home).unwrap()),
        Some(camino::Utf8Path::from_path(&project_root).unwrap()),
        Some("reviewer"),
        Some(camino::Utf8Path::from_path(&workspace_path).unwrap()),
        false,
        None,
        None,
    )
    .unwrap();

    let codex_home = project_root
        .join(".ccbr")
        .join("agents")
        .join("builder")
        .join("provider-state")
        .join("codex")
        .join("home");
    materialize_codex_home_config(
        &codex_home,
        Some(&ProviderProfileSpec::default()),
        Some(camino::Utf8Path::from_path(&codex_source_home).unwrap()),
        Some(camino::Utf8Path::from_path(&project_root).unwrap()),
        Some("builder"),
        None,
        Some(camino::Utf8Path::from_path(&workspace_path).unwrap()),
        None,
        None,
        None,
    )
    .unwrap();

    let opencode_config_path = project_root
        .join(".ccbr")
        .join("agents")
        .join("designer")
        .join("provider-state")
        .join("opencode")
        .join("opencode.json");
    let opencode_profile = ResolvedProviderProfile::new("opencode", "designer");
    let opencode_result = materialize_opencode_memory_config(
        &project_root,
        "designer",
        Some(&workspace_path),
        Some(&opencode_config_path),
        Some(&opencode_profile),
        None,
        Some(
            &project_root
                .join(".ccbr")
                .join("agents")
                .join("designer")
                .join("memory-projection.json"),
        ),
    );

    let gemini_materialization = materialize_runtime_memory_bundle(
        &project_root,
        "analyst",
        "gemini",
        Some(&workspace_path),
        None,
    )
    .unwrap();

    let claude_text = std::fs::read_to_string(claude_layout.claude_dir.join("CLAUDE.md")).unwrap();
    let codex_text = std::fs::read_to_string(codex_home.join("AGENTS.md")).unwrap();
    let opencode_text = std::fs::read_to_string(
        project_root
            .join(".ccbr")
            .join("runtime")
            .join("memory")
            .join("designer.md"),
    )
    .unwrap();
    let gemini_text = std::fs::read_to_string(&gemini_materialization.path).unwrap();

    for text in [&claude_text, &codex_text, &opencode_text, &gemini_text] {
        assert!(text.contains("# CCBR Managed Agent Memory"));
        assert!(text.contains("SHARED-MEMORY-SENTINEL"));
        assert_single_runtime_coordination(text);
    }

    assert!(claude_text.contains("provider: claude"));
    assert!(claude_text.contains("CLAUDE-USER-SENTINEL"));
    assert!(!claude_text.contains("OLD-CLAUDE-INSTALL-BLOCK"));
    assert!(!claude_text.contains("PROJECT-CLAUDE-SENTINEL"));
    assert!(claude_text.contains("CLAUDE-PRIVATE-SENTINEL"));

    assert!(codex_text.contains("provider: codex"));
    assert!(codex_text.contains("CODEX-USER-SENTINEL"));
    assert!(!codex_text.contains("OLD-CODEX-ROLES-BLOCK"));
    assert!(!codex_text.contains("PROJECT-AGENTS-SENTINEL"));
    assert!(codex_text.contains("CODEX-PRIVATE-SENTINEL"));

    assert_eq!(
        opencode_result.env.get("OPENCODE_CONFIG"),
        Some(&opencode_config_path.to_string_lossy().to_string())
    );
    let opencode_config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&opencode_config_path).unwrap()).unwrap();
    assert_eq!(
        opencode_config["instructions"],
        serde_json::json!(["AGENTS.md", ".ccbr/runtime/memory/designer.md"])
    );
    assert!(opencode_text.contains("provider: opencode"));
    assert!(!opencode_text.contains("PROJECT-AGENTS-SENTINEL"));
    assert!(opencode_text.contains("OPENCODE-PRIVATE-SENTINEL"));

    assert!(gemini_text.contains("provider: gemini"));
    assert!(gemini_text.contains("PROJECT-GEMINI-SENTINEL"));
    assert!(gemini_text.contains("GEMINI-PRIVATE-SENTINEL"));
}
