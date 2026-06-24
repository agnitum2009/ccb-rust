use std::collections::HashMap;
use std::path::Path;

use ccbr_types::user_session::user_session_transport_env;

/// Build environment variables that identify the caller context.
pub fn caller_context_env(
    actor: &str,
    runtime_dir: &Path,
    launch_session_id: &str,
) -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("CCBR_CALLER_ACTOR".to_string(), actor.trim().to_string());
    env.insert(
        "CCBR_CALLER_RUNTIME_DIR".to_string(),
        runtime_dir.to_string_lossy().to_string(),
    );
    env.insert(
        "CCBR_SESSION_ID".to_string(),
        launch_session_id.trim().to_string(),
    );
    env
}

/// Build environment variables that should be forwarded from the user's
/// session into a provider runtime.
pub fn provider_user_session_env() -> HashMap<String, String> {
    user_session_transport_env(None)
}

/// Render a map of environment variables as a single `export K=V ...` shell
/// clause. Empty values are skipped.
pub fn export_env_clause(env_map: &HashMap<String, String>) -> String {
    let mut items: Vec<(&String, &String)> = env_map.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    let rendered: Vec<String> = items
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("{}={}", k, shell_quote(v)))
        .collect();
    if rendered.is_empty() {
        String::new()
    } else {
        format!("export {}", rendered.join(" "))
    }
}

/// Join non-empty shell clauses with `; `.
pub fn join_env_prefix(clauses: &[&str]) -> String {
    let parts: Vec<&str> = clauses
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect();
    parts.join("; ")
}

fn shell_quote(value: &str) -> String {
    // Mirrors Python's `shlex.quote`: single-quote the value and escape any
    // embedded single quotes as `'\''`.
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || "_-./:=@".contains(c))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caller_context_env() {
        let env = caller_context_env("claude", Path::new("/tmp/rt"), "sess-1");
        assert_eq!(env.get("CCBR_CALLER_ACTOR").unwrap(), "claude");
        assert_eq!(env.get("CCBR_CALLER_RUNTIME_DIR").unwrap(), "/tmp/rt");
        assert_eq!(env.get("CCBR_SESSION_ID").unwrap(), "sess-1");
    }

    #[test]
    fn test_export_env_clause_skips_empty_and_sorts() {
        let mut env = HashMap::new();
        env.insert("B".to_string(), "2".to_string());
        env.insert("A".to_string(), "".to_string());
        env.insert("C".to_string(), "hello world".to_string());
        assert_eq!(export_env_clause(&env), "export B=2 C='hello world'");
    }

    #[test]
    fn test_export_env_clause_empty() {
        let env: HashMap<String, String> = HashMap::new();
        assert_eq!(export_env_clause(&env), "");
    }

    #[test]
    fn test_join_env_prefix() {
        assert_eq!(join_env_prefix(&["a", "", "  ", "b"]), "a; b");
        assert_eq!(join_env_prefix(&[]), "");
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
    }
}
