use std::io::Write;

use ccb_completion::models::{CompletionItemKind, CompletionStatus, JobRecord};
use ccb_provider_core::manifest::RuntimeMode;
use ccb_providers::{
    build_default_backend_registry, build_default_execution_registry,
    droid::{
        extract_reply_for_req, is_done_text, managed_droid_home_for_runtime, strip_done_text,
        wrap_droid_prompt, DroidLogReader, LogEvent,
    },
    execution::ExecutionAdapter,
    providers::droid::{backend, manifest, DroidExecutionAdapter, PROVIDER_NAME},
};
use serde_json::Value;
use tempfile::TempDir;

fn fake_now() -> String {
    "2025-01-01T00:00:00Z".to_string()
}

fn job_with_body(job_id: &str, agent_name: &str, body: &str) -> JobRecord {
    JobRecord {
        job_id: job_id.to_string(),
        agent_name: agent_name.to_string(),
        provider: PROVIDER_NAME.to_string(),
        target_kind: ccb_completion::models::TargetKind::Agent,
        request: ccb_completion::models::JobRequest {
            body: body.to_string(),
            message_type: None,
        },
        provider_options: serde_json::Map::new(),
        workspace_path: None,
        provider_instance: None,
    }
}

#[test]
fn test_manifest_capabilities_and_profiles() {
    let m = manifest();
    assert_eq!(m.provider, PROVIDER_NAME);
    assert!(!m.supports_resume);
    assert!(m.supports_workspace_attach);
    assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
}

#[test]
fn test_backend_includes_binding_and_launcher() {
    let b = backend();
    assert_eq!(b.provider(), PROVIDER_NAME);
    assert!(b.session_binding.is_some());
    assert!(b.runtime_launcher.is_some());
}

#[test]
fn test_execution_registry_contains_adapter() {
    let registry = build_default_execution_registry();
    assert!(registry.get(PROVIDER_NAME).is_some());
}

#[test]
fn test_backend_registry_contains_droid() {
    let registry = build_default_backend_registry();
    assert!(registry.get(PROVIDER_NAME).is_some());
}

#[test]
fn test_wrap_droid_prompt_format() {
    let wrapped = wrap_droid_prompt("do the thing", "req-12345678");
    assert!(wrapped.contains("CCB_REQ_ID: req-12345678"));
    assert!(wrapped.contains("do the thing"));
    assert!(wrapped.contains("CCB_DONE: req-12345678"));
}

#[test]
fn test_is_done_text_and_strip_done_text() {
    let text = "some reply\nCCB_DONE: req-1";
    assert!(is_done_text(text, "req-1"));
    assert_eq!(strip_done_text(text, "req-1"), "some reply");
}

#[test]
fn test_extract_reply_for_req() {
    let text = "reply body\nCCB_DONE: req-1";
    assert_eq!(extract_reply_for_req(text, "req-1"), "reply body");
}

#[test]
fn test_managed_droid_home_for_runtime() {
    let runtime = std::path::PathBuf::from("/tmp/agent/.ccbr/provider-runtime/droid");
    let home = managed_droid_home_for_runtime(&runtime);
    assert!(home.to_string_lossy().contains("provider-state/droid/home"));
}

#[test]
fn test_execution_adapter_start_sets_runtime_state() {
    let adapter = DroidExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello droid");
    let submission = adapter.start(&job, None, &fake_now());
    assert_eq!(submission.provider, PROVIDER_NAME);
    assert_eq!(submission.runtime_state.get("mode").unwrap(), "active");
    assert!(submission.runtime_state.contains_key("request_anchor"));
    assert!(submission.runtime_state.contains_key("prompt"));
}

#[test]
fn test_log_reader_reads_events() {
    let dir = TempDir::new().unwrap();
    let work_dir = dir.path().to_string_lossy();
    let session = dir.path().join("session.jsonl");
    let mut file = std::fs::File::create(&session).unwrap();
    writeln!(
        file,
        r#"{{"type":"message","message":{{"role":"user","content":"hi"}},"work_dir":"{}"}}"#,
        work_dir
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"message","message":{{"role":"assistant","content":"hello"}},"work_dir":"{}"}}"#,
        work_dir
    )
    .unwrap();

    let reader = DroidLogReader::new(Some(dir.path()), Some(dir.path()));
    let state = reader.capture_state();
    let (events, _new_state) = reader.try_get_events(&state);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], LogEvent::User("hi".to_string()));
    assert_eq!(events[1], LogEvent::Assistant("hello".to_string()));
}

#[test]
fn test_execution_adapter_poll_emits_items() {
    let dir = TempDir::new().unwrap();
    let work_dir = dir.path().to_string_lossy();
    let session = dir.path().join("session.jsonl");
    let mut file = std::fs::File::create(&session).unwrap();
    let request_anchor = "req-123";
    writeln!(
        file,
        r#"{{"type":"message","message":{{"role":"user","content":"CCB_REQ_ID: {}"}},"work_dir":"{}"}}"#,
        request_anchor, work_dir
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"type":"message","message":{{"role":"assistant","content":"reply body\nCCB_DONE: {}"}},"work_dir":"{}"}}"#,
        request_anchor, work_dir
    )
    .unwrap();

    let adapter = DroidExecutionAdapter;
    let job = job_with_body("j1", "agent1", "hello");
    let ctx = ccb_providers::execution::ProviderRuntimeContext {
        agent_name: "agent1".to_string(),
        workspace_path: Some(dir.path().to_string_lossy().to_string()),
        backend_type: Some("tmux".to_string()),
        runtime_ref: Some("%12".to_string()),
        session_ref: Some(session.to_string_lossy().to_string()),
        ..Default::default()
    };
    let submission = adapter.start(&job, Some(&ctx), &fake_now());

    // Override the request anchor to match the log entries.
    let mut submission = submission;
    submission.runtime_state.insert(
        "request_anchor".to_string(),
        Value::String(request_anchor.to_string()),
    );
    submission.runtime_state.insert(
        "session_path".to_string(),
        Value::String(session.to_string_lossy().to_string()),
    );

    let result = adapter.poll(&submission, &fake_now());
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(!result.items.is_empty());
    let final_item = result
        .items
        .iter()
        .find(|i| i.kind == CompletionItemKind::AssistantFinal);
    assert!(final_item.is_some());
    assert!(result.decision.is_some());
    assert_eq!(result.decision.unwrap().status, CompletionStatus::Completed);
}
