use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ccb_completion::models::{
    CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::manifest::RuntimeMode;
use ccb_providers::claude::{
    find_project_session_file, get_session_registry, load_project_session, ClaudeSessionRegistry,
};
use ccb_providers::execution::{
    with_prompt_target_override, ExecutionAdapter, PromptTarget, ProviderRuntimeContext,
};
use ccb_providers::providers::claude::{
    backend, manifest, wrap_claude_prompt, wrap_claude_turn_prompt, ClaudeExecutionAdapter,
    PROVIDER_NAME,
};
use serde_json::Value;
use tempfile::TempDir;

fn write_json(path: &Path, content: Value) {
    std::fs::write(path, serde_json::to_string(&content).unwrap()).unwrap();
}

fn claude_session_filename(instance: Option<&str>) -> String {
    match instance {
        None | Some("") => ".claude-session".to_string(),
        Some(inst) => format!(".claude-{}-session", inst.trim()),
    }
}

fn write_session_file(work_dir: &Path, instance: Option<&str>, extra: Value) {
    let mut data = serde_json::Map::new();
    data.insert("pane_id".to_string(), Value::String("%1".to_string()));
    if let Value::Object(map) = extra {
        data.extend(map);
    }
    let filename = claude_session_filename(instance);
    write_json(&work_dir.join(filename), Value::Object(data));
}

fn projects_root(dir: &TempDir) -> PathBuf {
    dir.path().join(".claude").join("projects")
}

fn project_key_for_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(['\\', '/'], "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .trim_matches('-')
        .to_string()
}

fn make_project_dir(root: &Path, work_dir: &Path) -> PathBuf {
    root.join(project_key_for_path(work_dir))
}

#[derive(Default, Clone)]
struct MockTarget {
    sent: Arc<Mutex<Vec<(String, String)>>>,
    content: Arc<Mutex<String>>,
}

impl PromptTarget for MockTarget {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
        self.sent
            .lock()
            .unwrap()
            .push((pane_id.to_string(), text.to_string()));
        Ok(())
    }

    fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
        Ok(self.content.lock().unwrap().clone())
    }
}

impl MockTarget {
    fn with_content(self, content: &str) -> Self {
        *self.content.lock().unwrap() = content.to_string();
        self
    }
}

fn make_job(body: &str) -> JobRecord {
    JobRecord::new("j1", "claude", PROVIDER_NAME).with_request_body(body)
}

#[test]
fn test_manifest_capabilities_and_profiles() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(m.supports_resume);
    assert!(m.supports_subagents);
    assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
    assert!(m.supports_runtime_mode(&RuntimeMode::Headless));
    let pane = m.completion_manifest_for(&RuntimeMode::PaneBacked).unwrap();
    assert_eq!(pane.provider, PROVIDER_NAME);
    assert!(pane.poll_interval_ms > 0);
    assert!(pane.timeout_ms > 0);
}

#[test]
fn test_backend_includes_binding_and_launcher() {
    let b = backend();
    assert_eq!(b.provider(), PROVIDER_NAME);
    assert!(b.session_binding.is_some());
    let binding = b.session_binding.unwrap();
    assert_eq!(binding.session_id_attr, "claude_session_id");
    assert_eq!(binding.session_path_attr, "claude_session_path");
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_load_project_session_primary_and_instance() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();

    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_session_id": "primary"}),
    );
    let primary = load_project_session(&work_dir, None).unwrap();
    assert_eq!(primary.claude_session_id(), Some("primary"));

    write_session_file(
        &work_dir,
        Some("reviewer"),
        serde_json::json!({"claude_session_id": "reviewer"}),
    );
    let inst = load_project_session(&work_dir, Some("reviewer")).unwrap();
    assert_eq!(inst.claude_session_id(), Some("reviewer"));
}

#[test]
fn test_find_project_session_file_for_instance() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(
        &work_dir,
        Some("reviewer"),
        serde_json::json!({"claude_session_id": "reviewer"}),
    );
    let path = find_project_session_file(&work_dir, Some("reviewer")).unwrap();
    assert!(path.to_string_lossy().contains("reviewer"));
}

#[test]
fn test_wrap_claude_prompt_format() {
    let wrapped = wrap_claude_prompt("hello", "req-12345678");
    assert!(wrapped.contains("CCB_REQ_ID: req-12345678"));
    assert!(wrapped.contains("<<BEGIN:req-12345678>>"));
    assert!(wrapped.contains("<<DONE:req-12345678>>"));
}

