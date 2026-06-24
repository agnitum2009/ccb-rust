pub const SOURCE_RUNTIME_COORDINATION_RULES: &str = "runtime_coordination_rules";
pub const SOURCE_CCBR_SHARED: &str = "ccbr_shared";
pub const SOURCE_PROVIDER_USER_MEMORY: &str = "provider_user_memory";
pub const SOURCE_PROVIDER_NATIVE_PROJECT: &str = "provider_native_project";
pub const SOURCE_AGENT_PRIVATE: &str = "agent_private";
pub const SOURCE_RULES_DIR: &str = "rules_dir";

pub const FILTER_CCBR_INSTALL_BLOCKS: &str = "ccbr_install_blocks";

/// Policy for a single memory source kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySourcePolicy {
    pub include_in_bundle: bool,
    pub filters: Vec<String>,
}

impl MemorySourcePolicy {
    pub fn new(include_in_bundle: bool, filters: Vec<String>) -> Self {
        Self {
            include_in_bundle,
            filters,
        }
    }
}

/// Policy for a provider's memory sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMemoryPolicy {
    pub provider: String,
    pub sources: std::collections::HashMap<String, MemorySourcePolicy>,
}

impl ProviderMemoryPolicy {
    pub fn source_policy(&self, kind: &str) -> MemorySourcePolicy {
        self.sources
            .get(kind)
            .cloned()
            .unwrap_or_else(|| MemorySourcePolicy {
                include_in_bundle: true,
                filters: Vec::new(),
            })
    }
}

/// Get the memory policy for a provider.
pub fn memory_policy_for_provider(provider: &str) -> ProviderMemoryPolicy {
    let key = provider.trim().to_lowercase();
    match key.as_str() {
        "claude" => policy(provider, false, true),
        "codex" => policy(provider, false, true),
        "opencode" => policy(provider, false, true),
        "gemini" => policy(provider, true, true),
        _ => policy("default", true, false),
    }
}

/// Whether a source kind should be included for a provider.
pub fn should_include_source(provider: &str, kind: &str) -> bool {
    memory_policy_for_provider(provider)
        .source_policy(kind)
        .include_in_bundle
}

/// Filters to apply to a source kind for a provider.
pub fn filters_for_source(provider: &str, kind: &str) -> Vec<String> {
    memory_policy_for_provider(provider)
        .source_policy(kind)
        .filters
        .clone()
}

fn policy(
    provider: &str,
    include_provider_native_project: bool,
    filter_provider_user_memory: bool,
) -> ProviderMemoryPolicy {
    let user_filters = if filter_provider_user_memory {
        vec![FILTER_CCBR_INSTALL_BLOCKS.to_string()]
    } else {
        Vec::new()
    };

    let mut sources = std::collections::HashMap::new();
    sources.insert(
        SOURCE_RUNTIME_COORDINATION_RULES.to_string(),
        MemorySourcePolicy::new(true, Vec::new()),
    );
    sources.insert(
        SOURCE_CCBR_SHARED.to_string(),
        MemorySourcePolicy::new(true, Vec::new()),
    );
    sources.insert(
        SOURCE_PROVIDER_USER_MEMORY.to_string(),
        MemorySourcePolicy::new(true, user_filters),
    );
    sources.insert(
        SOURCE_PROVIDER_NATIVE_PROJECT.to_string(),
        MemorySourcePolicy::new(include_provider_native_project, Vec::new()),
    );
    sources.insert(
        SOURCE_AGENT_PRIVATE.to_string(),
        MemorySourcePolicy::new(true, Vec::new()),
    );
    sources.insert(
        SOURCE_RULES_DIR.to_string(),
        MemorySourcePolicy::new(false, Vec::new()),
    );

    ProviderMemoryPolicy {
        provider: provider.to_string(),
        sources,
    }
}
