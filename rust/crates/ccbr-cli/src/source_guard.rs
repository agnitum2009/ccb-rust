//! Source-runtime guard for the CCB CLI binary.
//!
//! Mirrors the Python guard in the top-level `ccb` wrapper so that the Rust
//! binary can be exec'd directly without relying on Python for the check.

use std::path::{Path, PathBuf};

/// Result of the source-runtime guard check.
pub struct GuardResult {
    pub allowed: bool,
    pub reason: String,
}

impl GuardResult {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: String::new(),
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
        }
    }
}

/// Decide whether it is safe for the CCB binary to run from a source checkout.
pub fn source_runtime_allowed(argv: &[String], cwd: &Path) -> GuardResult {
    let Some(source_root) = resolve_source_root() else {
        return GuardResult::allow();
    };

    if !is_source_checkout(&source_root) {
        return GuardResult::allow();
    }

    if is_safe_introspection(argv) {
        return GuardResult::allow();
    }

    if std::env::var("CCBR_SOURCE_RUNTIME_OK").ok().as_deref() == Some("1") {
        return GuardResult::allow();
    }

    if std::env::var("PYTEST_CURRENT_TEST").is_ok() {
        return GuardResult::allow();
    }

    let allowed_roots = source_allowed_roots(&source_root);

    for project_path in project_arg_paths(argv, cwd) {
        for allowed in &allowed_roots {
            if path_is_under(&project_path, allowed) {
                return GuardResult::allow();
            }
        }
    }

    for allowed in &allowed_roots {
        if path_is_under(cwd, allowed) {
            return GuardResult::allow();
        }
    }

    let rendered = allowed_roots
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let example_test_root = default_source_allowed_roots(&source_root)
        .first()
        .cloned()
        .unwrap_or_else(|| {
            source_root
                .parent()
                .unwrap_or(&source_root)
                .join("test_ccb2")
        });
    let example_test_binary = source_root.join("ccbr_test");
    GuardResult::deny(format!(
        "Refusing to run the CCB source checkout outside an allowed test project.\n\
         Use the installed release `ccbr` for normal project/work-environment commands.\n\
         Use `{}` from \
         `{}` for source-change validation.\n\
         Current directory: {}\n\
         Allowed source roots: {rendered}\n\
         Override only for explicit diagnostics with CCBR_SOURCE_RUNTIME_OK=1.",
        example_test_binary.display(),
        example_test_root.display(),
        cwd.display()
    ))
}

fn resolve_source_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("CCBR_SOURCE_ROOT") {
        let path = PathBuf::from(root);
        if path.is_dir() {
            return Some(path);
        }
    }

    let mut exe = std::env::current_exe().ok()?;
    if let Ok(resolved) = std::fs::canonicalize(&exe) {
        exe = resolved;
    }

    let mut candidate = exe.parent()?;
    loop {
        if candidate.join(".git").exists() {
            return Some(candidate.to_path_buf());
        }
        match candidate.parent() {
            Some(parent) => candidate = parent,
            None => break,
        }
    }

    None
}

fn is_source_checkout(root: &Path) -> bool {
    root.join(".git").exists()
}

fn is_safe_introspection(argv: &[String]) -> bool {
    if argv.is_empty() {
        return false;
    }
    let first = argv[0].as_str();
    first == "--help"
        || first == "-h"
        || first == "version"
        || first == "--version"
        || argv.iter().any(|a| a == "--help" || a == "-h")
}

fn split_roots(value: &str) -> Vec<PathBuf> {
    value
        .split(if cfg!(windows) { ';' } else { ':' })
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| expand_tilde(s).into())
        .collect()
}

fn default_source_allowed_roots(root: &Path) -> Vec<PathBuf> {
    let parent = root.parent().unwrap_or(root);
    vec![parent.join("test_ccb2")]
}

fn source_allowed_roots(root: &Path) -> Vec<PathBuf> {
    match std::env::var("CCBR_SOURCE_ALLOWED_ROOTS")
        .ok()
        .filter(|s| !s.trim().is_empty())
    {
        Some(value) => split_roots(&value),
        None => default_source_allowed_roots(root),
    }
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}{rest}");
        }
    }
    input.to_string()
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let resolved_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let resolved_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    if resolved_path == resolved_root {
        return true;
    }

    resolved_path
        .ancestors()
        .any(|ancestor| ancestor == resolved_root)
}

