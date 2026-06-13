pub mod droid;
pub mod execution;
pub mod opencode;
pub mod providers;
pub mod runtime;

/// Build a registry containing the default provider execution adapters.
pub fn build_default_execution_registry() -> execution::ProviderExecutionRegistry {
    let mut registry = execution::ProviderExecutionRegistry::new();
    registry.register(Box::new(providers::claude::ClaudeExecutionAdapter));
    registry.register(Box::new(providers::codex::CodexExecutionAdapter));
    registry.register(Box::new(providers::gemini::GeminiExecutionAdapter));
    registry.register(Box::new(providers::opencode::OpenCodeExecutionAdapter));
    registry.register(Box::new(providers::droid::DroidExecutionAdapter));
    registry.register(Box::new(providers::agy::AgyExecutionAdapter));
    registry.register(Box::new(providers::copilot::CopilotExecutionAdapter));
    registry.register(Box::new(providers::codebuddy::CodeBuddyExecutionAdapter));
    registry.register(Box::new(providers::qwen::QwenExecutionAdapter));
    registry.register(Box::new(providers::kimi::KimiExecutionAdapter));
    registry
}

/// Build a registry of provider backends (manifests only or with minimal adapters).
pub fn build_default_backend_registry() -> ccb_provider_core::registry::ProviderBackendRegistry {
    let mut registry = ccb_provider_core::registry::ProviderBackendRegistry::new();
    registry.register(providers::claude::backend());
    registry.register(providers::codex::backend());
    registry.register(providers::gemini::backend());
    registry.register(providers::opencode::backend());
    registry.register(providers::droid::backend());
    registry.register(providers::agy::backend());
    registry.register(providers::copilot::backend());
    registry.register(providers::codebuddy::backend());
    registry.register(providers::qwen::backend());
    registry.register(providers::kimi::backend());
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_execution_registry() {
        let registry = build_default_execution_registry();
        assert!(registry.get("claude").is_some());
        assert!(registry.get("codex").is_some());
        assert!(registry.get("gemini").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("droid").is_some());
        assert!(registry.get("agy").is_some());
        assert!(registry.get("copilot").is_some());
        assert!(registry.get("codebuddy").is_some());
        assert!(registry.get("qwen").is_some());
        assert!(registry.get("kimi").is_some());
    }

    #[test]
    fn test_default_backend_registry() {
        let registry = build_default_backend_registry();
        assert!(registry.get("claude").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("qwen").is_some());
        assert!(registry.get("kimi").is_some());
    }
}
