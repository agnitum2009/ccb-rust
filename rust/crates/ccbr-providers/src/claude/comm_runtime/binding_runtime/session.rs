use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DROID_SESSION_FILENAME: &str = ".droid-session";
const WORKSPACE_BINDING_FILENAME: &str = ".ccbr-workspace.json";
const CCBR_DIRNAME: &str = ".ccbr";

/// A loaded Droid project session.
///
/// Mirrors Python `provider_backends.droid.session_runtime.model.DroidProjectSession`.
#[derive(Debug, Clone, Default)]
pub struct DroidProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl DroidProjectSession {
    pub fn droid_session_id(&self) -> Option<&str> {
        self.data
            .get("droid_session_id")
            .and_then(Value::as_str)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn droid_session_path(&self) -> Option<&str> {
        self.data
            .get("droid_session_path")
            .and_then(Value::as_str)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn is_active(&self) -> bool {
        self.data
            .get("active")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }
}

/// Find the Droid project session file for a working directory.
///
/// Mirrors Python `provider_backends.droid.comm_runtime.session_runtime.find_droid_session_file`.
pub fn find_project_session_file(work_dir: &Path) -> Option<PathBuf> {
    find_workspace_bound_session(work_dir).or_else(|| find_nearest_project_session(work_dir))
}

/// Load the Droid project session, returning `None` if missing or inactive.
///
/// Mirrors Python `provider_backends.droid.session_runtime.loading.load_project_session`.
pub fn load_project_session(work_dir: &Path) -> Option<DroidProjectSession> {
    let session_file = find_project_session_file(work_dir)?;
    let data = read_json(&session_file)?;
    if data.get("active").and_then(Value::as_bool) == Some(false) {
        return None;
    }
    Some(DroidProjectSession { session_file, data })
}

fn find_workspace_bound_session(work_dir: &Path) -> Option<PathBuf> {
    let binding_path = find_workspace_binding(work_dir)?;
    let target = load_workspace_binding(&binding_path)?;
    let candidate = project_config_dir(&target).join(DROID_SESSION_FILENAME);
    candidate.exists().then_some(candidate)
}

fn find_nearest_project_session(work_dir: &Path) -> Option<PathBuf> {
    let anchor = find_nearest_project_anchor(work_dir)?;
    let candidate = project_config_dir(&anchor).join(DROID_SESSION_FILENAME);
    candidate.exists().then_some(candidate)
}

fn find_workspace_binding(current: &Path) -> Option<PathBuf> {
    for root in search_roots(current) {
        let candidate = root.join(WORKSPACE_BINDING_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn load_workspace_binding(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let target = value.get("target_project")?.as_str()?;
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(expand_tilde(trimmed).into())
}

fn find_nearest_project_anchor(current: &Path) -> Option<PathBuf> {
    let mut root = Some(current);
    while let Some(r) = root {
        if project_anchor_dir(r).is_some() {
            let dangerous = r != current && is_dangerous_project_root(r);
            if !dangerous {
                return Some(r.to_path_buf());
            }
        }
        root = r.parent();
    }
    None
}

fn project_anchor_dir(root: &Path) -> Option<PathBuf> {
    let primary = root.join(CCBR_DIRNAME);
    primary.is_dir().then_some(primary)
}

fn project_config_dir(work_dir: &Path) -> PathBuf {
    resolve_dir(work_dir).join(CCBR_DIRNAME)
}

fn resolve_dir(path: &Path) -> PathBuf {
    let expanded = expand_tilde_path(path);
    std::fs::canonicalize(&expanded).unwrap_or_else(|_| {
        if expanded.is_absolute() {
            expanded.components().collect()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&expanded).components().collect())
                .unwrap_or(expanded)
        }
    })
}

fn search_roots(current: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut cur = Some(current);
    while let Some(c) = cur {
        roots.push(c.to_path_buf());
        cur = c.parent();
    }
    roots
}

fn is_dangerous_project_root(root: &Path) -> bool {
    if std::env::var("HOME").ok().map(PathBuf::from).as_ref() == Some(&root.to_path_buf()) {
        return true;
    }
    if std::env::temp_dir() == root {
        return true;
    }
    root.parent().is_none()
}

pub(crate) fn read_json(path: &Path) -> Option<HashMap<String, Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value.as_object().cloned().map(|m| m.into_iter().collect())
}

fn expand_tilde_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    expand_tilde(&s).into()
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_project_session_inactive() {
        let dir = TempDir::new().unwrap();
        let ccb = dir.path().join(".ccbr");
        std::fs::create_dir(&ccb).unwrap();
        let session_file = ccb.join(".droid-session");
        let mut file = std::fs::File::create(&session_file).unwrap();
        file.write_all(br#"{"active": false}"#).unwrap();

        assert!(load_project_session(dir.path()).is_none());
    }

    #[test]
    fn test_load_project_session_active() {
        let dir = TempDir::new().unwrap();
        let ccb = dir.path().join(".ccbr");
        std::fs::create_dir(&ccb).unwrap();
        let session_file = ccb.join(".droid-session");
        let mut file = std::fs::File::create(&session_file).unwrap();
        file.write_all(br#"{"active": true, "droid_session_id": "s1"}"#)
            .unwrap();

        let session = load_project_session(dir.path()).unwrap();
        assert_eq!(session.droid_session_id(), Some("s1"));
        assert!(session.is_active());
    }

    #[test]
    fn test_find_project_session_file_workspace_binding() {
        let dir = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let ccb = target.path().join(".ccbr");
        std::fs::create_dir(&ccb).unwrap();
        std::fs::File::create(ccb.join(".droid-session")).unwrap();

        std::fs::write(
            dir.path().join(".ccbr-workspace.json"),
            serde_json::json!({"target_project": target.path()}).to_string(),
        )
        .unwrap();

        let found = find_project_session_file(dir.path()).unwrap();
        assert!(found.ends_with(".ccbr/.droid-session"));
    }
}
