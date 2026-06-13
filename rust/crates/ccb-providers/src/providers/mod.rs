pub mod agy;
pub mod claude;
pub mod codebuddy;
pub mod codex;
pub mod copilot;
pub mod droid;
pub mod gemini;
pub mod kimi;
pub mod opencode;
pub mod qwen;

use ccb_provider_core::contracts::ProviderBackend;
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use std::collections::HashMap;

/// Build the default pane-backed completion manifest for a provider.
pub fn pane_backed_manifest(provider: &str, supports_resume: bool) -> ProviderManifest {
    let provider = provider.trim().to_lowercase();
    let mut profiles = HashMap::new();
    profiles.insert(
        RuntimeMode::PaneBacked,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "pane-backed".to_string(),
            poll_interval_ms: 500,
            timeout_ms: 300_000,
        },
    );
    ProviderManifest::new(
        provider,
        supports_resume,
        false,
        false,
        false,
        false,
        profiles,
    )
}

/// Build a backend with only a manifest (no adapter/binding/launcher).
pub fn manifest_only_backend(provider: &str, supports_resume: bool) -> ProviderBackend {
    ProviderBackend {
        manifest: pane_backed_manifest(provider, supports_resume),
        execution_adapter: None,
        session_binding: None,
        runtime_launcher: None,
    }
}
