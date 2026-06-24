use std::collections::HashMap;

use crate::error::{ProviderCoreError, Result};
use crate::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use crate::registry::build_default_provider_manifests;

/// Catalog of provider manifests.
pub struct ProviderCatalog {
    manifests: HashMap<String, ProviderManifest>,
}

impl ProviderCatalog {
    /// Create a catalog from an optional list of manifests.
    pub fn new(manifests: Option<Vec<ProviderManifest>>) -> Self {
        let mut catalog = Self {
            manifests: HashMap::new(),
        };
        if let Some(list) = manifests {
            for manifest in list {
                let _ = catalog.register(manifest);
            }
        }
        catalog
    }

    /// Register a manifest in the catalog.
    pub fn register(&mut self, manifest: ProviderManifest) -> Result<()> {
        let provider = manifest.provider.clone();
        if self.manifests.contains_key(&provider) {
            return Err(ProviderCoreError::DuplicateManifest(provider));
        }
        self.manifests.insert(provider, manifest);
        Ok(())
    }

    /// Get a manifest by provider name.
    pub fn get(&self, provider: &str) -> Result<&ProviderManifest> {
        let key = provider.trim().to_lowercase();
        self.manifests
            .get(&key)
            .ok_or_else(|| ProviderCoreError::UnknownProvider(provider.to_string()))
    }

    /// Resolve the completion manifest for a provider/runtime-mode pair.
    pub fn resolve_completion_manifest(
        &self,
        provider: &str,
        runtime_mode: &RuntimeMode,
    ) -> Result<&CompletionManifest> {
        let manifest = self.get(provider)?;
        if !manifest.supports_runtime_mode(runtime_mode) {
            return Err(ProviderCoreError::UnsupportedProvider(format!(
                "provider {} does not support runtime_mode {:?}",
                manifest.provider, runtime_mode
            )));
        }
        Ok(manifest.completion_manifest_for(runtime_mode).unwrap())
    }

    /// Return the sorted list of registered provider names.
    pub fn providers(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.manifests.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

/// Build a catalog with default provider manifests.
pub fn build_default_provider_catalog(
    include_optional: bool,
    include_test_doubles: bool,
) -> ProviderCatalog {
    ProviderCatalog::new(Some(build_default_provider_manifests(
        include_optional,
        include_test_doubles,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(name: &str) -> ProviderManifest {
        use crate::manifest::{CompletionFamily, CompletionSourceKind, SelectorFamily};
        let mut profiles = HashMap::new();
        profiles.insert(
            RuntimeMode::PaneBacked,
            CompletionManifest {
                provider: name.to_string(),
                runtime_mode: "pane-backed".to_string(),
                poll_interval_ms: 500,
                timeout_ms: 30000,
                completion_family: CompletionFamily::ProtocolTurn,
                completion_source_kind: CompletionSourceKind::ProtocolEventStream,
                supports_exact_completion: true,
                supports_observed_completion: false,
                supports_anchor_binding: true,
                supports_reply_stability: false,
                supports_terminal_reason: true,
                selector_family: SelectorFamily::FinalMessage,
            },
        );
        ProviderManifest::new(name, true, false, false, false, false, profiles)
    }

    #[test]
    fn test_catalog_register_and_get() {
        let mut catalog = ProviderCatalog::new(None);
        catalog.register(manifest("claude")).unwrap();
        assert!(catalog.get("claude").is_ok());
        assert!(catalog.get("CLAUDE").is_ok());
        assert!(catalog.get("codex").is_err());
    }

    #[test]
    fn test_catalog_providers_sorted() {
        let mut catalog = ProviderCatalog::new(None);
        catalog.register(manifest("codex")).unwrap();
        catalog.register(manifest("claude")).unwrap();
        assert_eq!(catalog.providers(), vec!["claude", "codex"]);
    }

    #[test]
    fn test_catalog_duplicate() {
        let mut catalog = ProviderCatalog::new(None);
        catalog.register(manifest("claude")).unwrap();
        assert!(catalog.register(manifest("claude")).is_err());
    }

    #[test]
    fn default_catalog_contains_expected_providers_and_families() {
        use crate::manifest::{CompletionFamily, CompletionSourceKind};
        let catalog = build_default_provider_catalog(true, true);
        let providers: std::collections::HashSet<String> =
            catalog.providers().iter().map(|s| s.to_string()).collect();
        for expected in &[
            "fake",
            "fake-codex",
            "fake-claude",
            "fake-gemini",
            "fake-legacy",
            "claude",
            "codex",
            "gemini",
            "opencode",
            "droid",
            "agy",
            "kimi",
            "deepseek",
        ] {
            assert!(
                providers.contains(*expected),
                "missing provider {}",
                expected
            );
        }

        let codex = catalog
            .resolve_completion_manifest("codex", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(codex.completion_family, CompletionFamily::ProtocolTurn);

        let gemini = catalog
            .resolve_completion_manifest("gemini", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(
            gemini.completion_family,
            CompletionFamily::AnchoredSessionStability
        );

        let fake = catalog
            .resolve_completion_manifest("fake", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(fake.completion_family, CompletionFamily::StructuredResult);

        let fake_codex = catalog
            .resolve_completion_manifest("fake-codex", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(fake_codex.completion_family, CompletionFamily::ProtocolTurn);

        let fake_gemini = catalog
            .resolve_completion_manifest("fake-gemini", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(
            fake_gemini.completion_family,
            CompletionFamily::AnchoredSessionStability
        );

        let agy = catalog
            .resolve_completion_manifest("agy", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(agy.completion_family, CompletionFamily::SessionBoundary);
        assert_eq!(
            agy.completion_source_kind,
            CompletionSourceKind::SessionEventLog
        );
        assert!(agy.supports_observed_completion);
        assert!(agy.supports_anchor_binding);
        assert!(catalog.get("agy").unwrap().supports_resume);

        let kimi = catalog
            .resolve_completion_manifest("kimi", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(kimi.completion_family, CompletionFamily::SessionBoundary);
        assert_eq!(
            kimi.completion_source_kind,
            CompletionSourceKind::SessionEventLog
        );
        assert!(kimi.supports_observed_completion);
        assert!(kimi.supports_anchor_binding);

        let deepseek = catalog
            .resolve_completion_manifest("deepseek", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(
            deepseek.completion_family,
            CompletionFamily::SessionBoundary
        );
        assert_eq!(
            deepseek.completion_source_kind,
            CompletionSourceKind::SessionSnapshot
        );
        assert!(deepseek.supports_observed_completion);
        assert!(deepseek.supports_anchor_binding);

        let fake_legacy = catalog
            .resolve_completion_manifest("fake-legacy", &RuntimeMode::PaneBacked)
            .unwrap();
        assert_eq!(
            fake_legacy.completion_family,
            CompletionFamily::TerminalTextQuiet
        );
    }

    #[test]
    fn core_only_catalog_excludes_optional_and_test_doubles() {
        let catalog = build_default_provider_catalog(false, false);
        let providers: std::collections::HashSet<String> =
            catalog.providers().iter().map(|s| s.to_string()).collect();
        assert_eq!(
            providers,
            ["claude", "codex", "gemini"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        );
    }
}
