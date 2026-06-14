//! Subprocess adapter for the external `agent-roles` CLI.
//!
//! Mirrors Python `rolepacks.agent_roles_manager`. The install/update/sync
//! operations delegate to an external `agent-roles` command, resolved in this
//! precedence order:
//!   1. `AGENT_ROLES_CLI` env (shlex-split)
//!   2. `agent-roles` binary on `PATH`
//!   3. `python -m agent_roles` if the `agent_roles` module is importable
//!   4. `python -m agent_roles` with `PYTHONPATH` pointing at a located source root
//!
//! Each command is invoked with a trailing `--json` and its stdout parsed as a
//! JSON object. Non-zero exits surface the embedded `error`/`status` field.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::rolepacks::default_agent_roles_source;

const DEFAULT_TIMEOUT: f64 = 120.0;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Resolved command vector, optional working directory, and environment.
type CommandContext = (Vec<String>, Option<PathBuf>, HashMap<String, String>);

/// Install a role via the external `agent-roles` CLI.
///
/// Mirrors `agent_roles_manager.install(role_id, source_path=...)`.
pub fn install(
    role_id: Option<&str>,
    source_path: Option<&Path>,
) -> crate::Result<Map<String, Value>> {
    let mut args: Vec<String> = vec!["install".to_string()];
    if let Some(id) = role_id {
        args.push(id.to_string());
    }
    if let Some(path) = source_path {
        args.push("--path".to_string());
        args.push(expand_user(path).to_string_lossy().to_string());
    }
    run_json(&args)
}

/// Update a role via the external `agent-roles` CLI.
///
/// Mirrors `agent_roles_manager.update(role_id)`.
pub fn update(role_id: &str) -> crate::Result<Map<String, Value>> {
    if role_id.trim().is_empty() {
        return Err(crate::AgentError::Role(
            "role id is required for update".to_string(),
        ));
    }
    run_json(&["update".to_string(), role_id.to_string()])
}

/// Sync roles from a source path via the external `agent-roles` CLI.
///
/// Mirrors `agent_roles_manager.sync(path)`.
pub fn sync(path: &Path) -> crate::Result<Map<String, Value>> {
    run_json(&[
        "sync".to_string(),
        expand_user(path).to_string_lossy().to_string(),
    ])
}

/// Run the `agent-roles` CLI with `--json` and parse the object payload.
fn run_json(args: &[String]) -> crate::Result<Map<String, Value>> {
    let (mut command, cwd, env) = command_context()?;
    command.extend_from_slice(args);
    command.push("--json".to_string());

    let timeout = Duration::from_secs_f64(timeout_seconds());
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .envs(env.iter())
        .current_dir(match &cwd {
            Some(path) => path.as_path(),
            None => Path::new("."),
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|exc| crate::AgentError::Role(format!("agent-roles could not run: {exc}")))?;

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|exc| crate::AgentError::Role(format!("agent-roles wait failed: {exc}")))?
        {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(crate::AgentError::Role(format!(
                "agent-roles timed out after {:.1}s for {}",
                timeout.as_secs_f64(),
                args.join(" ")
            )));
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    let stdout_text = drain(child.stdout.take());
    let stderr_text = drain(child.stderr.take());

    if !status.success() {
        let code = status.code().unwrap_or(-1);
        for source in [&stdout_text, &stderr_text] {
            if source.trim().is_empty() {
                continue;
            }
            if let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(source.trim()) {
                let detail = payload
                    .get("error")
                    .or_else(|| payload.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent-roles failed")
                    .to_string();
                return Err(crate::AgentError::Role(detail));
            }
        }
        let detail = if !stderr_text.trim().is_empty() {
            stderr_text.clone()
        } else if !stdout_text.trim().is_empty() {
            stdout_text.clone()
        } else {
            "no output".to_string()
        };
        return Err(crate::AgentError::Role(format!(
            "agent-roles {} failed with exit code {}: {}",
            args.join(" "),
            code,
            detail.trim()
        )));
    }

    let payload = serde_json::from_str::<Value>(stdout_text.trim()).map_err(|_| {
        crate::AgentError::Role(format!(
            "agent-roles returned invalid JSON for {}: {}",
            args.join(" "),
            if stdout_text.trim().is_empty() {
                "no output"
            } else {
                stdout_text.trim()
            }
        ))
    })?;
    match payload {
        Value::Object(map) => Ok(map),
        _ => Err(crate::AgentError::Role(format!(
            "agent-roles returned non-object JSON for {}",
            args.join(" ")
        ))),
    }
}

/// Resolve the command vector, optional working directory, and environment.
///
/// Mirrors `agent_roles_manager._command_context`.
fn command_context() -> crate::Result<CommandContext> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    let raw = env.get("AGENT_ROLES_CLI").cloned().unwrap_or_default();
    let raw = raw.trim();
    if !raw.is_empty() {
        let parts = shell_words::split(raw)
            .map_err(|exc| crate::AgentError::Role(format!("invalid AGENT_ROLES_CLI: {exc}")))?;
        return Ok((parts, None, env));
    }
    if let Some(executable) = which("agent-roles") {
        return Ok((vec![executable.to_string_lossy().to_string()], None, env));
    }
    if python_can_import_agent_roles(&env) {
        let python = preferred_python();
        return Ok((
            vec![python, "-m".to_string(), "agent_roles".to_string()],
            None,
            env,
        ));
    }
    if let Some(source_root) = agent_roles_source_root() {
        let pythonpath = source_root.to_string_lossy().to_string();
        let existing = env.get("PYTHONPATH").cloned().unwrap_or_default();
        let joined = if existing.trim().is_empty() {
            pythonpath
        } else {
            format!("{pythonpath}{}{existing}", path_sep())
        };
        env.insert("PYTHONPATH".to_string(), joined);
        let python = preferred_python();
        return Ok((
            vec![python, "-m".to_string(), "agent_roles".to_string()],
            Some(source_root),
            env,
        ));
    }
    Err(crate::AgentError::Role(
        "agent-roles manager is enabled but no agent-roles command was found; set AGENT_ROLES_CLI or AGENT_ROLES_SPEC_HOME".to_string(),
    ))
}

/// Locate the `agent-roles-spec` source root containing `agent_roles/cli.py`.
///
/// Mirrors `agent_roles_manager._agent_roles_source_root`.
fn agent_roles_source_root() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for env_name in ["AGENT_ROLES_SPEC_HOME", "CCB_AGENT_ROLES_SPEC_HOME"] {
        if let Ok(value) = std::env::var(env_name) {
            let value = value.trim();
            if !value.is_empty() {
                candidates.push(expand_user(Path::new(&value)));
            }
        }
    }
    if let Some(home) = home_dir() {
        candidates.push(home.join("yunwei").join("agent-roles-spec"));
    }
    if let Some(default_source) = default_agent_roles_source(false) {
        candidates.push(default_source);
    }
    for candidate in candidates {
        if candidate.join("agent_roles").join("cli.py").is_file() {
            return Some(canonicalize(&candidate).unwrap_or(candidate));
        }
    }
    None
}

