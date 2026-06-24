//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/history.py`.
//!
//! Locates the most recent Claude session history for a workspace so that a
//! launched pane can `--continue` the right conversation.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};

/// Locates Claude session history under a managed (or real) Claude home.
#[derive(Debug, Clone)]
pub struct ClaudeHistoryLocator<'a> {
    pub invocation_dir: &'a Utf8Path,
    pub project_root: &'a Utf8Path,
    pub env: &'a HashMap<String, String>,
    pub home_dir: &'a Utf8Path,
}

impl<'a> ClaudeHistoryLocator<'a> {
    pub fn new(
        invocation_dir: &'a Utf8Path,
        project_root: &'a Utf8Path,
        env: &'a HashMap<String, String>,
        home_dir: &'a Utf8Path,
    ) -> Self {
        Self {
            invocation_dir,
            project_root,
            env,
            home_dir,
        }
    }

    /// Return the project-directory binding for a given working directory.
    ///
    /// The returned tuple is `(project_dir, matched_cwd)`.
    pub fn project_binding(&self, work_dir: &Utf8Path) -> (Utf8PathBuf, Utf8PathBuf) {
        project_binding_for_work_dir(work_dir, self.env, self.home_dir)
    }

    /// Find the best session id to continue, plus whether any history exists.
    ///
    /// Returns `(session_id, has_history, best_cwd)`. `session_id` is `Some`
    /// when a valid UUID session file with a matching `session-env` entry was
    /// found; otherwise `has_history` may still be true if non-UUID history
    /// files exist.
    pub fn latest_session_id(&self) -> (Option<String>, bool, Option<Utf8PathBuf>) {
        latest_session_id_for_candidates(
            history_candidates(self.invocation_dir, self.project_root),
            self.home_dir,
            &|work_dir| project_binding_for_work_dir(work_dir, self.env, self.home_dir),
        )
    }
}

/// Build the list of candidate working directories to search for history.
pub fn history_candidates(invocation_dir: &Utf8Path, project_root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut candidates = Vec::new();
    for candidate in [invocation_dir, project_root] {
        if let Some(path) = resolve_fallback_path(candidate) {
            if !candidates.iter().any(|p: &Utf8PathBuf| p == &path) {
                candidates.push(path);
            }
        }
    }
    candidates
}

/// Resolve the project directory and matched cwd for a working directory.
pub fn project_binding_for_work_dir(
    work_dir: &Utf8Path,
    env: &HashMap<String, String>,
    home_dir: &Utf8Path,
) -> (Utf8PathBuf, Utf8PathBuf) {
    let projects_root = home_dir.join(".claude").join("projects");
    for candidate in project_dir_candidates(work_dir, env) {
        let normalized_candidate = resolve_fallback_path(&candidate).unwrap_or(candidate);
        let project_dir = projects_root.join(project_key(&normalized_candidate));
        if project_dir.exists() {
            return (project_dir, normalized_candidate);
        }
    }
    let fallback = resolve_fallback_path(work_dir).unwrap_or_else(|| work_dir.to_path_buf());
    (projects_root.join(project_key(&fallback)), fallback)
}

/// Resolve just the project directory for a working directory.
pub fn project_dir_for_work_dir(
    work_dir: &Utf8Path,
    env: &HashMap<String, String>,
    home_dir: &Utf8Path,
) -> Utf8PathBuf {
    project_binding_for_work_dir(work_dir, env, home_dir).0
}

/// Candidate working directories derived from `work_dir` and the `PWD` env var.
pub fn project_dir_candidates(
    work_dir: &Utf8Path,
    env: &HashMap<String, String>,
) -> Vec<Utf8PathBuf> {
    let mut candidates = Vec::new();
    if let Some(env_pwd) = env.get("PWD") {
        if !env_pwd.is_empty() {
            candidates.push(Utf8PathBuf::from(env_pwd));
        }
    }
    candidates.push(work_dir.to_path_buf());
    if let Some(resolved) = resolve_fallback_path(work_dir) {
        if !candidates.iter().any(|p: &Utf8PathBuf| p == &resolved) {
            candidates.push(resolved);
        }
    }
    candidates
}

/// Try to canonicalize a path; fall back to the original path on error.
pub fn resolve_fallback_path(work_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    match std::fs::canonicalize(work_dir.as_std_path()) {
        Ok(p) => Utf8PathBuf::from_path_buf(p).ok(),
        Err(_) => Some(work_dir.to_path_buf()),
    }
}

/// Convert a path into a Claude project directory slug.
///
/// Mirrors Python `project_key`: every non-alphanumeric character becomes `-`.
pub fn project_key(work_dir: &Utf8Path) -> String {
    let re = match regex::Regex::new(r"[^A-Za-z0-9]") {
        Ok(r) => r,
        Err(_) => return work_dir.as_str().to_string(),
    };
    re.replace_all(work_dir.as_str(), "-").into_owned()
}