#[test]
fn test_wrap_claude_turn_prompt_format() {
    let wrapped = wrap_claude_turn_prompt("hello", "req-12345678");
    assert!(wrapped.contains("CCB_REQ_ID: req-12345678"));
    assert!(!wrapped.contains("<<DONE:req-12345678>>"));
}

#[test]
fn test_adapter_start_without_session_is_error() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();

    let adapter = ClaudeExecutionAdapter;
    let job = make_job("hi");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
    assert_eq!(
        submission
            .runtime_state
            .get("mode")
            .unwrap()
            .as_str()
            .unwrap(),
        "error"
    );
    assert!(submission
        .runtime_state
        .get("reason")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("missing_claude_session"));
}

#[test]
fn test_adapter_start_active_sends_prompt_when_ready() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(&work_dir, None, serde_json::json!({}));

    let target = Arc::new(MockTarget::default().with_content("❯ "));
    let sent = target.sent.clone();
    let submission = with_prompt_target_override(target, || {
        let adapter = ClaudeExecutionAdapter;
        let job = make_job("do it");
        let ctx = ProviderRuntimeContext {
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });

    assert_eq!(
        submission
            .runtime_state
            .get("mode")
            .unwrap()
            .as_str()
            .unwrap(),
        "active"
    );
    assert_eq!(
        submission.source_kind,
        CompletionSourceKind::SessionEventLog
    );
    assert!(!submission.is_terminal());
    assert!(submission
        .runtime_state
        .get("prompt_sent")
        .and_then(|v| v.as_bool())
        .unwrap());
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert_eq!(sent.lock().unwrap()[0].0, "%1");
}

#[test]
fn test_adapter_poll_reads_session_log_events() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    let root = projects_root(&tmp);
    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_projects_root": root}),
    );

    let project_dir = make_project_dir(&root, &work_dir);
    std::fs::create_dir_all(&project_dir).unwrap();
    let session = project_dir.join("session.jsonl");
    std::fs::File::create(&session).unwrap();

    let target = Arc::new(MockTarget::default().with_content("❯ "));
    let adapter = ClaudeExecutionAdapter;
    let job = make_job("do it");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });
    assert!(submission
        .runtime_state
        .get("prompt_sent")
        .and_then(|v| v.as_bool())
        .unwrap());

    // Append events after the reader tail position.
    let anchor = submission
        .runtime_state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&session)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"{}"}}}}"#,
            anchor
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"chunk one"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"chunk two"}}}}"#
        )
        .unwrap();
    }

    let result = with_prompt_target_override(target, || {
        adapter.poll(&submission, "2025-01-01T00:00:02Z").unwrap()
    });

    assert!(!result.items.is_empty());
    assert!(result
        .items
        .iter()
        .any(|i| i.kind == CompletionItemKind::AnchorSeen));
    assert_eq!(
        result
            .items
            .iter()
            .filter(|i| i.kind == CompletionItemKind::AssistantChunk)
            .count(),
        2
    );
}

#[test]
fn test_adapter_poll_detects_end_turn() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    let root = projects_root(&tmp);
    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_projects_root": root}),
    );

    let project_dir = make_project_dir(&root, &work_dir);
    std::fs::create_dir_all(&project_dir).unwrap();
    let session = project_dir.join("session.jsonl");
    std::fs::File::create(&session).unwrap();

    let target = Arc::new(MockTarget::default().with_content("❯ "));
    let adapter = ClaudeExecutionAdapter;
    let job = make_job("do it");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });
    let anchor = submission
        .runtime_state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&session)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"{}"}}}}"#,
            anchor
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"final","stop_reason":"end_turn"}},"uuid":"uuid-1"}}"#
        )
        .unwrap();
    }

    let result = with_prompt_target_override(target, || {
        adapter.poll(&submission, "2025-01-01T00:00:02Z").unwrap()
    });

    let boundary = result
        .items
        .iter()
        .find(|i| i.kind == CompletionItemKind::TurnBoundary)
        .expect("turn boundary expected");
    assert_eq!(
        boundary.payload.get("reason").unwrap(),
        "assistant_end_turn"
    );
    assert_eq!(boundary.payload.get("stop_reason").unwrap(), "end_turn");
}