/// Resolve the configured timeout in seconds.
///
/// Mirrors `agent_roles_manager._timeout_seconds`.
fn timeout_seconds() -> f64 {
    let raw = std::env::var("CCB_AGENT_ROLES_TIMEOUT_SECONDS").unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return DEFAULT_TIMEOUT;
    }
    match raw.parse::<f64>() {
        Ok(value) => value.max(1.0),
        Err(_) => DEFAULT_TIMEOUT,
    }
}

/// Probe whether `agent_roles` is importable by the current Python.
///
/// Mirrors `importlib.util.find_spec('agent_roles') is not None`.
fn python_can_import_agent_roles(env: &HashMap<String, String>) -> bool {
    let python = preferred_python();
    let status = Command::new(&python)
        .args(["-c", "import agent_roles"])
        .envs(env.iter())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

fn preferred_python() -> String {
    for candidate in ["python3", "python"] {
        if which(candidate).is_some() {
            return candidate.to_string();
        }
    }
    "python3".to_string()
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if let Ok(meta) = std::fs::metadata(&candidate) {
            if meta.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Drain a child pipe into a string.
fn drain<T: Read>(mut pipe: Option<T>) -> String {
    let mut buf = String::new();
    if let Some(ref mut p) = pipe {
        let _ = p.read_to_string(&mut buf);
    }
    buf
}

fn expand_user(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix('~') {
        if let Some(home) = home_dir() {
            let stripped = rest.strip_prefix('/').unwrap_or(rest);
            return home.join(stripped);
        }
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn canonicalize(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

fn path_sep() -> String {
    std::path::MAIN_SEPARATOR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_requires_role_id() {
        let result = update("");
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("role id is required for update"));
    }

    #[test]
    fn test_timeout_seconds_default() {
        std::env::remove_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS");
        assert!((timeout_seconds() - DEFAULT_TIMEOUT).abs() < f64::EPSILON);
    }

    #[test]
    fn test_timeout_seconds_parsed() {
        std::env::set_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS", "30");
        assert!((timeout_seconds() - 30.0).abs() < f64::EPSILON);
        std::env::remove_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_timeout_seconds_clamps_to_minimum() {
        std::env::set_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS", "0.1");
        assert!((timeout_seconds() - 1.0).abs() < f64::EPSILON);
        std::env::remove_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_timeout_seconds_invalid_falls_back() {
        std::env::set_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS", "not-a-number");
        assert!((timeout_seconds() - DEFAULT_TIMEOUT).abs() < f64::EPSILON);
        std::env::remove_var("CCB_AGENT_ROLES_TIMEOUT_SECONDS");
    }

    #[test]
    fn test_expand_user_with_home() {
        std::env::set_var("HOME", "/tmp/fake-home");
        let expanded = expand_user(Path::new("~/roles"));
        assert_eq!(expanded, PathBuf::from("/tmp/fake-home/roles"));
    }

    #[test]
    fn test_expand_user_without_tilde() {
        std::env::set_var("HOME", "/tmp/fake-home");
        let expanded = expand_user(Path::new("/absolute/path"));
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }
}