/// Scan candidate directories and pick the best session id.
#[allow(clippy::type_complexity)]
pub fn latest_session_id_for_candidates<F>(
    candidates: Vec<Utf8PathBuf>,
    home_dir: &Utf8Path,
    project_binding_fn: &F,
) -> (Option<String>, bool, Option<Utf8PathBuf>)
where
    F: Fn(&Utf8Path) -> (Utf8PathBuf, Utf8PathBuf),
{
    let session_env_root = home_dir.join(".claude").join("session-env");
    let mut best_uuid: Option<Utf8PathBuf> = None;
    let mut best_any: Option<Utf8PathBuf> = None;
    let mut has_any_history = false;
    let mut best_cwd: Option<Utf8PathBuf> = None;

    for work_dir in &candidates {
        let (project_dir, matched_cwd) = project_binding_fn(work_dir);
        if !project_dir.exists() {
            continue;
        }
        let session_files = match jsonl_files_in_dir(&project_dir) {
            Some(files) if !files.is_empty() => files,
            _ => continue,
        };
        has_any_history = true;
        (best_any, best_cwd) =
            maybe_update_best_any(&session_files, &matched_cwd, best_any, best_cwd);
        (best_uuid, best_cwd) = update_best_uuid_session(
            &session_files,
            &matched_cwd,
            &session_env_root,
            best_uuid,
            best_cwd,
        );
    }

    if best_uuid.is_none() && !has_any_history {
        // Fallback: search all project directories under the managed home.
        let projects_root = home_dir.join(".claude").join("projects");
        if projects_root.exists() {
            let fallback_cwd = candidates.first().cloned();
            if let Ok(entries) = std::fs::read_dir(projects_root.as_std_path()) {
                for entry in entries.flatten() {
                    let project_dir = entry.path();
                    if !project_dir.is_dir() {
                        continue;
                    }
                    let project_dir = match Utf8PathBuf::from_path_buf(project_dir) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let session_files = match jsonl_files_in_dir(&project_dir) {
                        Some(files) if !files.is_empty() => files,
                        _ => continue,
                    };
                    has_any_history = true;
                    (best_any, best_cwd) = maybe_update_best_any(
                        &session_files,
                        fallback_cwd.as_ref().unwrap_or(&project_dir),
                        best_any,
                        best_cwd,
                    );
                    (best_uuid, best_cwd) = update_best_uuid_session(
                        &session_files,
                        fallback_cwd.as_ref().unwrap_or(&project_dir),
                        &session_env_root,
                        best_uuid,
                        best_cwd,
                    );
                }
            }
        }
    }

    if let Some(uuid_path) = best_uuid {
        return (uuid_path.file_stem().map(|s| s.to_string()), true, best_cwd);
    }
    if has_any_history {
        return (None, true, best_cwd);
    }
    (None, false, None)
}

fn jsonl_files_in_dir(dir: &Utf8Path) -> Option<Vec<Utf8PathBuf>> {
    let entries = std::fs::read_dir(dir.as_std_path()).ok()?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let path = Utf8PathBuf::from_path_buf(path).ok()?;
        if path.as_str().ends_with(".jsonl") {
            files.push(path);
        }
    }
    Some(files)
}

fn maybe_update_best_any(
    session_files: &[Utf8PathBuf],
    work_dir: &Utf8Path,
    best_any: Option<Utf8PathBuf>,
    best_cwd: Option<Utf8PathBuf>,
) -> (Option<Utf8PathBuf>, Option<Utf8PathBuf>) {
    let best_in_dir = match session_files.iter().max_by_key(|p| mtime(p).unwrap_or(0)) {
        Some(p) => p,
        None => return (best_any, best_cwd),
    };
    let new_mtime = mtime(best_in_dir);
    if best_any.is_none() {
        return (Some(best_in_dir.clone()), Some(work_dir.to_path_buf()));
    }
    let cur_mtime = mtime(best_any.as_ref().unwrap());
    match (new_mtime, cur_mtime) {
        (Some(new_mtime), Some(cur_mtime)) if new_mtime > cur_mtime => {
            (Some(best_in_dir.clone()), Some(work_dir.to_path_buf()))
        }
        _ => (best_any, best_cwd),
    }
}

#[allow(clippy::too_many_arguments)]
fn update_best_uuid_session(
    session_files: &[Utf8PathBuf],
    work_dir: &Utf8Path,
    session_env_root: &Utf8Path,
    best_uuid: Option<Utf8PathBuf>,
    best_cwd: Option<Utf8PathBuf>,
) -> (Option<Utf8PathBuf>, Option<Utf8PathBuf>) {
    let mut best_uuid = best_uuid;
    let mut best_cwd = best_cwd;
    for session_file in session_files {
        if !valid_uuid_session_file(session_file, session_env_root) {
            continue;
        }
        if best_uuid.is_none() {
            best_uuid = Some(session_file.clone());
            best_cwd = Some(work_dir.to_path_buf());
            continue;
        }
        match (mtime(session_file), mtime(best_uuid.as_ref().unwrap())) {
            (Some(new_mtime), Some(cur_mtime)) if new_mtime > cur_mtime => {
                best_uuid = Some(session_file.clone());
                best_cwd = Some(work_dir.to_path_buf());
            }
            _ => {}
        }
    }
    (best_uuid, best_cwd)
}

fn valid_uuid_session_file(session_file: &Utf8Path, session_env_root: &Utf8Path) -> bool {
    let stem = match session_file.file_stem() {
        Some(s) => s,
        None => return false,
    };
    if uuid::Uuid::parse_str(stem).is_err() {
        return false;
    }
    let size = match std::fs::metadata(session_file.as_std_path()) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    if size == 0 {
        return false;
    }
    session_env_root.join(stem).exists()
}

fn mtime(path: &Utf8Path) -> Option<u64> {
    let metadata = std::fs::metadata(path.as_std_path()).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_secs())
}
