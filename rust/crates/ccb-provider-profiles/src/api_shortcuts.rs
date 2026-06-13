use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
struct ApiShortcutMapping {
    key_var: &'static str,
    url_var: &'static str,
}

const PROVIDER_API_SHORTCUTS: &[(&str, ApiShortcutMapping)] = &[
    (
        "codex",
        ApiShortcutMapping {
            key_var: "OPENAI_API_KEY",
            url_var: "OPENAI_BASE_URL",
        },
    ),
    (
        "claude",
        ApiShortcutMapping {
            key_var: "ANTHROPIC_API_KEY",
            url_var: "ANTHROPIC_BASE_URL",
        },
    ),
    (
        "gemini",
        ApiShortcutMapping {
            key_var: "GEMINI_API_KEY",
            url_var: "GOOGLE_GEMINI_BASE_URL",
        },
    ),
];

/// Return env vars for a supported provider API shortcut.
///
/// Mirrors Python `provider_profiles.api_shortcuts.provider_api_shortcut_env`.
pub fn provider_api_shortcut_env(
    provider: &str,
    key: Option<&str>,
    url: Option<&str>,
) -> crate::Result<HashMap<String, String>> {
    let normalized_provider = provider.trim().to_lowercase();
    let mapping = PROVIDER_API_SHORTCUTS
        .iter()
        .find(|(name, _)| *name == normalized_provider)
        .map(|(_, m)| *m)
        .ok_or_else(|| {
            let supported: Vec<_> = supported_provider_api_shortcuts();
            crate::ProfilesError::Validation(format!(
                "api shortcut is supported only for providers: {}",
                supported.join(", ")
            ))
        })?;

    let mut env = HashMap::new();
    if let Some(k) = key {
        let trimmed = k.trim();
        if !trimmed.is_empty() {
            env.insert(mapping.key_var.into(), trimmed.into());
        }
    }
    if let Some(u) = url {
        let trimmed = u.trim();
        if !trimmed.is_empty() {
            env.insert(
                mapping.url_var.into(),
                normalize_shortcut_url(&normalized_provider, trimmed),
            );
        }
    }
    Ok(env)
}

fn normalize_shortcut_url(provider: &str, url: &str) -> String {
    if provider != "codex" {
        return url.into();
    }
    let parsed = match url::Url::parse(url) {
        Ok(p) => p,
        Err(_) => return url.into(),
    };
    if parsed.scheme().is_empty() || parsed.host().is_none() {
        return url.into();
    }
    let path = parsed.path();
    let new_path = if path.is_empty() || path == "/" || path == "/v1/" {
        "/v1"
    } else {
        path
    };
    let mut fixed = parsed.clone();
    fixed.set_path(new_path);
    fixed.to_string()
}

/// Return the sorted list of providers that support API shortcuts.
pub fn supported_provider_api_shortcuts() -> Vec<&'static str> {
    let mut names: Vec<_> = PROVIDER_API_SHORTCUTS
        .iter()
        .map(|(name, _)| *name)
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_api_shortcut_env_codex() {
        let env =
            provider_api_shortcut_env("codex", Some("my-key"), Some("https://api.example.test/"))
                .unwrap();
        assert_eq!(env.get("OPENAI_API_KEY"), Some(&"my-key".to_string()));
        assert_eq!(
            env.get("OPENAI_BASE_URL"),
            Some(&"https://api.example.test/v1".to_string())
        );
    }

    #[test]
    fn test_provider_api_shortcut_env_claude() {
        let env = provider_api_shortcut_env(
            "claude",
            Some("claude-key"),
            Some("https://claude.example.test"),
        )
        .unwrap();
        assert_eq!(
            env.get("ANTHROPIC_API_KEY"),
            Some(&"claude-key".to_string())
        );
        assert_eq!(
            env.get("ANTHROPIC_BASE_URL"),
            Some(&"https://claude.example.test".to_string())
        );
    }

    #[test]
    fn test_provider_api_shortcut_env_rejects_empty_values() {
        let env = provider_api_shortcut_env("codex", Some(""), Some("   ")).unwrap();
        assert!(env.is_empty());
    }

    #[test]
    fn test_provider_api_shortcut_env_rejects_unsupported_provider() {
        assert!(provider_api_shortcut_env("unknown", Some("k"), None).is_err());
    }

    #[test]
    fn test_supported_provider_api_shortcuts_sorted() {
        let supported = supported_provider_api_shortcuts();
        assert_eq!(supported, vec!["claude", "codex", "gemini"]);
    }
}
