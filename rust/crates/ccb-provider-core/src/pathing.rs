use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{ProviderCoreError, Result};

/// Default session filenames by provider.
pub fn provider_session_filenames() -> HashMap<String, String> {
    [
        ("codex", ".codex-session"),
        ("claude", ".claude-session"),
        ("gemini", ".gemini-session"),
        ("opencode", ".opencode-session"),
        ("droid", ".droid-session"),
        ("agy", ".agy-session"),
        ("qwen", ".qwen-session"),
        ("copilot", ".copilot-session"),
        ("codebuddy", ".codebuddy-session"),
        ("cursor", ".cursor-session"),
        ("crush", ".crush-session"),
        ("kiro", ".kiro-session"),
        ("pi", ".pi-session"),
        ("kimi", ".kimi-session"),
        ("deepseek", ".deepseek-session"),
        ("mimo", ".mimo-session"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Build a per-instance session filename.
pub fn session_filename_for_instance(base_filename: &str, instance: Option<&str>) -> String {
    let base = base_filename.trim();
    let instance = instance.map(|s| s.trim()).filter(|s| !s.is_empty());
    match instance {
        None => base.to_string(),
        Some(inst) => {
            if let Some(prefix) = base.strip_suffix("-session") {
                format!("{}-{}-session", prefix, inst)
            } else {
                format!("{}-{}", base, inst)
            }
        }
    }
}

/// Look for a session file in a work directory.
pub fn find_session_file_for_work_dir(work_dir: &Path, session_filename: &str) -> Option<PathBuf> {
    let candidate = work_dir.expand_home().join(session_filename);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Build the session filename for an agent/provider pair.
pub fn session_filename_for_agent(provider: &str, agent_name: &str) -> Result<String> {
    let normalized_provider = provider.trim().to_lowercase();
    if normalized_provider.is_empty() {
        return Err(ProviderCoreError::EmptyProvider);
    }
    let base = provider_session_filenames()
        .get(&normalized_provider)
        .ok_or_else(|| ProviderCoreError::UnsupportedProvider(provider.to_string()))?
        .clone();
    let normalized_agent = agent_name.trim();
    if normalized_agent.is_empty() {
        Ok(base)
    } else {
        Ok(session_filename_for_instance(&base, Some(normalized_agent)))
    }
}

trait ExpandHome {
    fn expand_home(&self) -> PathBuf;
}

impl ExpandHome for Path {
    fn expand_home(&self) -> PathBuf {
        if let Some(std::path::Component::Normal(seg)) = self.components().next() {
            if seg == "~" {
                if let Ok(home) = std::env::var("HOME") {
                    let rest: PathBuf = self.components().skip(1).collect();
                    return PathBuf::from(home).join(rest);
                }
            }
        }
        self.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_filename_for_instance() {
        assert_eq!(
            session_filename_for_instance(".claude-session", None),
            ".claude-session"
        );
        assert_eq!(
            session_filename_for_instance(".claude-session", Some("reviewer")),
            ".claude-reviewer-session"
        );
        assert_eq!(
            session_filename_for_instance("session", Some("reviewer")),
            "session-reviewer"
        );
    }

    #[test]
    fn test_session_filename_for_agent() {
        assert_eq!(
            session_filename_for_agent("claude", "").unwrap(),
            ".claude-session"
        );
        assert_eq!(
            session_filename_for_agent("claude", "reviewer").unwrap(),
            ".claude-reviewer-session"
        );
        assert!(session_filename_for_agent("unknown", "").is_err());
    }

    #[test]
    fn test_find_session_file_for_work_dir() {
        let tmp = std::env::temp_dir();
        let found = find_session_file_for_work_dir(&tmp, ".definitely-missing-session");
        assert!(found.is_none());
    }
}
