use ccbr_provider_core::contracts::LaunchMode;
use ccbr_provider_core::manifest::{
    CompletionFamily, CompletionSourceKind, RuntimeMode, SelectorFamily,
};
use ccbr_providers::providers::zai::{
    backend, build_execution_adapter, find_project_session_file, load_project_session, manifest,
    observe_zai_output, PROVIDER_NAME,
};

#[test]
fn test_manifest_matches_python_native_cli_zai_contract() {
    let manifest = manifest();
    assert_eq!(manifest.provider, PROVIDER_NAME);
    assert!(!manifest.supports_resume);
    assert!(!manifest.supports_permission_auto);
    assert!(!manifest.supports_stream_watch);
    assert!(manifest.supports_subagents);
    assert!(manifest.supports_workspace_attach);

    let pane = manifest
        .completion_manifest_for(&RuntimeMode::PaneBacked)
        .unwrap();
    assert_eq!(pane.completion_family, CompletionFamily::StructuredResult);
    assert_eq!(
        pane.completion_source_kind,
        CompletionSourceKind::StructuredResultStream
    );
    assert!(!pane.supports_exact_completion);
    assert!(pane.supports_observed_completion);
    assert!(pane.supports_anchor_binding);
    assert_eq!(pane.selector_family, SelectorFamily::StructuredResult);
}

#[test]
fn test_backend_includes_zai_binding_and_launcher() {
    let backend = backend();
    assert_eq!(backend.provider(), PROVIDER_NAME);
    let binding = backend.session_binding.unwrap();
    assert_eq!(binding.provider, PROVIDER_NAME);
    assert_eq!(binding.session_id_attr, "zai_session_id");
    assert_eq!(binding.session_path_attr, "zai_session_path");
    let launcher = backend.runtime_launcher.unwrap();
    assert_eq!(launcher.provider, PROVIDER_NAME);
    assert_eq!(launcher.launch_mode, LaunchMode::SimpleTmux);
}

#[test]
fn test_execution_adapter_provider_name() {
    let adapter = build_execution_adapter();
    assert_eq!(adapter.provider(), PROVIDER_NAME);
}

#[test]
fn test_observe_zai_output_reads_assistant_json_and_skips_progress() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("zai.jsonl");
    std::fs::write(
        &path,
        r#"{"role":"assistant","content":"thinking..."}
{"role":"user","content":"ignore me"}
{"role":"assistant","content":"hello ","id":"turn-1","timestamp":"2026-06-27T00:00:00Z"}
{"type":"assistant_delta","payload":{"text":"world"}}
"#,
    )
    .unwrap();

    let observed = observe_zai_output(&path);
    assert_eq!(observed.text, "hello world");
    assert_eq!(observed.turn_ref.as_deref(), Some("turn-1"));
    assert!(observed.error.is_empty());
}

#[test]
fn test_observe_zai_output_keeps_plain_stdout_fallback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("zai.out");
    std::fs::write(&path, "plain answer\nsecond line\n").unwrap();

    let observed = observe_zai_output(&path);
    assert_eq!(observed.text, "plain answer\nsecond line");
    assert!(observed.error.is_empty());
}

#[test]
fn test_observe_zai_output_reports_error_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("zai.jsonl");
    std::fs::write(&path, r#"{"type":"error","message":"boom"}"#).unwrap();

    let observed = observe_zai_output(&path);
    assert_eq!(observed.error, "boom");
}

#[test]
fn test_zai_session_file_lookup_and_loading() {
    let tmp = tempfile::TempDir::new().unwrap();
    let session_path = tmp.path().join(".zai-session");
    std::fs::write(
        &session_path,
        r#"{"zai_session_id":"s1","zai_session_path":"/tmp/s1"}"#,
    )
    .unwrap();

    assert_eq!(
        find_project_session_file(tmp.path(), None),
        Some(session_path.clone())
    );
    let session = load_project_session(tmp.path(), None).unwrap();
    assert_eq!(session.session_file, session_path);
    assert_eq!(session.zai_session_id(), "s1");
    assert_eq!(session.zai_session_path(), "/tmp/s1");
}
