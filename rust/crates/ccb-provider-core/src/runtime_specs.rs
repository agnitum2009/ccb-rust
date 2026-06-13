use serde::{Deserialize, Serialize};

/// Runtime metadata for a provider service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRuntimeSpec {
    pub provider_key: String,
    pub service_name: String,
    pub rpc_prefix: String,
    pub state_file_name: String,
    pub log_file_name: String,
    pub idle_timeout_env: String,
    pub lock_name: String,
}

/// Client-side metadata for a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderClientSpec {
    pub provider_key: String,
    pub enabled_env: String,
    pub autostart_env: String,
    pub state_file_env: String,
    pub session_filename: String,
}

fn env_stem(provider_key: &str) -> String {
    provider_key.trim().to_uppercase().replace('-', "_")
}

/// Build a `CCB_<PROVIDER>_<SUFFIX>` environment variable name.
///
/// Empty parts are ignored. When no parts are supplied the base
/// `CCB_<PROVIDER>` name is returned.
pub fn provider_env_name(provider_key: &str, parts: &[&str]) -> String {
    let suffix: Vec<String> = parts
        .iter()
        .map(|p| p.trim().to_uppercase())
        .filter(|p| !p.is_empty())
        .collect();
    let base = format!("CCB_{}", env_stem(provider_key));
    if suffix.is_empty() {
        base
    } else {
        format!("{}_{}", base, suffix.join("_"))
    }
}

/// Normalized marker prefix for a provider.
pub fn provider_marker_prefix(provider_key: &str) -> String {
    provider_key.trim().to_lowercase()
}

fn build_runtime_spec(provider_key: &str) -> ProviderRuntimeSpec {
    let stem = env_stem(provider_key);
    ProviderRuntimeSpec {
        provider_key: provider_key.to_string(),
        service_name: provider_key.to_string(),
        rpc_prefix: provider_key.to_string(),
        state_file_name: format!("{}-runtime.json", provider_key),
        log_file_name: format!("{}-runtime.log", provider_key),
        idle_timeout_env: format!("CCB_{}_RUNTIME_IDLE_TIMEOUT_S", stem),
        lock_name: format!("{}-runtime", provider_key),
    }
}

fn build_client_spec(provider_key: &str, session_filename: &str) -> ProviderClientSpec {
    let stem = env_stem(provider_key);
    ProviderClientSpec {
        provider_key: provider_key.to_string(),
        enabled_env: format!("CCB_{}", stem),
        autostart_env: format!("CCB_{}_AUTOSTART", stem),
        state_file_env: format!("CCB_{}_STATE_FILE", stem),
        session_filename: session_filename.to_string(),
    }
}

/// Runtime spec for Codex.
pub fn codex_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("codex")
}

/// Runtime spec for Gemini.
pub fn gemini_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("gemini")
}

/// Runtime spec for OpenCode.
pub fn opencode_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("opencode")
}

/// Runtime spec for Claude.
pub fn claude_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("claude")
}

/// Runtime spec for Droid.
pub fn droid_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("droid")
}

/// Runtime spec for AGY.
pub fn agy_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("agy")
}

/// Runtime spec for Copilot.
pub fn copilot_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("copilot")
}

/// Runtime spec for Codebuddy.
pub fn codebuddy_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("codebuddy")
}

/// Runtime spec for Qwen.
pub fn qwen_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("qwen")
}

/// Client spec for Codex.
pub fn codex_client_spec() -> ProviderClientSpec {
    build_client_spec("codex", ".codex-session")
}

/// Client spec for Gemini.
pub fn gemini_client_spec() -> ProviderClientSpec {
    build_client_spec("gemini", ".gemini-session")
}

/// Client spec for OpenCode.
pub fn opencode_client_spec() -> ProviderClientSpec {
    build_client_spec("opencode", ".opencode-session")
}

/// Client spec for Claude.
pub fn claude_client_spec() -> ProviderClientSpec {
    build_client_spec("claude", ".claude-session")
}

/// Client spec for Droid.
pub fn droid_client_spec() -> ProviderClientSpec {
    build_client_spec("droid", ".droid-session")
}

/// Client spec for AGY.
pub fn agy_client_spec() -> ProviderClientSpec {
    build_client_spec("agy", ".agy-session")
}

/// Client spec for Copilot.
pub fn copilot_client_spec() -> ProviderClientSpec {
    build_client_spec("copilot", ".copilot-session")
}

/// Client spec for Codebuddy.
pub fn codebuddy_client_spec() -> ProviderClientSpec {
    build_client_spec("codebuddy", ".codebuddy-session")
}

/// Client spec for Qwen.
pub fn qwen_client_spec() -> ProviderClientSpec {
    build_client_spec("qwen", ".qwen-session")
}

