use std::fs;
use std::path::{Path, PathBuf};

use crate::files::{expand_user_path_str, find_project_session_file, ProviderClientSpec};
use ccbr_types::env::env_bool;

pub struct SessionResolver {
    manager: crate::files::SessionFileManager,
}

impl SessionResolver {
    pub fn new(manager: crate::files::SessionFileManager) -> Self {
        Self { manager }
    }

    pub fn resolve_latest(
        &self,
        agent_name: &str,
        provider: &str,
    ) -> Option<crate::files::SessionFile> {
        let sessions = self.manager.discover_sessions(agent_name);
        sessions
            .into_iter()
            .filter(|s| s.provider == provider)
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
    }

    pub fn resolve_by_id(
        &self,
        agent_name: &str,
        session_id: &str,
    ) -> Option<crate::files::SessionFile> {
        self.manager.load_session(agent_name, session_id)
    }

    pub fn is_session_usable(session: &crate::files::SessionFile) -> bool {
        session.pane_id.is_some() && session.start_cmd.is_some()
    }
}

/// Resolve the working directory from an explicit session file selection.
///
/// Mirrors Python `ccbd.client_runtime.resolution.resolve_work_dir`.
pub fn resolve_work_dir(
    spec: &ProviderClientSpec,
    cli_session_file: Option<&str>,
    env_session_file: Option<&str>,
    default_cwd: Option<&Path>,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let raw = selected_session_file(cli_session_file, env_session_file);
    if raw.is_none() {
        let cwd = default_cwd
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or("cannot determine current directory")?;
        return Ok((cwd, None));
    }
    let raw = raw.unwrap();
    let session_path = resolved_session_path(&raw)?;
    validate_session_path(spec, &session_path)?;
    let work_dir = work_dir_from_session_path(&session_path);
    Ok((work_dir, Some(session_path)))
}

/// Resolve the working directory, falling back to project discovery and the
/// legacy registry-only environment variable guard.
///
/// Mirrors Python `ccbd.client_runtime.resolution.resolve_work_dir_with_registry`.
pub fn resolve_work_dir_with_registry(
    spec: &ProviderClientSpec,
    provider: &str,
    cli_session_file: Option<&str>,
    env_session_file: Option<&str>,
    default_cwd: Option<&Path>,
    registry_only_env: &str,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    if selected_session_file(cli_session_file, env_session_file).is_some() {
        return resolve_work_dir(spec, cli_session_file, env_session_file, default_cwd);
    }

    let cwd = default_cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("cannot determine current directory")?;

    if let Some(found) = find_project_session_file(&cwd, &spec.session_filename) {
        return Ok((cwd, Some(found)));
    }

    if env_bool(registry_only_env, false) {
        return Err(format!(
            "{registry_only_env}=1 is no longer supported for provider={provider:?}; \
             use --session-file or run inside a .ccbr project"
        ));
    }

    Ok((cwd, None))
}

fn selected_session_file(cli: Option<&str>, env: Option<&str>) -> Option<String> {
    fn non_empty(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
    cli.and_then(non_empty).or_else(|| env.and_then(non_empty))
}

fn resolved_session_path(raw: &str) -> Result<PathBuf, String> {
    let expanded = expand_user_path_str(raw);
    let session_path = PathBuf::from(&expanded);

    if std::env::var("CLAUDECODE").as_deref() == Ok("1") && !session_path.is_absolute() {
        return Err(format!(
            "--session-file must be an absolute path in Claude Code (got: {raw})"
        ));
    }

    Ok(fs::canonicalize(&session_path).unwrap_or_else(|_| normalize_path(&session_path)))
}

fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.components().collect()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path).components().collect())
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn validate_session_path(spec: &ProviderClientSpec, session_path: &Path) -> Result<(), String> {
    let name = session_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name != spec.session_filename {
        return Err(format!(
            "Invalid session file for {}: expected filename {}, got {name}",
            spec.provider_key, spec.session_filename
        ));
    }
    if !session_path.exists() {
        return Err(format!(
            "Session file not found: {}",
            session_path.display()
        ));
    }
    if !session_path.is_file() {
        return Err(format!(
            "Session file must be a file: {}",
            session_path.display()
        ));
    }
    Ok(())
}

fn work_dir_from_session_path(session_path: &Path) -> PathBuf {
    let parent = session_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let is_ccbr_dir = session_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some(".ccbr");
    if is_ccbr_dir {
        parent.parent().map(PathBuf::from).unwrap_or(parent)
    } else {
        parent
    }
}
