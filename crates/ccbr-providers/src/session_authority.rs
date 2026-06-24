//! Mirrors Python `lib/provider_backends/codex/session_authority.py`.

use std::collections::HashMap;
use std::path::Path;

use camino::Utf8Path;
use ccbr_provider_profiles::codex_home_config::codex_provider_authority_fingerprint;
use ccbr_provider_profiles::models::ProviderProfileSpec;
use serde_json::Value;

const MEMORY_PROJECTION_MARKER: &str = "codex-memory-projection.json";

/// Return the current provider authority fingerprint for a profile.
pub fn current_provider_authority_fingerprint(profile: Option<&ProviderProfileSpec>) -> String {
    normalized_fingerprint(codex_provider_authority_fingerprint(profile).as_deref())
}

/// Read the memory projection fingerprint from the runtime marker file.
pub fn current_memory_projection_fingerprint(runtime_dir: Option<&Utf8Path>) -> String {
    let runtime_dir = match runtime_dir {
        Some(d) => d,
        None => return String::new(),
    };
    let marker_path = runtime_dir.join(MEMORY_PROJECTION_MARKER);
    let text = match std::fs::read_to_string(&marker_path) {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let data: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let sha256 = data.get("sha256").and_then(|v| v.as_str());
    normalized_fingerprint(sha256)
}

/// Provider authority fingerprint stored in a session payload.
pub fn stored_provider_authority_fingerprint(data: &HashMap<String, Value>) -> String {
    normalized_fingerprint(
        data.get("codex_provider_authority_fingerprint")
            .and_then(|v| v.as_str()),
    )
}

/// Session authority fingerprint stored in a session payload.
pub fn stored_session_authority_fingerprint(data: &HashMap<String, Value>) -> String {
    normalized_fingerprint(
        data.get("codex_session_authority_fingerprint")
            .and_then(|v| v.as_str()),
    )
}

/// Memory projection fingerprint stored in a session payload.
pub fn stored_memory_projection_fingerprint(data: &HashMap<String, Value>) -> String {
    normalized_fingerprint(
        data.get("codex_memory_projection_sha256")
            .and_then(|v| v.as_str()),
    )
}

/// Decide whether a stored session payload is still authoritative enough to resume.
pub fn resume_authority_matches(
    data: &HashMap<String, Value>,
    profile: Option<&ProviderProfileSpec>,
    current_fingerprint: Option<&str>,
    _current_memory_fingerprint: Option<&str>,
) -> bool {
    let current = if let Some(fp) = current_fingerprint {
        normalized_fingerprint(Some(fp))
    } else {
        current_provider_authority_fingerprint(profile)
    };
    if stored_provider_authority_fingerprint(data) != current {
        return false;
    }
    if !has_resume_candidate(data) {
        return true;
    }
    let stored_binding = stored_session_authority_fingerprint(data);
    if !stored_binding.is_empty() {
        return stored_binding == current;
    }
    current.is_empty()
}

/// Mark the session payload with the current provider authority fingerprint.
pub fn remember_bound_session_authority(data: &mut HashMap<String, Value>) {
    let current = stored_provider_authority_fingerprint(data);
    if !current.is_empty() {
        data.insert(
            "codex_session_authority_fingerprint".to_string(),
            current.into(),
        );
    } else {
        data.remove("codex_session_authority_fingerprint");
    }
}

/// Does the session payload contain something that could be resumed?
pub fn has_resume_candidate(data: &HashMap<String, Value>) -> bool {
    if let Some(id) = data.get("codex_session_id").and_then(|v| v.as_str()) {
        if !id.trim().is_empty() {
            return true;
        }
    }
    for key in ["codex_start_cmd", "start_cmd"] {
        if let Some(cmd) = data.get(key).and_then(|v| v.as_str()) {
            if extract_resume_session_id(cmd).is_some() {
                return true;
            }
        }
    }
    false
}

/// Extract a Codex session id from a start command string.
///
/// Mirrors Python `extract_resume_session_id`.
pub fn extract_resume_session_id(command: &str) -> Option<String> {
    let raw = command.trim();
    if raw.is_empty() {
        return None;
    }
    regex_resume_session_id(raw).or_else(|| token_resume_session_id(raw))
}

fn regex_resume_session_id(raw: &str) -> Option<String> {
    let re = regex::Regex::new(r"\bcodex\b(?:[^;\n]*?)\bresume\s+(?P<session>[^\s;]+)").ok()?;
    re.captures(raw)
        .and_then(|c| c.name("session"))
        .map(|m| normalized_session_id(m.as_str()))
}

fn token_resume_session_id(raw: &str) -> Option<String> {
    let tokens = match shell_split(raw) {
        Some(t) if !t.is_empty() => t,
        _ => return None,
    };
    for (index, token) in tokens.iter().enumerate() {
        if token == "resume" {
            return tokens.get(index + 1).map(|s| normalized_session_id(s));
        }
    }
    None
}

fn shell_split(raw: &str) -> Option<Vec<String>> {
    match shlex::split(raw) {
        Some(parts) if !parts.is_empty() => Some(parts),
        _ => None,
    }
}

fn normalized_session_id(value: &str) -> String {
    value.trim().to_string()
}

fn normalized_fingerprint(value: Option<&str>) -> String {
    value.unwrap_or("").trim().to_string()
}

fn find_codex_token_index(tokens: &[String]) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate() {
        if Path::new(token)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "codex")
            .unwrap_or(false)
            || token == "codex"
        {
            return Some(index);
        }
    }
    None
}

