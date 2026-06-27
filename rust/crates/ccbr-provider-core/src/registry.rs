use std::collections::HashMap;

use crate::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use crate::manifest::{
    CompletionFamily, CompletionManifest, CompletionSourceKind, ProviderManifest, RuntimeMode,
    SelectorFamily,
};

/// Provider names that are always included.
pub const CORE_PROVIDER_NAMES: &[&str] = &["claude", "codex", "gemini"];

/// Provider names that are optionally included.
/// Mirrors Python `provider_core.registry_runtime.OPTIONAL_PROVIDER_NAMES`.
pub const OPTIONAL_PROVIDER_NAMES: &[&str] = &["opencode", "droid", "agy", "kimi", "deepseek"];

/// Additional Rust-only optional providers kept for backward compatibility.
pub const EXTRA_PROVIDER_NAMES: &[&str] = &[
    "qwen",
    "copilot",
    "codebuddy",
    "cursor",
    "crush",
    "kiro",
    "pi",
];

/// Provider names used as test doubles.
pub const TEST_DOUBLE_PROVIDER_NAMES: &[&str] = &[
    "fake",
    "fake-codex",
    "fake-claude",
    "fake-gemini",
    "fake-legacy",
];

/// Registry of provider backends, keyed by provider name.
pub struct ProviderBackendRegistry {
    backends: HashMap<String, ProviderBackend>,
}

impl ProviderBackendRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a backend.
    pub fn register(&mut self, backend: ProviderBackend) {
        let provider = backend.provider().to_string();
        if self.backends.contains_key(&provider) {
            panic!("duplicate provider backend: {}", provider);
        }
        self.backends.insert(provider, backend);
    }

    /// Get a backend by provider name.
    pub fn get(&self, provider: &str) -> Option<&ProviderBackend> {
        let key = provider.trim().to_lowercase();
        self.backends.get(&key)
    }

    /// Return references to all registered manifests.
    pub fn manifests(&self) -> Vec<&ProviderManifest> {
        self.backends.values().map(|b| &b.manifest).collect()
    }

    /// Return all registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.backends.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of registered backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Return true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    /// Return references to all registered execution adapters.
    pub fn execution_adapters(&self) -> Vec<&dyn crate::execution::ExecutionAdapter> {
        self.backends
            .values()
            .filter_map(|b| b.execution_adapter.as_ref().map(|ea| ea.as_ref()))
            .collect()
    }

    /// Return a map of provider names to session bindings.
    pub fn session_bindings(
        &self,
    ) -> std::collections::HashMap<&str, &crate::contracts::ProviderSessionBinding> {
        self.backends
            .values()
            .filter_map(|b| b.session_binding.as_ref().map(|sb| (b.provider(), sb)))
            .collect()
    }

    /// Return a map of provider names to runtime launchers.
    pub fn runtime_launchers(
        &self,
    ) -> std::collections::HashMap<&str, &crate::contracts::ProviderRuntimeLauncher> {
        self.backends
            .values()
            .filter_map(|b| b.runtime_launcher.as_ref().map(|rl| (b.provider(), rl)))
            .collect()
    }
}

impl Default for ProviderBackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a registry containing the default set of provider backends.
///
/// Real provider backends are created with manifests only; execution adapters,
/// session bindings, and launchers must be supplied by the caller or a later
/// migration phase.
pub fn build_default_backend_registry(
    include_optional: bool,
    include_test_doubles: bool,
) -> ProviderBackendRegistry {
    let mut registry = ProviderBackendRegistry::new();
    if include_test_doubles {
        for backend in build_test_double_backends() {
            registry.register(backend);
        }
    }
    for backend in build_builtin_backends(include_optional) {
        registry.register(backend);
    }
    registry
}

/// Build the default provider manifests.
pub fn build_default_provider_manifests(
    include_optional: bool,
    include_test_doubles: bool,
) -> Vec<ProviderManifest> {
    build_default_backend_registry(include_optional, include_test_doubles)
        .manifests()
        .into_iter()
        .cloned()
        .collect()
}

