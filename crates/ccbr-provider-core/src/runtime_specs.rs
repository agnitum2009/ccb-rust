use std::collections::HashMap;
use std::sync::LazyLock;

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

/// Build a `CCBR_<PROVIDER>_<SUFFIX>` environment variable name.
///
/// Empty parts are ignored. When no parts are supplied the base
/// `CCBR_<PROVIDER>` name is returned.
pub fn provider_env_name(provider_key: &str, parts: &[&str]) -> String {
    let suffix: Vec<String> = parts
        .iter()
        .map(|p| p.trim().to_uppercase())
        .filter(|p| !p.is_empty())
        .collect();
    let base = format!("CCBR_{}", env_stem(provider_key));
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
        idle_timeout_env: format!("CCBR_{}_RUNTIME_IDLE_TIMEOUT_S", stem),
        lock_name: format!("{}-runtime", provider_key),
    }
}

fn build_client_spec(provider_key: &str, session_filename: &str) -> ProviderClientSpec {
    let stem = env_stem(provider_key);
    ProviderClientSpec {
        provider_key: provider_key.to_string(),
        enabled_env: format!("CCBR_{}", stem),
        autostart_env: format!("CCBR_{}_AUTOSTART", stem),
        state_file_env: format!("CCBR_{}_STATE_FILE", stem),
        session_filename: session_filename.to_string(),
    }
}

macro_rules! define_runtime_spec {
    ($name:ident, $key:literal) => {
        /// Runtime spec constant.
        pub static $name: LazyLock<ProviderRuntimeSpec> =
            LazyLock::new(|| build_runtime_spec($key));
    };
}

macro_rules! define_client_spec {
    ($name:ident, $key:literal, $filename:literal) => {
        /// Client spec constant.
        pub static $name: LazyLock<ProviderClientSpec> =
            LazyLock::new(|| build_client_spec($key, $filename));
    };
}

/// Runtime spec for Codex.
pub fn codex_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("codex")
}
define_runtime_spec!(CODEX_RUNTIME_SPEC, "codex");

/// Runtime spec for Gemini.
pub fn gemini_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("gemini")
}
define_runtime_spec!(GEMINI_RUNTIME_SPEC, "gemini");

/// Runtime spec for OpenCode.
pub fn opencode_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("opencode")
}
define_runtime_spec!(OPENCODE_RUNTIME_SPEC, "opencode");

/// Runtime spec for Claude.
pub fn claude_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("claude")
}
define_runtime_spec!(CLAUDE_RUNTIME_SPEC, "claude");

/// Runtime spec for Droid.
pub fn droid_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("droid")
}
define_runtime_spec!(DROID_RUNTIME_SPEC, "droid");

/// Runtime spec for AGY.
pub fn agy_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("agy")
}
define_runtime_spec!(AGY_RUNTIME_SPEC, "agy");

/// Runtime spec for Kimi.
pub fn kimi_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("kimi")
}
define_runtime_spec!(KIMI_RUNTIME_SPEC, "kimi");

/// Runtime spec for DeepSeek.
pub fn deepseek_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("deepseek")
}
define_runtime_spec!(DEEPSEEK_RUNTIME_SPEC, "deepseek");

/// Runtime spec for Mimo.
pub fn mimo_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("mimo")
}
define_runtime_spec!(MIMO_RUNTIME_SPEC, "mimo");

/// Runtime spec for Copilot.
pub fn copilot_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("copilot")
}
define_runtime_spec!(COPILOT_RUNTIME_SPEC, "copilot");

/// Runtime spec for Codebuddy.
pub fn codebuddy_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("codebuddy")
}
define_runtime_spec!(CODEBUDDY_RUNTIME_SPEC, "codebuddy");

/// Runtime spec for Qwen.
pub fn qwen_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("qwen")
}
define_runtime_spec!(QWEN_RUNTIME_SPEC, "qwen");

/// Runtime spec for Cursor.
pub fn cursor_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("cursor")
}
define_runtime_spec!(CURSOR_RUNTIME_SPEC, "cursor");

/// Runtime spec for Crush.
pub fn crush_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("crush")
}
define_runtime_spec!(CRUSH_RUNTIME_SPEC, "crush");

/// Runtime spec for Kiro.
pub fn kiro_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("kiro")
}
define_runtime_spec!(KIRO_RUNTIME_SPEC, "kiro");

/// Runtime spec for Pi.
pub fn pi_runtime_spec() -> ProviderRuntimeSpec {
    build_runtime_spec("pi")
}
define_runtime_spec!(PI_RUNTIME_SPEC, "pi");

/// Client spec for Codex.
pub fn codex_client_spec() -> ProviderClientSpec {
    build_client_spec("codex", ".codex-session")
}
define_client_spec!(CODEX_CLIENT_SPEC, "codex", ".codex-session");

