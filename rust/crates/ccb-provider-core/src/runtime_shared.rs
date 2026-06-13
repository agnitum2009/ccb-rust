use std::collections::HashMap;
use std::env;

use crate::error::{ProviderCoreError, Result};

const PROVIDER_START_ENV_VARS: &[(&str, &str)] = &[
    ("codex", "CODEX_START_CMD"),
    ("claude", "CLAUDE_START_CMD"),
    ("gemini", "GEMINI_START_CMD"),
    ("opencode", "OPENCODE_START_CMD"),
    ("droid", "DROID_START_CMD"),
    ("agy", "AGY_START_CMD"),
];

/// Placeholder token used in provider command templates.
pub const PROVIDER_COMMAND_PLACEHOLDER: &str = "{command}";

/// Return the start command parts for a provider.
///
/// First checks `<PROVIDER>_START_CMD`, otherwise falls back to the provider
/// name as the executable.
pub fn provider_start_parts(provider: &str) -> Vec<String> {
    let normalized = provider.trim().to_lowercase();
    let env_map: HashMap<&str, &str> = PROVIDER_START_ENV_VARS.iter().copied().collect();
    if let Some(env_name) = env_map.get(normalized.as_str()) {
        if let Ok(raw) = env::var(env_name) {
            let raw = raw.trim();
            if !raw.is_empty() {
                return shell_split(raw);
            }
        }
    }
    vec![normalized]
}

/// Apply a wrapper template to a command.
pub fn apply_provider_command_template(command: &str, template: Option<&str>) -> Result<String> {
    let template = template.unwrap_or("").trim();
    if template.is_empty() {
        return Ok(command.to_string());
    }
    if template.matches(PROVIDER_COMMAND_PLACEHOLDER).count() != 1 {
        return Err(ProviderCoreError::InvalidCommandTemplate);
    }
    Ok(template.replace(PROVIDER_COMMAND_PLACEHOLDER, command.trim()))
}

/// Return the executable name for a provider.
pub fn provider_executable(provider: &str) -> String {
    let parts = provider_start_parts(provider);
    parts
        .first()
        .cloned()
        .unwrap_or_else(|| provider.to_string())
}

/// Build a tmux pane title marker.
pub fn pane_title_marker(project_id: &str, agent_name: &str) -> String {
    let suffix = project_id.trim();
    let suffix = if suffix.is_empty() {
        String::new()
    } else {
        format!("-{}", &suffix[..suffix.len().min(8)])
    };
    format!("CCB-{}{}", agent_name, suffix)
}

fn shell_split(raw: &str) -> Vec<String> {
    match shlex::split(raw) {
        Some(parts) if !parts.is_empty() => parts,
        _ => vec![raw.to_string()],
    }
}

// Minimal in-tree `shlex::split` to avoid adding a dependency.
mod shlex {
    pub fn split(input: &str) -> Option<Vec<String>> {
        let mut result = Vec::new();
        let mut current = String::new();
        let mut in_single = false;
        let mut in_double = false;
        let mut escape = false;
        let mut has_word = false;
        for ch in input.chars() {
            if escape {
                current.push(ch);
                escape = false;
                has_word = true;
                continue;
            }
            if ch == '\\' && !in_single {
                escape = true;
                has_word = true;
                continue;
            }
            if ch == '\'' && !in_double {
                in_single = !in_single;
                has_word = true;
                continue;
            }
            if ch == '"' && !in_single {
                in_double = !in_double;
                has_word = true;
                continue;
            }
            if ch.is_whitespace() && !in_single && !in_double {
                if has_word {
                    result.push(current.clone());
                    current.clear();
                    has_word = false;
                }
                continue;
            }
            current.push(ch);
            has_word = true;
        }
        if in_single || in_double || escape {
            return None;
        }
        if has_word {
            result.push(current);
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_start_parts_default() {
        assert_eq!(provider_start_parts("claude"), vec!["claude"]);
        assert_eq!(provider_start_parts("  CODEX  "), vec!["codex"]);
    }

    #[test]
    fn test_apply_provider_command_template() {
        assert_eq!(
            apply_provider_command_template("claude", None).unwrap(),
            "claude"
        );
        assert_eq!(
            apply_provider_command_template("claude", Some("tmux new-window {command}")).unwrap(),
            "tmux new-window claude"
        );
        assert!(apply_provider_command_template("claude", Some("no placeholder")).is_err());
        assert!(apply_provider_command_template("claude", Some("{command} {command}")).is_err());
    }

    #[test]
    fn test_provider_executable() {
        assert_eq!(provider_executable("claude"), "claude");
    }

    #[test]
    fn test_pane_title_marker() {
        assert_eq!(pane_title_marker("proj123", "claude"), "CCB-claude-proj123");
        assert_eq!(pane_title_marker("", "claude"), "CCB-claude");
        assert_eq!(
            pane_title_marker("verylongid", "claude"),
            "CCB-claude-verylong"
        );
    }

    #[test]
    fn test_shell_split() {
        assert_eq!(shlex::split("a b c").unwrap(), vec!["a", "b", "c"]);
        assert_eq!(shlex::split("'a b' c").unwrap(), vec!["a b", "c"]);
        assert_eq!(shlex::split("\"a b\" c").unwrap(), vec!["a b", "c"]);
        assert!(shlex::split("'a b").is_none());
    }
}