/// Build the default execution adapters.
pub fn build_default_execution_adapters(
    _include_optional: bool,
    _include_test_doubles: bool,
) -> Vec<Box<dyn crate::execution::ExecutionAdapter>> {
    // Execution adapters live in provider-execution/provider-backends and are
    // not available at this layer. Return an empty list to keep the API
    // surface aligned with Python.
    Vec::new()
}

/// Build a map of provider names to session bindings.
pub fn build_default_session_binding_map(
    include_optional: bool,
) -> HashMap<String, ProviderSessionBinding> {
    let mut map = HashMap::new();
    for &provider in CORE_PROVIDER_NAMES {
        if let Some(binding) = session_binding_for_provider(provider) {
            map.insert(provider.to_string(), binding);
        }
    }
    if include_optional {
        for &provider in OPTIONAL_PROVIDER_NAMES {
            if let Some(binding) = session_binding_for_provider(provider) {
                map.insert(provider.to_string(), binding);
            }
        }
    }
    map
}

/// Build a map of provider names to runtime launchers.
pub fn build_default_runtime_launcher_map(
    include_optional: bool,
) -> HashMap<String, ProviderRuntimeLauncher> {
    let mut map = HashMap::new();
    for &provider in CORE_PROVIDER_NAMES {
        if let Some(launcher) = runtime_launcher_for_provider(provider) {
            map.insert(provider.to_string(), launcher);
        }
    }
    if include_optional {
        for &provider in OPTIONAL_PROVIDER_NAMES {
            if let Some(launcher) = runtime_launcher_for_provider(provider) {
                map.insert(provider.to_string(), launcher);
            }
        }
    }
    map
}

fn session_binding_for_provider(provider: &str) -> Option<ProviderSessionBinding> {
    let provider = provider.trim().to_lowercase();
    if provider.is_empty() {
        return None;
    }
    let (session_id_attr, session_path_attr) = match provider.as_str() {
        "opencode" => (
            "opencode_session_id".to_string(),
            "session_file".to_string(),
        ),
        "codex" | "claude" | "gemini" | "droid" | "agy" | "kimi" | "deepseek" | "zai" => (
            format!("{}_session_id", provider),
            format!("{}_session_path", provider),
        ),
        _ => return None,
    };
    let mut binding = ProviderSessionBinding::new(&provider);
    binding.session_id_attr = session_id_attr;
    binding.session_path_attr = session_path_attr;
    Some(binding)
}

fn runtime_launcher_for_provider(provider: &str) -> Option<ProviderRuntimeLauncher> {
    let provider = provider.trim().to_lowercase();
    if provider.is_empty() {
        return None;
    }
    let launch_mode = match provider.as_str() {
        "codex" => LaunchMode::CodexTmux,
        "claude" | "gemini" | "opencode" | "droid" | "agy" | "kimi" | "deepseek" | "zai" => {
            LaunchMode::SimpleTmux
        }
        _ => return None,
    };
    Some(ProviderRuntimeLauncher::new(provider, launch_mode))
}

fn build_builtin_backends(include_optional: bool) -> Vec<ProviderBackend> {
    let mut backends = Vec::new();
    for provider in CORE_PROVIDER_NAMES {
        backends.push(ProviderBackend {
            manifest: default_manifest(provider),
            execution_adapter: None,
            session_binding: None,
            runtime_launcher: None,
        });
    }
    if include_optional {
        for provider in OPTIONAL_PROVIDER_NAMES {
            backends.push(ProviderBackend {
                manifest: default_manifest(provider),
                execution_adapter: None,
                session_binding: None,
                runtime_launcher: None,
            });
        }
        for provider in EXTRA_PROVIDER_NAMES {
            backends.push(ProviderBackend {
                manifest: default_manifest(provider),
                execution_adapter: None,
                session_binding: None,
                runtime_launcher: None,
            });
        }
    }
    backends
}

fn build_test_double_backends() -> Vec<ProviderBackend> {
    TEST_DOUBLE_PROVIDER_NAMES
        .iter()
        .map(|provider| ProviderBackend {
            manifest: default_manifest(provider),
            execution_adapter: None,
            session_binding: None,
            runtime_launcher: None,
        })
        .collect()
}