/// Client spec for Gemini.
pub fn gemini_client_spec() -> ProviderClientSpec {
    build_client_spec("gemini", ".gemini-session")
}
define_client_spec!(GEMINI_CLIENT_SPEC, "gemini", ".gemini-session");

/// Client spec for OpenCode.
pub fn opencode_client_spec() -> ProviderClientSpec {
    build_client_spec("opencode", ".opencode-session")
}
define_client_spec!(OPENCODE_CLIENT_SPEC, "opencode", ".opencode-session");

/// Client spec for Claude.
pub fn claude_client_spec() -> ProviderClientSpec {
    build_client_spec("claude", ".claude-session")
}
define_client_spec!(CLAUDE_CLIENT_SPEC, "claude", ".claude-session");

/// Client spec for Droid.
pub fn droid_client_spec() -> ProviderClientSpec {
    build_client_spec("droid", ".droid-session")
}
define_client_spec!(DROID_CLIENT_SPEC, "droid", ".droid-session");

/// Client spec for AGY.
pub fn agy_client_spec() -> ProviderClientSpec {
    build_client_spec("agy", ".agy-session")
}
define_client_spec!(AGY_CLIENT_SPEC, "agy", ".agy-session");

/// Client spec for Kimi.
pub fn kimi_client_spec() -> ProviderClientSpec {
    build_client_spec("kimi", ".kimi-session")
}
define_client_spec!(KIMI_CLIENT_SPEC, "kimi", ".kimi-session");

/// Client spec for DeepSeek.
pub fn deepseek_client_spec() -> ProviderClientSpec {
    build_client_spec("deepseek", ".deepseek-session")
}
define_client_spec!(DEEPSEEK_CLIENT_SPEC, "deepseek", ".deepseek-session");

/// Client spec for Mimo.
pub fn mimo_client_spec() -> ProviderClientSpec {
    build_client_spec("mimo", ".mimo-session")
}
define_client_spec!(MIMO_CLIENT_SPEC, "mimo", ".mimo-session");

/// Client spec for Copilot.
pub fn copilot_client_spec() -> ProviderClientSpec {
    build_client_spec("copilot", ".copilot-session")
}
define_client_spec!(COPILOT_CLIENT_SPEC, "copilot", ".copilot-session");

/// Client spec for Codebuddy.
pub fn codebuddy_client_spec() -> ProviderClientSpec {
    build_client_spec("codebuddy", ".codebuddy-session")
}
define_client_spec!(CODEBUDDY_CLIENT_SPEC, "codebuddy", ".codebuddy-session");

/// Client spec for Qwen.
pub fn qwen_client_spec() -> ProviderClientSpec {
    build_client_spec("qwen", ".qwen-session")
}
define_client_spec!(QWEN_CLIENT_SPEC, "qwen", ".qwen-session");

/// Client spec for Cursor.
pub fn cursor_client_spec() -> ProviderClientSpec {
    build_client_spec("cursor", ".cursor-session")
}
define_client_spec!(CURSOR_CLIENT_SPEC, "cursor", ".cursor-session");

/// Client spec for Crush.
pub fn crush_client_spec() -> ProviderClientSpec {
    build_client_spec("crush", ".crush-session")
}
define_client_spec!(CRUSH_CLIENT_SPEC, "crush", ".crush-session");

/// Client spec for Kiro.
pub fn kiro_client_spec() -> ProviderClientSpec {
    build_client_spec("kiro", ".kiro-session")
}
define_client_spec!(KIRO_CLIENT_SPEC, "kiro", ".kiro-session");

/// Client spec for Pi.
pub fn pi_client_spec() -> ProviderClientSpec {
    build_client_spec("pi", ".pi-session")
}
define_client_spec!(PI_CLIENT_SPEC, "pi", ".pi-session");

/// All runtime specs keyed by provider name.
pub static RUNTIME_SPECS_BY_PROVIDER: LazyLock<HashMap<&'static str, ProviderRuntimeSpec>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("codex", CODEX_RUNTIME_SPEC.clone());
        map.insert("gemini", GEMINI_RUNTIME_SPEC.clone());
        map.insert("opencode", OPENCODE_RUNTIME_SPEC.clone());
        map.insert("claude", CLAUDE_RUNTIME_SPEC.clone());
        map.insert("droid", DROID_RUNTIME_SPEC.clone());
        map.insert("agy", AGY_RUNTIME_SPEC.clone());
        map.insert("kimi", KIMI_RUNTIME_SPEC.clone());
        map.insert("deepseek", DEEPSEEK_RUNTIME_SPEC.clone());
        map.insert("mimo", MIMO_RUNTIME_SPEC.clone());
        map.insert("copilot", COPILOT_RUNTIME_SPEC.clone());
        map.insert("codebuddy", CODEBUDDY_RUNTIME_SPEC.clone());
        map.insert("qwen", QWEN_RUNTIME_SPEC.clone());
        map.insert("cursor", CURSOR_RUNTIME_SPEC.clone());
        map.insert("crush", CRUSH_RUNTIME_SPEC.clone());
        map.insert("kiro", KIRO_RUNTIME_SPEC.clone());
        map.insert("pi", PI_RUNTIME_SPEC.clone());
        map
    });