/// Does the command look like a bare `codex resume <id>` command?
pub fn looks_like_bare_resume_cmd(command: &str) -> bool {
    let raw = command.trim();
    if raw.is_empty() || raw.contains(';') || raw.contains("CODEX_") || raw.contains(" export ") {
        return false;
    }
    let tokens = match shell_split(raw) {
        Some(t) => t,
        None => return false,
    };
    tokens_form_bare_resume(&tokens)
}

fn tokens_form_bare_resume(tokens: &[String]) -> bool {
    if tokens.len() < 3 {
        return false;
    }
    let codex_index = match find_codex_token_index(tokens) {
        Some(i) => i,
        None => return false,
    };
    codex_index + 2 == tokens.len() - 1 && tokens[codex_index + 1] == "resume"
}

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
    fn test_extract_resume_session_id_from_codex_resume() {
        assert_eq!(
            extract_resume_session_id("codex resume agent1-session-id"),
            Some("agent1-session-id".to_string())
        );
        assert_eq!(
            extract_resume_session_id("codex -c disable_paste_burst=true resume agent1-session-id"),
            Some("agent1-session-id".to_string())
        );
        assert_eq!(extract_resume_session_id("codex"), None);
    }

    #[test]
    fn test_current_memory_projection_fingerprint_reads_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = camino::Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        std::fs::write(
            runtime.join(MEMORY_PROJECTION_MARKER),
            r#"{"sha256":"abc123"}"#,
        )
        .unwrap();
        assert_eq!(
            current_memory_projection_fingerprint(Some(&runtime)),
            "abc123"
        );
    }

    #[test]
    fn test_resume_authority_matches_requires_provider_fingerprint() {
        let mut data = HashMap::new();
        data.insert("codex_session_id".to_string(), "s1".into());
        data.insert(
            "codex_provider_authority_fingerprint".to_string(),
            "fp1".into(),
        );
        // A non-empty provider authority without a bound session authority is
        // not resumable until it has been explicitly bound.
        assert!(!resume_authority_matches(&data, None, Some("fp1"), None));
        assert!(!resume_authority_matches(&data, None, Some("fp2"), None));
    }

    #[test]
    fn test_resume_authority_matches_allows_unbound_session_with_empty_authority() {
        let mut data = HashMap::new();
        data.insert("codex_session_id".to_string(), "s1".into());
        assert!(resume_authority_matches(&data, None, Some(""), None));
        assert!(!resume_authority_matches(&data, None, Some("fp1"), None));
    }
}