/// All runtime specs keyed by provider name.
pub fn runtime_specs_by_provider() -> Vec<(&'static str, ProviderRuntimeSpec)> {
    vec![
        ("codex", codex_runtime_spec()),
        ("gemini", gemini_runtime_spec()),
        ("opencode", opencode_runtime_spec()),
        ("claude", claude_runtime_spec()),
        ("droid", droid_runtime_spec()),
        ("agy", agy_runtime_spec()),
        ("copilot", copilot_runtime_spec()),
        ("codebuddy", codebuddy_runtime_spec()),
        ("qwen", qwen_runtime_spec()),
    ]
}

/// All client specs keyed by provider name.
pub fn client_specs_by_provider() -> Vec<(&'static str, ProviderClientSpec)> {
    vec![
        ("codex", codex_client_spec()),
        ("gemini", gemini_client_spec()),
        ("opencode", opencode_client_spec()),
        ("claude", claude_client_spec()),
        ("droid", droid_client_spec()),
        ("agy", agy_client_spec()),
        ("copilot", copilot_client_spec()),
        ("codebuddy", codebuddy_client_spec()),
        ("qwen", qwen_client_spec()),
    ]
}

/// Look up a runtime spec by provider key.
pub fn runtime_spec_by_provider(provider_key: &str) -> Option<ProviderRuntimeSpec> {
    let key = provider_key.trim().to_lowercase();
    runtime_specs_by_provider()
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, s)| s)
}

/// Look up a client spec by provider key.
pub fn client_spec_by_provider(provider_key: &str) -> Option<ProviderClientSpec> {
    let key = provider_key.trim().to_lowercase();
    client_specs_by_provider()
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, s)| s)
}

/// Split a qualified provider key such as `claude:reviewer` into its base
/// provider and optional instance.
pub fn parse_qualified_provider(key: &str) -> (String, Option<String>) {
    let key = key.trim().to_lowercase();
    if key.is_empty() {
        return (String::new(), None);
    }
    match key.split_once(':') {
        Some((base, instance)) => {
            let base = base.trim().to_string();
            let instance = instance.trim();
            if instance.is_empty() {
                (base, None)
            } else {
                (base, Some(instance.to_string()))
            }
        }
        None => (key, None),
    }
}

/// Build a qualified provider key from a base provider and optional instance.
pub fn make_qualified_key(base: &str, instance: Option<&str>) -> String {
    let base = base.trim().to_lowercase();
    match instance {
        Some(inst) => {
            let inst = inst.trim();
            if inst.is_empty() {
                base
            } else {
                format!("{}:{}", base, inst)
            }
        }
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_env_name() {
        assert_eq!(provider_env_name("claude", &[]), "CCB_CLAUDE");
        assert_eq!(
            provider_env_name("claude", &["autostart"]),
            "CCB_CLAUDE_AUTOSTART"
        );
        assert_eq!(
            provider_env_name("open-code", &["runtime", "idle_timeout_s"]),
            "CCB_OPEN_CODE_RUNTIME_IDLE_TIMEOUT_S"
        );
    }

    #[test]
    fn test_provider_marker_prefix() {
        assert_eq!(provider_marker_prefix("  Claude "), "claude");
    }

    #[test]
    fn test_parse_qualified_provider() {
        assert_eq!(
            parse_qualified_provider("claude:reviewer"),
            ("claude".to_string(), Some("reviewer".to_string()))
        );
        assert_eq!(
            parse_qualified_provider("  Codex  "),
            ("codex".to_string(), None)
        );
        assert_eq!(parse_qualified_provider(""), (String::new(), None));
        assert_eq!(
            parse_qualified_provider("claude:"),
            ("claude".to_string(), None)
        );
    }

    #[test]
    fn test_make_qualified_key() {
        assert_eq!(make_qualified_key("claude", None), "claude");
        assert_eq!(
            make_qualified_key("claude", Some("reviewer")),
            "claude:reviewer"
        );
        assert_eq!(make_qualified_key("  Claude  ", Some("  R  ")), "claude:R");
    }

    #[test]
    fn test_runtime_spec_by_provider() {
        let spec = runtime_spec_by_provider("codex").unwrap();
        assert_eq!(spec.provider_key, "codex");
        assert_eq!(spec.state_file_name, "codex-runtime.json");
        assert_eq!(spec.idle_timeout_env, "CCB_CODEX_RUNTIME_IDLE_TIMEOUT_S");
    }

    #[test]
    fn test_client_spec_by_provider() {
        let spec = client_spec_by_provider("claude").unwrap();
        assert_eq!(spec.session_filename, ".claude-session");
        assert_eq!(spec.enabled_env, "CCB_CLAUDE");
    }
}
