//! Compile-time check that the crate-root re-exports mirror Python's
//! `provider_core/__init__.py` public API.

// Import the whole public surface under a wildcard to prove the names exist.
use ccb_provider_core::*;

#[test]
fn test_crate_root_reexports_compile() {
    // Catalog
    let _ = ProviderCatalog::new(None);
    let _ = build_default_provider_catalog(true, true);
    let _ = CORE_PROVIDER_NAMES;
    let _ = OPTIONAL_PROVIDER_NAMES;
    let _ = TEST_DOUBLE_PROVIDER_NAMES;

    // Specs
    let _ = ProviderRuntimeSpec {
        provider_key: String::new(),
        service_name: String::new(),
        rpc_prefix: String::new(),
        state_file_name: String::new(),
        log_file_name: String::new(),
        idle_timeout_env: String::new(),
        lock_name: String::new(),
    };
    let _ = ProviderClientSpec {
        provider_key: String::new(),
        enabled_env: String::new(),
        autostart_env: String::new(),
        state_file_env: String::new(),
        session_filename: String::new(),
    };
    let _ = &*CODEX_RUNTIME_SPEC;
    let _ = &*KIMI_CLIENT_SPEC;
    let _ = RUNTIME_SPECS_BY_PROVIDER.len();
    let _ = CLIENT_SPECS_BY_PROVIDER.len();
    let _ = make_qualified_key("claude", Some("reviewer"));
    let _ = parse_qualified_provider("claude:reviewer");
    let _ = provider_env_name("claude", &["autostart"]);
    let _ = provider_marker_prefix("claude");

    // Contracts
    let _ = LaunchMode::SimpleTmux;

    // Protocol
    let _ = CodexRequest {
        client_id: String::new(),
        work_dir: String::new(),
        timeout_s: 0.0,
        quiet: false,
        message: String::new(),
        req_id: None,
        caller: String::new(),
    };
    let _ = CodexResult {
        exit_code: 0,
        reply: String::new(),
        req_id: String::new(),
        session_key: String::new(),
        log_path: None,
        anchor_seen: false,
        done_seen: false,
        fallback_scan: false,
        anchor_ms: None,
        done_ms: None,
    };
    let _ = BEGIN_PREFIX;
    let _ = DONE_PREFIX;
    let _ = REQ_ID_PREFIX;
    let _ = ANY_DONE_LINE_RE;
    let _ = ANY_REQ_ID_PATTERN;
    let _ = REQ_ID_BOUNDARY_PATTERN;
    let _ = make_req_id();
    let _ = request_anchor_for_job(Some("job-1"), None);
    let _ = extract_reply_for_req("", "job_1");
    let _ = is_done_text("", "job_1");
    let _ = strip_done_text("", "job_1");
    let _ = strip_trailing_markers("");
    let _ = wrap_codex_prompt("hi", "job_1");
    let _ = wrap_codex_turn_prompt("hi", "job_1");

    // Registry
    let _ = ProviderBackendRegistry::new();
    let _ = build_default_backend_registry(true, true);
    let _ = build_default_execution_adapters(true, true);
    let _ = build_default_provider_manifests(true, true);
    let _ = build_default_runtime_launcher_map(true);
    let _ = build_default_session_binding_map(true);

    // Lock
    let _ = ProviderLock::new("claude", 1.0, None);

    // Session binding
    let _ = AgentBinding::default();
    let _ = binding_status(Some("r"), Some("s"), Some("w"));
    let _ = default_binding_adapter("claude");
    let _ = inspect_session_pane(&Default::default());
}
