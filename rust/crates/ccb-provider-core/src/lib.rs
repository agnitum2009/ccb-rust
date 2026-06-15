pub mod backend;
pub mod binding;
pub mod builtin_backends;
pub mod caller_env;
pub mod catalog;
pub mod contracts;
pub mod discovery;
pub mod discovery_candidates;
pub mod discovery_names;
pub mod discovery_workspace;
pub mod error;
pub mod execution;
pub mod extraction;
pub mod fields;
pub mod identity;
pub mod inspection;
pub mod instance_resolution;
pub mod loading;
pub mod manifest;
pub mod manifests;
pub mod markers;
pub mod memory_projection;
pub mod model_shortcuts;
pub mod pane;
pub mod pathing;
pub mod projected_assets;
pub mod protocol;
pub mod protocol_runtime;
pub mod registry;
pub mod reporting;
pub mod resolve;
pub mod runtime_lock;
pub mod runtime_shared;
pub mod runtime_specs;
pub mod session;
pub mod session_binding;
pub mod session_binding_evidence;
pub mod source_home;
pub mod state;
pub mod test_double_backends;
pub mod tmux_ownership;
pub mod utils;
pub mod validation;
pub mod validation_checks;

pub use error::{ProviderCoreError, Result};

// Crate-root re-exports matching Python `provider_core/__init__.py`.

pub use catalog::{build_default_provider_catalog, ProviderCatalog};

pub use runtime_specs::{
    make_qualified_key, parse_qualified_provider, provider_env_name, provider_marker_prefix,
    ProviderClientSpec, ProviderRuntimeSpec, AGY_CLIENT_SPEC, AGY_RUNTIME_SPEC, CLAUDE_CLIENT_SPEC,
    CLAUDE_RUNTIME_SPEC, CLIENT_SPECS_BY_PROVIDER, CODEBUDDY_CLIENT_SPEC, CODEBUDDY_RUNTIME_SPEC,
    CODEX_CLIENT_SPEC, CODEX_RUNTIME_SPEC, COPILOT_CLIENT_SPEC, COPILOT_RUNTIME_SPEC,
    CRUSH_CLIENT_SPEC, CRUSH_RUNTIME_SPEC, CURSOR_CLIENT_SPEC, CURSOR_RUNTIME_SPEC,
    DEEPSEEK_CLIENT_SPEC, DEEPSEEK_RUNTIME_SPEC, DROID_CLIENT_SPEC, DROID_RUNTIME_SPEC,
    GEMINI_CLIENT_SPEC, GEMINI_RUNTIME_SPEC, KIMI_CLIENT_SPEC, KIMI_RUNTIME_SPEC, KIRO_CLIENT_SPEC,
    KIRO_RUNTIME_SPEC, MIMO_CLIENT_SPEC, MIMO_RUNTIME_SPEC, OPENCODE_CLIENT_SPEC,
    OPENCODE_RUNTIME_SPEC, PI_CLIENT_SPEC, PI_RUNTIME_SPEC, QWEN_CLIENT_SPEC, QWEN_RUNTIME_SPEC,
    RUNTIME_SPECS_BY_PROVIDER,
};

pub use contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeIdentity, ProviderRuntimeLauncher,
    ProviderSessionBinding,
};

// Legacy protocol types are re-exported from `protocol`; the Python-aligned
// constants and functions come from `protocol_runtime`.
pub use protocol::{CodexRequest, CodexResult};
pub use protocol_runtime::{
    extract_reply_for_req, is_done_text, make_req_id, request_anchor_for_job, strip_done_text,
    strip_trailing_markers, wrap_codex_prompt, wrap_codex_turn_prompt, ANY_DONE_LINE_RE,
    ANY_REQ_ID_PATTERN, BEGIN_PREFIX, DONE_PREFIX, REQ_ID_BOUNDARY_PATTERN, REQ_ID_PREFIX,
};

pub use registry::{
    build_default_backend_registry, build_default_execution_adapters,
    build_default_provider_manifests, build_default_runtime_launcher_map,
    build_default_session_binding_map, ProviderBackendRegistry,
};

pub use runtime_lock::ProviderLock;

pub use session_binding::{
    binding_status, default_binding_adapter, inspect_session_pane, resolve_agent_binding,
    AgentBinding, BindingAdapter, PaneDetails,
};

pub use memory_projection::materialize_provider_memory_file;

// Provider-name constants are defined in `registry` and surfaced at the crate
// root (mirroring how Python exposes them via `catalog` and `registry`).
pub use registry::{CORE_PROVIDER_NAMES, OPTIONAL_PROVIDER_NAMES, TEST_DOUBLE_PROVIDER_NAMES};
