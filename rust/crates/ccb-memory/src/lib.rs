pub mod deduper;
pub mod formatter;
pub mod project_memory;
pub mod session_parser;
pub mod transfer;
pub mod types;

pub use deduper::{ConversationDeduper, PROTOCOL_PATTERNS, SYSTEM_NOISE_PATTERNS};
pub use formatter::ContextFormatter;
pub use session_parser::{claude_projects_root, ClaudeSessionParser};
pub use transfer::{auto_source_candidates, ContextTransfer, SUPPORTED_SOURCES};
pub use types::{
    ConversationEntry, MemoryError, ProjectMemoryEnsureResult, ProjectMemoryMaterialization,
    ProjectMemorySource, ProjectMemorySourceRef, Result, SessionInfo, SessionStats, ToolExecution,
    TransferContext,
};

// Re-export project memory items at the crate root for ergonomic use.
pub use project_memory::{
    agent_private_memory_path, ensure_project_memory, filter_memory_source, filters_for_source,
    load_memory_sources, materialize_runtime_memory_bundle, memory_policy_for_provider,
    project_memory_path, provider_native_memory_path, read_memory_source, read_seed_metadata,
    render_memory_bundle, runtime_memory_bundle_path, seed_metadata_path, sha256_text,
    should_include_source, MemorySourcePolicy, ProviderMemoryPolicy, FILTER_CCB_INSTALL_BLOCKS,
    SOURCE_AGENT_PRIVATE, SOURCE_CCB_SHARED, SOURCE_PROVIDER_NATIVE_PROJECT,
    SOURCE_PROVIDER_USER_MEMORY, SOURCE_RULES_DIR, SOURCE_RUNTIME_COORDINATION_RULES,
};
