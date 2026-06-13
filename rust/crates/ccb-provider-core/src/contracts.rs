use serde::{Deserialize, Serialize};

/// Runtime identity of a provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRuntimeIdentity {
    pub state: String,
    pub reason: Option<String>,
}

/// Binds a provider to its session management behavior.
/// In Python this used Callable fields; in Rust we use trait objects.
pub struct ProviderSessionBinding {
    pub provider: String,
    pub session_id_attr: String,
    pub session_path_attr: String,
}

impl ProviderSessionBinding {
    pub fn new(provider: impl Into<String>) -> Self {
        let provider = provider.into().trim().to_lowercase();
        assert!(!provider.is_empty(), "provider cannot be empty");
        Self {
            provider,
            session_id_attr: String::new(),
            session_path_attr: String::new(),
        }
    }
}

/// Launch mode for a provider runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    SimpleTmux,
    CodexTmux,
}

/// Describes how to launch a provider runtime in a tmux pane.
pub struct ProviderRuntimeLauncher {
    pub provider: String,
    pub launch_mode: LaunchMode,
}

impl ProviderRuntimeLauncher {
    pub fn new(provider: impl Into<String>, launch_mode: LaunchMode) -> Self {
        let provider = provider.into().trim().to_lowercase();
        assert!(!provider.is_empty(), "provider cannot be empty");
        Self {
            provider,
            launch_mode,
        }
    }
}

/// A complete provider backend registration: manifest + optional execution/session/launcher.
pub struct ProviderBackend {
    pub manifest: super::manifest::ProviderManifest,
    pub execution_adapter: Option<Box<dyn super::execution::ExecutionAdapter>>,
    pub session_binding: Option<ProviderSessionBinding>,
    pub runtime_launcher: Option<ProviderRuntimeLauncher>,
}

impl ProviderBackend {
    pub fn provider(&self) -> &str {
        &self.manifest.provider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_session_binding_normalizes() {
        let binding = ProviderSessionBinding::new("  CLAUDE  ");
        assert_eq!(binding.provider, "claude");
    }

    #[test]
    #[should_panic(expected = "provider cannot be empty")]
    fn test_provider_session_binding_empty_panics() {
        ProviderSessionBinding::new("");
    }

    #[test]
    fn test_launch_mode_serde() {
        let mode = LaunchMode::SimpleTmux;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"simple_tmux\"");
        let deserialized: LaunchMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, LaunchMode::SimpleTmux);
    }
}
