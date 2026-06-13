use std::collections::HashMap;

use ccb_agents::models::{AgentSpec, RuntimeMode};

use crate::error::{CompletionError, Result};
use crate::models::{CompletionFamily, CompletionProfile, CompletionSourceKind, SelectorFamily};
use crate::utils::runtime_mode_to_string;

/// Manifest describing how completion should behave for a provider/runtime pair.
/// Mirrors Python `completion.profiles.CompletionManifest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionManifest {
    pub provider: String,
    pub runtime_mode: String,
    pub completion_family: CompletionFamily,
    pub completion_source_kind: CompletionSourceKind,
    pub supports_exact_completion: bool,
    pub supports_observed_completion: bool,
    pub supports_anchor_binding: bool,
    pub supports_reply_stability: bool,
    pub supports_terminal_reason: bool,
    pub selector_family: SelectorFamily,
}

impl CompletionManifest {
    pub fn new(provider: impl Into<String>, runtime_mode: impl Into<String>) -> Result<Self> {
        let provider = provider.into();
        if provider.trim().is_empty() {
            return Err(CompletionError::Validation(
                "provider cannot be empty".into(),
            ));
        }
        let runtime_mode = runtime_mode.into();
        if runtime_mode.trim().is_empty() {
            return Err(CompletionError::Validation(
                "runtime_mode cannot be empty".into(),
            ));
        }
        Ok(Self {
            provider,
            runtime_mode,
            completion_family: CompletionFamily::ProtocolTurn,
            completion_source_kind: CompletionSourceKind::ProtocolEventStream,
            supports_exact_completion: false,
            supports_observed_completion: false,
            supports_anchor_binding: false,
            supports_reply_stability: false,
            supports_terminal_reason: false,
            selector_family: SelectorFamily::FinalMessage,
        })
    }

    pub fn with_completion_family(mut self, family: CompletionFamily) -> Self {
        self.completion_family = family;
        self
    }

    pub fn with_completion_source_kind(mut self, kind: CompletionSourceKind) -> Self {
        self.completion_source_kind = kind;
        self
    }

    pub fn with_selector_family(mut self, family: SelectorFamily) -> Self {
        self.selector_family = family;
        self
    }

    pub fn with_supports_exact_completion(mut self, value: bool) -> Self {
        self.supports_exact_completion = value;
        self
    }

    pub fn with_supports_observed_completion(mut self, value: bool) -> Self {
        self.supports_observed_completion = value;
        self
    }

    pub fn with_supports_anchor_binding(mut self, value: bool) -> Self {
        self.supports_anchor_binding = value;
        self
    }

    pub fn with_supports_reply_stability(mut self, value: bool) -> Self {
        self.supports_reply_stability = value;
        self
    }

    pub fn with_supports_terminal_reason(mut self, value: bool) -> Self {
        self.supports_terminal_reason = value;
        self
    }
}

/// Build a `CompletionProfile` from an agent spec and provider manifest.
pub fn build_completion_profile(
    agent_spec: &AgentSpec,
    manifest: &CompletionManifest,
) -> Result<CompletionProfile> {
    if agent_spec.provider != manifest.provider {
        return Err(CompletionError::Validation(format!(
            "agent provider {:?} does not match manifest provider {:?}",
            agent_spec.provider, manifest.provider
        )));
    }
    if runtime_mode_to_string(&agent_spec.runtime_mode) != manifest.runtime_mode {
        return Err(CompletionError::Validation(format!(
            "agent runtime_mode {:?} does not match manifest runtime_mode {:?}",
            agent_spec.runtime_mode, manifest.runtime_mode
        )));
    }
    Ok(CompletionProfile {
        provider: manifest.provider.clone(),
        runtime_mode: agent_spec.runtime_mode,
        completion_family: manifest.completion_family,
        completion_source_kind: manifest.completion_source_kind,
        supports_exact_completion: manifest.supports_exact_completion,
        supports_observed_completion: manifest.supports_observed_completion,
        supports_anchor_binding: manifest.supports_anchor_binding,
        supports_reply_stability: manifest.supports_reply_stability,
        supports_terminal_reason: manifest.supports_terminal_reason,
        selector_family: manifest.selector_family,
    })
}

/// Resolves a `CompletionManifest` for a provider/runtime pair.
pub trait CompletionManifestResolver {
    fn resolve_completion_manifest(
        &self,
        provider: &str,
        runtime_mode: &RuntimeMode,
    ) -> Result<CompletionManifest>;
}

impl CompletionManifestResolver for HashMap<(String, String), CompletionManifest> {
    fn resolve_completion_manifest(
        &self,
        provider: &str,
        runtime_mode: &RuntimeMode,
    ) -> Result<CompletionManifest> {
        let key = (
            provider.trim().to_lowercase(),
            runtime_mode_to_string(runtime_mode),
        );
        self.get(&key).cloned().ok_or_else(|| {
            CompletionError::Validation(format!(
                "no completion manifest for provider {provider:?} runtime_mode {runtime_mode:?}"
            ))
        })
    }
}

impl CompletionManifestResolver for ccb_provider_core::catalog::ProviderCatalog {
    fn resolve_completion_manifest(
        &self,
        provider: &str,
        runtime_mode: &RuntimeMode,
    ) -> Result<CompletionManifest> {
        use ccb_provider_core::manifest::RuntimeMode as CoreRuntimeMode;

        let core_mode = match runtime_mode {
            RuntimeMode::PaneBacked => CoreRuntimeMode::PaneBacked,
            RuntimeMode::PtyBacked => CoreRuntimeMode::PtyBacked,
            RuntimeMode::Headless => CoreRuntimeMode::Headless,
        };
        let manifest = self.resolve_completion_manifest(provider, &core_mode)?;
        Ok(CompletionManifest {
            provider: manifest.provider.clone(),
            runtime_mode: manifest.runtime_mode.clone(),
            completion_family: CompletionFamily::ProtocolTurn,
            completion_source_kind: CompletionSourceKind::ProtocolEventStream,
            supports_exact_completion: false,
            supports_observed_completion: false,
            supports_anchor_binding: false,
            supports_reply_stability: false,
            supports_terminal_reason: false,
            selector_family: SelectorFamily::FinalMessage,
        })
    }
}
