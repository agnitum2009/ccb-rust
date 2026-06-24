use std::collections::HashMap;

/// Provider-specific model flag mappings.
static PROVIDER_MODEL_FLAGS: &[(&str, &[&str])] = &[
    ("codex", &["-m", "--model"]),
    ("claude", &["--model"]),
    ("gemini", &["-m", "--model"]),
    ("opencode", &["-m", "--model"]),
    ("mimo", &["--model"]),
    ("qwen", &["-m", "--model"]),
    ("kimi", &["-m", "--model"]),
    ("deepseek", &["-m", "--model"]),
];

/// Single startup flag per provider.
static PROVIDER_MODEL_STARTUP_FLAGS: &[(&str, &str)] = &[
    ("codex", "-m"),
    ("claude", "--model"),
    ("gemini", "-m"),
    ("opencode", "-m"),
    ("mimo", "--model"),
    ("qwen", "-m"),
    ("kimi", "-m"),
    ("deepseek", "-m"),
];

fn provider_model_flags_map() -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for (provider, flags) in PROVIDER_MODEL_FLAGS {
        map.insert(
            provider.to_string(),
            flags.iter().map(|s| s.to_string()).collect(),
        );
    }
    map
}

fn provider_startup_flags_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (provider, flag) in PROVIDER_MODEL_STARTUP_FLAGS {
        map.insert(provider.to_string(), flag.to_string());
    }
    map
}

pub fn supported_provider_model_shortcuts() -> Vec<String> {
    let mut providers: Vec<String> = PROVIDER_MODEL_STARTUP_FLAGS
        .iter()
        .map(|(provider, _)| provider.to_string())
        .collect();
    providers.sort();
    providers
}

pub fn provider_model_flag_tokens(provider: &str) -> Vec<String> {
    let normalized = provider.trim().to_lowercase();
    let flags = provider_model_flags_map();
    flags.get(&normalized).cloned().unwrap_or_default()
}

pub fn provider_model_startup_args(
    provider: &str,
    model: &str,
) -> Result<(String, String), String> {
    let normalized = provider.trim().to_lowercase();
    let startup_flags = provider_startup_flags_map();
    let flag = startup_flags.get(&normalized).ok_or_else(|| {
        let supported = supported_provider_model_shortcuts().join(", ");
        format!("model shortcut is supported only for providers: {supported}")
    })?;

    let resolved_model = model.trim();
    if resolved_model.is_empty() {
        return Err("model cannot be empty".to_string());
    }

    Ok((flag.clone(), resolved_model.to_string()))
}

pub fn startup_args_contain_model_flag(provider: &str, startup_args: &[String]) -> bool {
    let flags: Vec<String> = provider_model_flag_tokens(provider)
        .iter()
        .map(|s| s.to_string())
        .collect();

    if flags.is_empty() {
        return false;
    }

    let flags_set: std::collections::HashSet<&str> = flags.iter().map(|s| s.as_str()).collect();

    for arg in startup_args {
        let arg_str = arg.as_str();
        if flags_set.contains(arg_str) || arg_str.starts_with("--model=") {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_provider_model_shortcuts() {
        let providers = supported_provider_model_shortcuts();
        assert!(providers.contains(&"codex".to_string()));
        assert!(providers.contains(&"claude".to_string()));
        assert!(providers.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_provider_model_flag_tokens() {
        let flags = provider_model_flag_tokens("codex");
        assert_eq!(flags, vec!["-m", "--model"]);

        let flags = provider_model_flag_tokens("claude");
        assert_eq!(flags, vec!["--model"]);

        let flags = provider_model_flag_tokens("unknown");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_provider_model_startup_args() {
        let result = provider_model_startup_args("codex", "gpt-4");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ("-m".to_string(), "gpt-4".to_string()));

        let result = provider_model_startup_args("claude", "claude-3");
        assert_eq!(
            result.unwrap(),
            ("--model".to_string(), "claude-3".to_string())
        );
    }

    #[test]
    fn test_provider_model_startup_args_empty_model() {
        let result = provider_model_startup_args("codex", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("model cannot be empty"));
    }

    #[test]
    fn test_provider_model_startup_args_unsupported_provider() {
        let result = provider_model_startup_args("unknown", "gpt-4");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("supported only for providers"));
    }

    #[test]
    fn test_startup_args_contain_model_flag() {
        // Test -m flag (only supported by codex, gemini, opencode, etc.)
        let args = vec!["start".to_string(), "-m".to_string(), "gpt-4".to_string()];
        assert!(startup_args_contain_model_flag("codex", &args));
        assert!(!startup_args_contain_model_flag("claude", &args));

        // Test --model flag (supported by all)
        let args = vec![
            "start".to_string(),
            "--model".to_string(),
            "gpt-4".to_string(),
        ];
        assert!(startup_args_contain_model_flag("codex", &args));
        assert!(startup_args_contain_model_flag("claude", &args));

        // Test --model=value format
        let args = vec!["start".to_string(), "--model=gpt-4".to_string()];
        assert!(startup_args_contain_model_flag("codex", &args));
        assert!(startup_args_contain_model_flag("claude", &args));

        // Test no model flag
        let args = vec!["start".to_string()];
        assert!(!startup_args_contain_model_flag("codex", &args));
        assert!(!startup_args_contain_model_flag("claude", &args));
    }
}
