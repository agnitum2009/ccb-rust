//! Mirrors Python `lib/cli/ask_sender.py`.

use std::path::{Path, PathBuf};

use ccbr_agents::models::normalize_agent_name;
use ccbr_mailbox::targets::USER_ACTOR;

use crate::context::CliContext;

/// Resolve the sender actor for an `ask` command.
///
/// Mirrors Python `resolve_ask_sender`.
pub fn resolve_ask_sender(context: &CliContext, explicit_sender: Option<&str>) -> String {
    let sender = explicit_sender.unwrap_or("").trim();
    if !sender.is_empty() {
        return sender.to_string();
    }

    let allowed = allowed_session_actors(context);
    if let Some(actor) = _resolve_session_actor(context, &allowed) {
        return actor;
    }

    USER_ACTOR.to_string()
}

fn allowed_session_actors(context: &CliContext) -> Vec<String> {
    let config_result = ccbr_agents::config::load_project_config(&context.paths);
    match config_result {
        Ok(result) => result
            .config
            .agents
            .keys()
            .map(|name| normalize_agent_name(name).unwrap_or_else(|_| name.to_lowercase()))
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn _resolve_session_actor(context: &CliContext, allowed: &[String]) -> Option<String> {
    if let Some(actor) = std::env::var("CCB_CALLER_ACTOR")
        .ok()
        .and_then(|v| normalized_actor_candidate(&v))
    {
        if allowed.contains(&actor) {
            return Some(actor);
        }
    }

    for env_name in ["CCB_CALLER_RUNTIME_DIR", "CODEX_RUNTIME_DIR"] {
        if let Some(actor) = std::env::var(env_name).ok().and_then(|v| {
            _actor_from_runtime_dir(&v, context.paths.agents_dir().as_std_path(), allowed)
        }) {
            return Some(actor);
        }
    }

    std::env::var("CCB_SESSION_ID")
        .ok()
        .and_then(|v| _actor_from_session_id(&v, allowed))
}

fn _actor_from_runtime_dir(value: &str, agents_dir: &Path, allowed: &[String]) -> Option<String> {
    let runtime_dir = value.trim();
    if runtime_dir.is_empty() {
        return None;
    }
    let resolved_runtime_dir = _resolve_path(Path::new(runtime_dir));
    let resolved_agents_dir = _resolve_path(agents_dir);
    let relative = match resolved_runtime_dir.strip_prefix(&resolved_agents_dir) {
        Ok(r) => r,
        Err(_) => return None,
    };
    let first = relative.components().next()?;
    let candidate = normalized_actor_candidate(first.as_os_str().to_str()?)?;
    if allowed.contains(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

fn _actor_from_session_id(value: &str, allowed: &[String]) -> Option<String> {
    let session_id = value.trim().to_lowercase();
    if !session_id.starts_with("ccbr-") {
        return None;
    }
    let suffix = &session_id[4..];
    let mut matches: Vec<String> = allowed
        .iter()
        .filter(|actor| *actor == suffix || suffix.starts_with(&format!("{actor}-")))
        .cloned()
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|a| a.len());
    matches.pop()
}

fn normalized_actor_candidate(value: &str) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    Some(normalize_agent_name(text).unwrap_or_else(|_| text.to_lowercase()))
}

fn _resolve_path(path: &Path) -> PathBuf {
    let expanded = if let Some(rest) = path.to_string_lossy().strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home + rest)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };
    expanded
        .canonicalize()
        .unwrap_or_else(|_| expanded.absolute())
}

trait AbsolutePath {
    fn absolute(&self) -> PathBuf;
}

impl AbsolutePath for PathBuf {
    fn absolute(&self) -> PathBuf {
        if self.is_absolute() {
            self.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{CliContext, CliContextBuilder};
    use crate::models::ParsedCommand;

    /// Serialize tests that mutate process-global env vars. `std::env::set_var`
    /// is not thread-safe; without this the default parallel runner races and
    /// produces flaky pass/fail.
    static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn make_context(tmp: &tempfile::TempDir) -> CliContext {
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".ccbr")).unwrap();
        std::fs::write(root.join(".ccbr/ccbr.config"), "agent1:codex\n").unwrap();
        CliContextBuilder::new(ParsedCommand::Ask(
            crate::models_mailbox::ParsedAskCommand::new(
                None,
                "agent1".into(),
                None,
                "hello".into(),
            ),
        ))
        .cwd(root.to_path_buf())
        .build()
        .unwrap()
    }

    #[test]
    fn explicit_sender_is_returned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = make_context(&tmp);
        assert_eq!(resolve_ask_sender(&ctx, Some("foo")), "foo");
    }

    #[test]
    fn no_sender_defaults_to_user() {
        let _env_lock = ENV_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = make_context(&tmp);
        for name in [
            "CCB_CALLER_ACTOR",
            "CCB_CALLER_RUNTIME_DIR",
            "CODEX_RUNTIME_DIR",
            "CCB_SESSION_ID",
        ] {
            std::env::remove_var(name);
        }
        assert_eq!(resolve_ask_sender(&ctx, None), "user");
    }

    #[test]
    fn runtime_dir_actor_is_preferred() {
        let _env_lock = ENV_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = make_context(&tmp);
        for name in [
            "CCB_CALLER_ACTOR",
            "CCB_CALLER_RUNTIME_DIR",
            "CODEX_RUNTIME_DIR",
            "CCB_SESSION_ID",
        ] {
            std::env::remove_var(name);
        }
        let runtime_dir = tmp
            .path()
            .join(".ccbr/agents/agent1/provider-runtime/codex");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        std::env::set_var("CODEX_RUNTIME_DIR", runtime_dir.as_os_str());
        std::env::set_var("CCB_SESSION_ID", "legacy-session-without-actor");
        assert_eq!(resolve_ask_sender(&ctx, None), "agent1");
        std::env::remove_var("CODEX_RUNTIME_DIR");
    }
}
