use crate::error::{ProviderCoreError, Result};

/// Providers that support model shortcuts in their CLI.
pub fn supported_provider_model_shortcuts() -> Vec<&'static str> {
    vec!["codex", "claude", "gemini", "opencode"]
}

/// Return the model flag tokens recognized for a provider.
pub fn provider_model_flag_tokens(provider: &str) -> Vec<&'static str> {
    let normalized = provider.trim().to_lowercase();
    match normalized.as_str() {
        "codex" | "gemini" | "opencode" => vec!["-m", "--model"],
        "claude" => vec!["--model"],
        _ => Vec::new(),
    }
}

/// Build the startup argument prefix for a provider/model pair.
pub fn provider_model_startup_args(provider: &str, model: &str) -> Result<Vec<String>> {
    let normalized = provider.trim().to_lowercase();
    let flag = match normalized.as_str() {
        "codex" | "gemini" | "opencode" => "-m",
        "claude" => "--model",
        _ => {
            let supported = supported_provider_model_shortcuts().join(", ");
            return Err(ProviderCoreError::UnsupportedProvider(format!(
                "model shortcut is supported only for providers: {supported}"
            )));
        }
    };
    let resolved_model = model.trim();
    if resolved_model.is_empty() {
        return Err(ProviderCoreError::EmptyModel);
    }
    Ok(vec![flag.to_string(), resolved_model.to_string()])
}

/// Check whether a startup argument list already contains a model flag.
pub fn startup_args_contain_model_flag(provider: &str, startup_args: &[String]) -> bool {
    let flags: std::collections::HashSet<&str> =
        provider_model_flag_tokens(provider).into_iter().collect();
    if flags.is_empty() {
        return false;
    }
    for arg in startup_args {
        if flags.contains(arg.as_str()) || arg.starts_with("--model=") {
            return true;
        }
    }
    false
}

/// Strip the leading model flag prefix from startup args if it matches.
pub fn strip_provider_model_startup_args(
    provider: &str,
    startup_args: &[String],
    model: &str,
) -> Vec<String> {
    let Ok(prefix) = provider_model_startup_args(provider, model) else {
        return startup_args.to_vec();
    };
    let normalized: Vec<String> = startup_args.iter().map(|a| a.to_string()).collect();
    if normalized.starts_with(&prefix) {
        normalized[prefix.len()..].to_vec()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_provider_model_shortcuts() {
        let shortcuts = supported_provider_model_shortcuts();
        assert!(shortcuts.contains(&"claude"));
        assert!(shortcuts.contains(&"codex"));
    }

    #[test]
    fn test_provider_model_flag_tokens() {
        assert_eq!(provider_model_flag_tokens("claude"), vec!["--model"]);
        assert_eq!(provider_model_flag_tokens("codex"), vec!["-m", "--model"]);
        assert!(provider_model_flag_tokens("droid").is_empty());
    }

    #[test]
    fn test_provider_model_startup_args() {
        assert_eq!(
            provider_model_startup_args("claude", "sonnet-4").unwrap(),
            vec!["--model", "sonnet-4"]
        );
        assert_eq!(
            provider_model_startup_args("codex", "gpt-4").unwrap(),
            vec!["-m", "gpt-4"]
        );
        assert!(provider_model_startup_args("droid", "x").is_err());
        assert!(provider_model_startup_args("claude", "").is_err());
    }

    #[test]
    fn test_startup_args_contain_model_flag() {
        assert!(startup_args_contain_model_flag(
            "claude",
            &["--model".to_string(), "x".to_string()]
        ));
        assert!(startup_args_contain_model_flag(
            "claude",
            &["--model=x".to_string()]
        ));
        assert!(!startup_args_contain_model_flag(
            "claude",
            &["-m".to_string(), "x".to_string()]
        ));
        assert!(!startup_args_contain_model_flag(
            "droid",
            &["--model".to_string()]
        ));
    }

    #[test]
    fn test_strip_provider_model_startup_args() {
        assert_eq!(
            strip_provider_model_startup_args(
                "claude",
                &[
                    "--model".to_string(),
                    "sonnet-4".to_string(),
                    "--quiet".to_string()
                ],
                "sonnet-4"
            ),
            vec!["--quiet"]
        );
        assert_eq!(
            strip_provider_model_startup_args("claude", &["--quiet".to_string()], "sonnet-4"),
            vec!["--quiet"]
        );
    }
}
