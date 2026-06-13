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
        let mut profiles = HashMap::new();
        profiles.insert(
            RuntimeMode::PaneBacked,
            CompletionManifest {
                provider: name.to_string(),
                runtime_mode: "pane-backed".to_string(),
                poll_interval_ms: 500,
                timeout_ms: 30000,
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
}