#[test]
fn test_adapter_poll_reads_subagent_events() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    let root = projects_root(&tmp);
    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_projects_root": root}),
    );

    let project_dir = make_project_dir(&root, &work_dir);
    std::fs::create_dir_all(&project_dir).unwrap();
    let session = project_dir.join("session.jsonl");
    std::fs::File::create(&session).unwrap();
    let sub_dir = session.with_extension("").join("subagents");
    std::fs::create_dir_all(&sub_dir).unwrap();
    let sub_log = sub_dir.join("sub1.jsonl");
    std::fs::File::create(&sub_log).unwrap();

    let target = Arc::new(MockTarget::default().with_content("❯ "));
    let adapter = ClaudeExecutionAdapter;
    let job = make_job("do it");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });
    let anchor = submission
        .runtime_state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&session)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"{}"}}}}"#,
            anchor
        )
        .unwrap();
    }
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&sub_log)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","role":"assistant","content":"sub reply","agentId":"sub-1","slug":"helper"}}"#
        )
        .unwrap();
    }

    let result = with_prompt_target_override(target, || {
        adapter.poll(&submission, "2025-01-01T00:00:02Z").unwrap()
    });

    let sub = result
        .items
        .iter()
        .find(|i| i.kind == CompletionItemKind::AssistantChunk)
        .expect("subagent chunk expected");
    assert_eq!(sub.payload.get("subagent_id").unwrap(), "sub-1");
    assert_eq!(sub.payload.get("subagent_name").unwrap(), "helper");
}

#[test]
fn test_adapter_poll_hook_artifact() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"completion_artifact_dir": work_dir.join("completion")}),
    );

    let completion_dir = work_dir.join("completion");
    let events_dir = completion_dir.join("events");
    std::fs::create_dir_all(&events_dir).unwrap();

    let target = Arc::new(MockTarget::default().with_content("❯ "));
    let adapter = ClaudeExecutionAdapter;
    let job = make_job("do it");
    let ctx = ProviderRuntimeContext {
        workspace_path: Some(work_dir.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = with_prompt_target_override(target.clone(), || {
        adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
    });
    let anchor = submission
        .runtime_state
        .get("request_anchor")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    write_json(
        &events_dir.join(format!("{}.json", anchor)),
        serde_json::json!({
            "req_id": anchor,
            "status": "completed",
            "reply": "hook reply",
            "session_id": "session-1",
            "timestamp": "2025-01-01T00:00:05Z",
        }),
    );

    let result = with_prompt_target_override(target, || {
        adapter.poll(&submission, "2025-01-01T00:00:02Z").unwrap()
    });

    assert!(result.decision.as_ref().unwrap().terminal);
    assert_eq!(
        result.decision.as_ref().unwrap().status,
        CompletionStatus::Completed
    );
    assert_eq!(result.submission.reply, "hook reply");
}

#[test]
fn test_session_registry_registers_and_refreshes() {
    let tmp = TempDir::new().unwrap();
    let work_dir = tmp.path().join("workspace");
    std::fs::create_dir(&work_dir).unwrap();
    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_session_path": "/path/old.jsonl"}),
    );

    let registry = ClaudeSessionRegistry::new();
    let entry = registry
        .register(&work_dir, "2025-01-01T00:00:00Z")
        .unwrap();
    assert_eq!(
        entry.session_path.as_ref().unwrap().to_string_lossy(),
        "/path/old.jsonl"
    );

    write_session_file(
        &work_dir,
        None,
        serde_json::json!({"claude_session_path": "/path/new.jsonl"}),
    );
    let entry = registry.refresh(&work_dir, "2025-01-01T00:01:00Z").unwrap();
    assert_eq!(
        entry.session_path.as_ref().unwrap().to_string_lossy(),
        "/path/new.jsonl"
    );
}

#[test]
fn test_global_session_registry_is_singleton() {
    let r1 = get_session_registry();
    let r2 = get_session_registry();
    assert!(std::ptr::eq(r1, r2));
}

#[test]
fn test_route_claude_binary_cache_moves_versions_to_shared_cache() {
    let tmp = TempDir::new().unwrap();
    let home_root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("home")).unwrap();
    let cache_root = camino::Utf8PathBuf::from_path_buf(tmp.path().join("cache")).unwrap();
    let versions_dir = home_root
        .join(".local")
        .join("share")
        .join("claude")
        .join("versions");
    std::fs::create_dir_all(&versions_dir).unwrap();
    std::fs::write(versions_dir.join("v1"), "v1 binary").unwrap();

    ccb_providers::claude::launcher_runtime::binary_cache::route_claude_binary_cache(
        &home_root,
        &cache_root,
        None,
    )
    .unwrap();

    assert!(cache_root.join("versions").join("v1").exists());
    assert!(versions_dir.is_symlink());
    assert_eq!(
        std::fs::read_link(versions_dir.as_std_path()).unwrap(),
        cache_root.join("versions").as_std_path()
    );
}
