use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Return the inherited-skills and role-skills directories under a state dir.
pub fn kimi_skill_dirs_for_state_dir(state_dir: &Path) -> (PathBuf, PathBuf) {
    let root = state_dir;
    (root.join("inherited-skills"), root.join("role-skills"))
}

/// Return all skill directories to pass to the Kimi CLI at launch.
pub fn kimi_skill_dirs_for_launch(
    project_root: Option<&Path>,
    workspace_path: Option<&Path>,
    state_dir: &Path,
    env: Option<&HashMap<String, String>>,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    result.extend(kimi_default_skill_dirs(project_root, workspace_path, env));
    let (inherited, role) = kimi_skill_dirs_for_state_dir(state_dir);
    result.push(inherited);
    result.push(role);
    dedupe_paths(result)
}

/// Return the default skill directory search paths for Kimi.
pub fn kimi_default_skill_dirs(
    project_root: Option<&Path>,
    workspace_path: Option<&Path>,
    env: Option<&HashMap<String, String>>,
) -> Vec<PathBuf> {
    let mut source: HashMap<String, String> = HashMap::new();
    for (k, v) in std::env::vars() {
        source.insert(k, v);
    }
    if let Some(env) = env {
        for (k, v) in env {
            source.insert(k.clone(), v.clone());
        }
    }

    let home = env_path(&source, "HOME")
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")));

    let mut paths: Vec<PathBuf> = Vec::new();
    for root in project_skill_roots(project_root, workspace_path) {
        paths.push(root.join(".kimi").join("skills"));
        paths.push(root.join(".claude").join("skills"));
        paths.push(root.join(".codex").join("skills"));
        paths.push(root.join(".agents").join("skills"));
    }
    paths.push(home.join(".kimi").join("skills"));
    paths.push(home.join(".claude").join("skills"));
    paths.push(home.join(".codex").join("skills"));
    paths.push(home.join(".config").join("agents").join("skills"));
    paths.push(home.join(".agents").join("skills"));

    if let Some(kimi_code_home) = env_path(&source, "KIMI_CODE_HOME") {
        paths.push(kimi_code_home.join("skills"));
    } else {
        paths.push(home.join(".kimi-code").join("skills"));
    }

    for root in project_skill_roots(project_root, workspace_path) {
        paths.push(root.join(".kimi-code").join("skills"));
    }

    dedupe_paths(paths)
}

fn project_skill_roots(project_root: Option<&Path>, workspace_path: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(ws) = workspace_path {
        candidates.push(nearest_git_root(ws));
    }
    if let Some(pr) = project_root {
        candidates.push(pr.to_path_buf());
    }
    dedupe_paths(candidates)
}

fn nearest_git_root(path: &Path) -> PathBuf {
    let start = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    let mut current = if start.exists() {
        std::fs::canonicalize(&start).unwrap_or(start.clone())
    } else {
        start.clone()
    };
    loop {
        if current.join(".git").exists() {
            return current;
        }
        let parent = current.parent().map(|p| p.to_path_buf());
        match parent {
            Some(parent) if parent != current => current = parent,
            _ => return start,
        }
    }
}

fn env_path(env: &HashMap<String, String>, key: &str) -> Option<PathBuf> {
    let value = env.get(key).map(|s| s.trim()).filter(|s| !s.is_empty())?;
    Some(PathBuf::from(expand_tilde(value)))
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen: Vec<String> = Vec::new();
    let mut result: Vec<PathBuf> = Vec::new();
    for path in paths {
        let expanded = path.expand_tilde();
        let key = expanded.to_string_lossy().to_string();
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        result.push(expanded);
    }
    result
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

trait PathTildeExt {
    fn expand_tilde(&self) -> PathBuf;
}

impl PathTildeExt for Path {
    fn expand_tilde(&self) -> PathBuf {
        PathBuf::from(expand_tilde(&self.to_string_lossy()))
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_kimi_skill_dirs_for_launch_includes_defaults_and_state() {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let pr = tmp.path().join("project");
        std::fs::create_dir_all(&pr).unwrap();
        let ws = tmp.path().join("workspace");
        std::fs::create_dir_all(&ws).unwrap();

        let dirs = kimi_skill_dirs_for_launch(Some(&pr), Some(&ws), &state_dir, None);
        assert!(!dirs.is_empty());
        assert!(dirs.iter().any(|d| d.ends_with("state/inherited-skills")));
        assert!(dirs.iter().any(|d| d.ends_with("state/role-skills")));
    }
}