/// All client specs keyed by provider name.
pub static CLIENT_SPECS_BY_PROVIDER: LazyLock<HashMap<&'static str, ProviderClientSpec>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("codex", CODEX_CLIENT_SPEC.clone());
        map.insert("gemini", GEMINI_CLIENT_SPEC.clone());
        map.insert("opencode", OPENCODE_CLIENT_SPEC.clone());
        map.insert("claude", CLAUDE_CLIENT_SPEC.clone());
        map.insert("droid", DROID_CLIENT_SPEC.clone());
        map.insert("agy", AGY_CLIENT_SPEC.clone());
        map.insert("kimi", KIMI_CLIENT_SPEC.clone());
        map.insert("deepseek", DEEPSEEK_CLIENT_SPEC.clone());
        map.insert("mimo", MIMO_CLIENT_SPEC.clone());
        map.insert("copilot", COPILOT_CLIENT_SPEC.clone());
        map.insert("codebuddy", CODEBUDDY_CLIENT_SPEC.clone());
        map.insert("qwen", QWEN_CLIENT_SPEC.clone());
        map.insert("cursor", CURSOR_CLIENT_SPEC.clone());
        map.insert("crush", CRUSH_CLIENT_SPEC.clone());
        map.insert("kiro", KIRO_CLIENT_SPEC.clone());
        map.insert("pi", PI_CLIENT_SPEC.clone());
        map
    });

/// Look up a runtime spec by provider key.
pub fn runtime_spec_by_provider(provider_key: &str) -> Option<ProviderRuntimeSpec> {
    let key = provider_key.trim().to_lowercase();
    RUNTIME_SPECS_BY_PROVIDER.get(key.as_str()).cloned()
}

/// Look up a client spec by provider key.
pub fn client_spec_by_provider(provider_key: &str) -> Option<ProviderClientSpec> {
    let key = provider_key.trim().to_lowercase();
    CLIENT_SPECS_BY_PROVIDER.get(key.as_str()).cloned()
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
        assert_eq!(provider_env_name("claude", &[]), "CCBR_CLAUDE");
        assert_eq!(
            provider_env_name("claude", &["autostart"]),
            "CCBR_CLAUDE_AUTOSTART"
        );
        assert_eq!(
            provider_env_name("open-code", &["runtime", "idle_timeout_s"]),
            "CCBR_OPEN_CODE_RUNTIME_IDLE_TIMEOUT_S"
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
        assert_eq!(spec.idle_timeout_env, "CCBR_CODEX_RUNTIME_IDLE_TIMEOUT_S");
    }

    #[test]
    fn test_client_spec_by_provider() {
        let spec = client_spec_by_provider("claude").unwrap();
        assert_eq!(spec.session_filename, ".claude-session");
        assert_eq!(spec.enabled_env, "CCBR_CLAUDE");
    }

    #[test]
    fn test_kimi_specs() {
        let runtime = runtime_spec_by_provider("kimi").unwrap();
        assert_eq!(runtime.provider_key, "kimi");
        assert_eq!(runtime.state_file_name, "kimi-runtime.json");
        assert_eq!(runtime.idle_timeout_env, "CCBR_KIMI_RUNTIME_IDLE_TIMEOUT_S");

        let client = client_spec_by_provider("kimi").unwrap();
        assert_eq!(client.session_filename, ".kimi-session");
        assert_eq!(client.enabled_env, "CCBR_KIMI");

        assert_eq!(KIMI_RUNTIME_SPEC.provider_key, "kimi");
        assert_eq!(KIMI_CLIENT_SPEC.provider_key, "kimi");
    }

    #[test]
    fn test_specs_by_provider_include_all_providers() {
        let runtime_keys: std::collections::HashSet<_> =
            RUNTIME_SPECS_BY_PROVIDER.keys().copied().collect();
        let client_keys: std::collections::HashSet<_> =
            CLIENT_SPECS_BY_PROVIDER.keys().copied().collect();
        assert!(runtime_keys.contains("kimi"));
        assert!(runtime_keys.contains("cursor"));
        assert!(client_keys.contains("kimi"));
        assert!(client_keys.contains("pi"));
        assert_eq!(runtime_keys.len(), 16);
        assert_eq!(client_keys.len(), 16);
    }
}
