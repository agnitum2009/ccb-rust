use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Runtime modes for provider execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMode {
    PaneBacked,
    PtyBacked,
    Headless,
}

/// Completion family mirrors Python `completion.models.CompletionFamily`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompletionFamily {
    #[default]
    ProtocolTurn,
    AnchoredSessionStability,
    SessionBoundary,
    StructuredResult,
    TerminalTextQuiet,
}

/// Completion source kind mirrors Python `completion.models.CompletionSourceKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompletionSourceKind {
    #[default]
    ProtocolEventStream,
    SessionEventLog,
    SessionSnapshot,
}

/// Selector family mirrors Python `completion.models.SelectorFamily`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelectorFamily {
    #[default]
    FinalMessage,
}

/// Completion profile for a specific runtime mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionManifest {
    pub provider: String,
    pub runtime_mode: String,
    #[serde(default)]
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub timeout_ms: u64,
    #[serde(default)]
    pub completion_family: CompletionFamily,
    #[serde(default)]
    pub completion_source_kind: CompletionSourceKind,
    #[serde(default)]
    pub supports_exact_completion: bool,
    #[serde(default)]
    pub supports_observed_completion: bool,
    #[serde(default)]
    pub supports_anchor_binding: bool,
    #[serde(default)]
    pub supports_reply_stability: bool,
    #[serde(default)]
    pub supports_terminal_reason: bool,
    #[serde(default)]
    pub selector_family: SelectorFamily,
}

/// Provider manifest: declares capabilities and runtime profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderManifest {
    pub provider: String,
    pub supports_resume: bool,
    pub supports_permission_auto: bool,
    pub supports_stream_watch: bool,
    pub supports_subagents: bool,
    pub supports_workspace_attach: bool,
    pub runtime_profiles: HashMap<RuntimeMode, CompletionManifest>,
}

impl ProviderManifest {
    pub fn new(
        provider: impl Into<String>,
        supports_resume: bool,
        supports_permission_auto: bool,
        supports_stream_watch: bool,
        supports_subagents: bool,
        supports_workspace_attach: bool,
        runtime_profiles: HashMap<RuntimeMode, CompletionManifest>,
    ) -> Self {
        let provider = provider.into().trim().to_lowercase();
        assert!(!provider.is_empty(), "provider cannot be empty");
        assert!(
            !runtime_profiles.is_empty(),
            "runtime_profiles cannot be empty"
        );
        for (mode, profile) in &runtime_profiles {
            assert_eq!(
                profile.provider, provider,
                "runtime profile provider {} does not match manifest provider {}",
                profile.provider, provider
            );
            let expected_mode = serde_json::to_string(mode)
                .ok()
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();
            assert_eq!(
                profile.runtime_mode, expected_mode,
                "runtime profile mode {} does not match runtime key {:?}",
                profile.runtime_mode, mode
            );
        }
        Self {
            provider,
            supports_resume,
            supports_permission_auto,
            supports_stream_watch,
            supports_subagents,
            supports_workspace_attach,
            runtime_profiles,
        }
    }

    pub fn supports_runtime_mode(&self, mode: &RuntimeMode) -> bool {
        self.runtime_profiles.contains_key(mode)
    }

    pub fn completion_manifest_for(&self, mode: &RuntimeMode) -> Option<&CompletionManifest> {
        self.runtime_profiles.get(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> ProviderManifest {
        let mut profiles = HashMap::new();
        profiles.insert(
            RuntimeMode::PaneBacked,
            CompletionManifest {
                provider: "test".into(),
                runtime_mode: "pane-backed".into(),
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
        ProviderManifest::new("test", true, false, false, false, false, profiles)
    }

    #[test]
    fn test_manifest_supports_mode() {
        let m = test_manifest();
        assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
        assert!(!m.supports_runtime_mode(&RuntimeMode::Headless));
    }

    #[test]
    fn test_manifest_completion_for() {
        let m = test_manifest();
        let c = m.completion_manifest_for(&RuntimeMode::PaneBacked).unwrap();
        assert_eq!(c.poll_interval_ms, 500);
    }

    #[test]
    #[should_panic(expected = "provider cannot be empty")]
    fn test_empty_provider_panics() {
        ProviderManifest::new("", true, false, false, false, false, HashMap::new());
    }
}