fn project_arg_paths(argv: &[String], cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let item = &argv[i];
        let mut value: Option<&str> = None;
        if item == "--project" && i + 1 < argv.len() {
            value = Some(&argv[i + 1]);
            i += 1;
        } else if let Some(v) = item.strip_prefix("--project=") {
            value = Some(v);
        }
        if let Some(v) = value {
            let mut project_path = PathBuf::from(expand_tilde(v));
            if !project_path.is_absolute() {
                project_path = cwd.join(project_path);
            }
            paths.push(project_path);
        }
        i += 1;
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate process-global env vars. `std::env::set_var`
    /// is not thread-safe; without this the default parallel runner races and
    /// produces flaky pass/fail across the with_env/with_source_root tests.
    static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn tmpdir_with_git() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ccbr-source");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        (dir, root)
    }

    #[test]
    fn guard_allows_non_source_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("not-a-source-root");
        let cwd = root.join("workspace");
        std::fs::create_dir_all(&cwd).unwrap();

        // Point CCBR_SOURCE_ROOT at a directory with no .git => allow.
        let argv = vec!["start".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);
    }

    #[test]
    fn guard_allows_help_and_version() {
        let (_dir, root) = tmpdir_with_git();
        let cwd = root.join("test_ccb2");
        std::fs::create_dir_all(&cwd).unwrap();

        // Even outside allowed roots, introspection is safe.
        let argv = vec!["--version".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);

        let argv = vec!["-h".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);

        let argv = vec!["version".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);
    }

    #[test]
    fn guard_allows_inside_default_test_root() {
        let (_dir, root) = tmpdir_with_git();
        let cwd = root.parent().unwrap().join("test_ccb2");
        std::fs::create_dir_all(&cwd).unwrap();

        let argv = vec!["start".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);
    }

    #[test]
    fn guard_denies_outside_allowed_roots() {
        let (_dir, root) = tmpdir_with_git();
        let cwd = root.parent().unwrap().join("some-other-project");
        std::fs::create_dir_all(&cwd).unwrap();

        let argv = vec!["start".to_string()];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(!result.allowed);
        assert!(result.reason.contains("Refusing to run"));
    }

    #[test]
    fn guard_allows_with_env_override() {
        let (_dir, root) = tmpdir_with_git();
        let cwd = root.parent().unwrap().join("some-other-project");
        std::fs::create_dir_all(&cwd).unwrap();

        let argv = vec!["start".to_string()];
        let result = with_env(
            &[
                ("CCBR_SOURCE_ROOT", root.to_str().unwrap()),
                ("CCBR_SOURCE_RUNTIME_OK", "1"),
            ],
            || source_runtime_allowed(&argv, &cwd),
        );
        assert!(result.allowed);
    }

    #[test]
    fn guard_allows_project_arg_under_allowed_root() {
        let (_dir, root) = tmpdir_with_git();
        let cwd = root.parent().unwrap().join("random");
        let project = root.parent().unwrap().join("test_ccb2");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&project).unwrap();

        let argv = vec![
            "start".to_string(),
            "--project".to_string(),
            project.to_str().unwrap().to_string(),
        ];
        let result = with_source_root(&root, || source_runtime_allowed(&argv, &cwd));
        assert!(result.allowed);
    }

    #[test]
    fn guard_allows_custom_allowed_roots() {
        let (_dir, root) = tmpdir_with_git();
        let custom = root.parent().unwrap().join("custom-root");
        let cwd = custom.join("nested");
        std::fs::create_dir_all(&cwd).unwrap();

        let argv = vec!["start".to_string()];
        let roots = format!(
            "{}:{}",
            custom.to_str().unwrap(),
            root.parent().unwrap().join("another").to_str().unwrap()
        );
        let result = with_env(
            &[
                ("CCBR_SOURCE_ROOT", root.to_str().unwrap()),
                ("CCBR_SOURCE_ALLOWED_ROOTS", &roots),
            ],
            || source_runtime_allowed(&argv, &cwd),
        );
        assert!(result.allowed);
    }

    fn with_source_root<T>(root: &Path, f: impl FnOnce() -> T) -> T {
        with_env(&[("CCBR_SOURCE_ROOT", root.to_str().unwrap())], f)
    }

    fn with_env<T>(vars: &[(&str, &str)], f: impl FnOnce() -> T) -> T {
        let _env_lock = ENV_TEST_LOCK.lock().unwrap();
        let mut guards = Vec::new();
        for (name, value) in vars {
            let prev = std::env::var(name).ok();
            std::env::set_var(name, value);
            guards.push((name.to_string(), prev));
        }
        let result = f();
        for (name, prev) in guards {
            match prev {
                Some(v) => std::env::set_var(&name, v),
                None => std::env::remove_var(&name),
            }
        }
        result
    }
}
