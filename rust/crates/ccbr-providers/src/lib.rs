pub mod claude;
pub mod codex;
pub mod deepseek;
pub mod execution;
pub mod kimi;
pub mod mimo;
pub mod model_shortcuts;
pub mod native_cli_support;
pub mod opencode;
pub mod pane_log_support;
pub mod pane_quiet_support;
pub mod providers;
pub mod runtime;

pub const TEST_DOUBLE_PROVIDER_NAMES: &[&str] = providers::fake::TEST_DOUBLE_PROVIDER_NAMES;

/// Build a registry containing the default provider execution adapters.
pub fn build_default_execution_registry() -> execution::ProviderExecutionRegistry {
    let mut registry = execution::ProviderExecutionRegistry::new();
    registry.register(Box::new(providers::claude::ClaudeExecutionAdapter));
    registry.register(Box::new(providers::codex::CodexExecutionAdapter));
    registry.register(Box::new(providers::gemini::GeminiExecutionAdapter));
    registry.register(Box::new(providers::opencode::OpenCodeExecutionAdapter));
    registry.register(Box::new(providers::droid::DroidExecutionAdapter));
    registry.register(Box::new(providers::agy::AgyExecutionAdapter));
    registry.register(Box::new(providers::copilot::build_execution_adapter()));
    registry.register(Box::new(providers::codebuddy::build_execution_adapter()));
    registry.register(Box::new(providers::qwen::build_execution_adapter()));
    registry.register(Box::new(providers::kimi::KimiExecutionAdapter));
    registry.register(Box::new(providers::deepseek::DeepSeekExecutionAdapter));
    registry.register(Box::new(providers::mimo::MimoExecutionAdapter));
    registry.register(Box::new(providers::cursor::build_execution_adapter()));
    registry.register(Box::new(providers::crush::build_execution_adapter()));
    registry.register(Box::new(providers::kiro::build_execution_adapter()));
    registry.register(Box::new(providers::pi::build_execution_adapter()));
    for adapter in providers::fake::execution_adapters() {
        registry.register(Box::new(adapter));
    }
    registry
}

/// Build a registry of provider backends (manifests only or with minimal adapters).
pub fn build_default_backend_registry() -> ccbr_provider_core::registry::ProviderBackendRegistry {
    let mut registry = ccbr_provider_core::registry::ProviderBackendRegistry::new();
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
    registry.register(providers::deepseek::backend());
    registry.register(providers::mimo::backend());
    registry.register(providers::cursor::backend());
    registry.register(providers::crush::backend());
    registry.register(providers::kiro::backend());
    registry.register(providers::pi::backend());
    for backend in providers::fake::backends() {
        registry.register(backend);
    }
    registry
}

pub mod active_runtime;

pub mod helper_cleanup;
pub mod helper_manifest;

pub mod session_authority;

pub mod session_paths;

pub mod workspace_preparation;

pub mod droid;
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
        assert!(registry.get("deepseek").is_some());
        assert!(registry.get("mimo").is_some());
        assert!(registry.get("cursor").is_some());
        assert!(registry.get("crush").is_some());
        assert!(registry.get("kiro").is_some());
        assert!(registry.get("pi").is_some());
        for name in providers::fake::TEST_DOUBLE_PROVIDER_NAMES {
            assert!(
                registry.get(name).is_some(),
                "execution registry missing {name}"
            );
        }
    }

    #[test]
    fn test_default_backend_registry() {
        let registry = build_default_backend_registry();
        assert!(registry.get("claude").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("qwen").is_some());
        assert!(registry.get("kimi").is_some());
        assert!(registry.get("deepseek").is_some());
        assert!(registry.get("mimo").is_some());
        assert!(registry.get("cursor").is_some());
        assert!(registry.get("crush").is_some());
        assert!(registry.get("kiro").is_some());
        assert!(registry.get("pi").is_some());
        for name in providers::fake::TEST_DOUBLE_PROVIDER_NAMES {
            assert!(
                registry.get(name).is_some(),
                "backend registry missing {name}"
            );
        }
    }
}