fn default_manifest(provider: &str) -> ProviderManifest {
    let provider = provider.trim().to_lowercase();
    let (family, source, supports_observed, supports_anchor) = match provider.as_str() {
        "codex" | "fake-codex" | "claude" | "fake-claude" => (
            CompletionFamily::ProtocolTurn,
            CompletionSourceKind::ProtocolEventStream,
            false,
            true,
        ),
        "gemini" | "fake-gemini" => (
            CompletionFamily::AnchoredSessionStability,
            CompletionSourceKind::ProtocolEventStream,
            false,
            true,
        ),
        "agy" | "kimi" => (
            CompletionFamily::SessionBoundary,
            CompletionSourceKind::SessionEventLog,
            true,
            true,
        ),
        "deepseek" => (
            CompletionFamily::SessionBoundary,
            CompletionSourceKind::SessionSnapshot,
            true,
            true,
        ),
        "zai" => (
            CompletionFamily::StructuredResult,
            CompletionSourceKind::StructuredResultStream,
            true,
            true,
        ),
        "fake" => (
            CompletionFamily::StructuredResult,
            CompletionSourceKind::ProtocolEventStream,
            false,
            false,
        ),
        "fake-legacy" => (
            CompletionFamily::TerminalTextQuiet,
            CompletionSourceKind::ProtocolEventStream,
            false,
            false,
        ),
        _ => (
            CompletionFamily::ProtocolTurn,
            CompletionSourceKind::ProtocolEventStream,
            false,
            true,
        ),
    };

    let mut profiles = HashMap::new();
    profiles.insert(
        RuntimeMode::PaneBacked,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "pane-backed".to_string(),
            poll_interval_ms: 500,
            timeout_ms: 30000,
            completion_family: family,
            completion_source_kind: source,
            supports_exact_completion: true,
            supports_observed_completion: supports_observed,
            supports_anchor_binding: supports_anchor,
            supports_reply_stability: false,
            supports_terminal_reason: true,
            selector_family: SelectorFamily::FinalMessage,
        },
    );
    let (supports_resume, supports_subagents, supports_workspace_attach) = if provider == "zai" {
        (false, true, true)
    } else {
        (true, false, false)
    };
    ProviderManifest::new(
        &provider,
        supports_resume,
        false,
        false,
        supports_subagents,
        supports_workspace_attach,
        profiles,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_backend(name: &str) -> ProviderBackend {
        ProviderBackend {
            manifest: default_manifest(name),
            execution_adapter: None,
            session_binding: None,
            runtime_launcher: None,
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut reg = ProviderBackendRegistry::new();
        reg.register(test_backend("claude"));
        assert!(reg.get("claude").is_some());
        assert!(reg.get("codex").is_none());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_registry_case_insensitive() {
        let mut reg = ProviderBackendRegistry::new();
        reg.register(test_backend("Claude"));
        assert!(reg.get("claude").is_some());
        assert!(reg.get("CLAUDE").is_some());
    }

    #[test]
    fn test_registry_manifests() {
        let mut reg = ProviderBackendRegistry::new();
        reg.register(test_backend("claude"));
        reg.register(test_backend("codex"));
        let manifests = reg.manifests();
        assert_eq!(manifests.len(), 2);
    }

    #[test]
    #[should_panic(expected = "duplicate provider backend")]
    fn test_registry_duplicate_panics() {
        let mut reg = ProviderBackendRegistry::new();
        reg.register(test_backend("claude"));
        reg.register(test_backend("claude"));
    }

    #[test]
    fn test_build_default_backend_registry() {
        let reg = build_default_backend_registry(true, true);
        assert!(reg.get("claude").is_some());
        assert!(reg.get("opencode").is_some());
        assert!(reg.get("kimi").is_some());
        assert!(reg.get("deepseek").is_some());
        assert!(reg.get("fake").is_some());
    }

    #[test]
    fn test_registry_aggregator_methods() {
        let reg = build_default_backend_registry(true, true);
        assert!(reg.execution_adapters().is_empty());
        assert!(reg.session_bindings().is_empty());
        assert!(reg.runtime_launchers().is_empty());
    }
}
