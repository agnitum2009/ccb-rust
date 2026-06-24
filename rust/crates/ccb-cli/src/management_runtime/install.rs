//! Mirrors Python `lib/cli/management_runtime/install.py`.
//!
//! Install directory discovery, installer environment construction, and the
//! staged Unix installer runner. Tarball download / extract remain TODO until a
//! release-fetch installer runtime is wired in.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve the source root and install directory for an installer action.
///
/// Mirrors Python `resolve_installer_paths(action, script_root=...)`.
/// The `action` argument is accepted for API parity but does not change the
/// resolved paths for the `install` action.
pub fn resolve_installer_paths(action: &str, script_root: &Path) -> (PathBuf, PathBuf) {
    let _ = action;
    let root = expand_path(script_root);
    if is_source_repo_root(&root) {
        return (root, default_install_dir());
    }
    let install_dir = find_install_dir(&root);
    (install_dir.clone(), install_dir)
}

/// Resolve the managed install directory for a script root.
///
/// Mirrors Python `resolve_managed_install_dir(script_root=...)`.
pub fn resolve_managed_install_dir(script_root: &Path) -> PathBuf {
    let root = expand_path(script_root);
    if is_source_repo_root(&root) {
        return default_install_dir();
    }
    if root.join("install.sh").exists() || root.join("install.ps1").exists() {
        return root;
    }
    if let Some(prefix) = env_install_prefix() {
        return prefix;
    }
    for candidate in install_dir_candidates() {
        if installed_candidate(&candidate) {
            return candidate;
        }
    }
    root
}

/// Build the environment used to run the Unix installer.
///
/// Mirrors Python `_build_unix_installer_env(install_dir, source_dir=...)`.
pub fn build_unix_installer_env(install_dir: &Path, source_dir: &Path) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert(
        "CODEX_INSTALL_PREFIX".into(),
        install_dir.to_string_lossy().into_owned(),
    );

    let source_root = expand_path(source_dir);
    if source_root.join(".git").exists() {
        if !env.contains_key("CCB_SOURCE_KIND") {
            env.insert("CCB_SOURCE_KIND".into(), "source".into());
        }
        if !env.contains_key("CCB_SOURCE_ROOT") {
            env.insert(
                "CCB_SOURCE_ROOT".into(),
                source_root.to_string_lossy().into_owned(),
            );
        }
    }

    if !env.contains_key("CCB_GIT_COMMIT") {
        let (commit, date) = detect_git_head(&source_root);
        if let Some(commit) = commit {
            env.insert("CCB_GIT_COMMIT".into(), commit);
            if !env.contains_key("CCB_GIT_DATE") {
                if let Some(date) = date {
                    env.insert("CCB_GIT_DATE".into(), date);
                }
            }
        }
    }

    env
}

/// Run the installer for `action` from `script_root` and return its exit code.
///
/// Mirrors Python `run_installer(action, script_root=...)`.
/// On Unix the installer tree is staged in a temporary directory, text files are
/// normalized to LF line endings, and `install.sh` is executed with the
/// environment produced by [`build_unix_installer_env`].
pub fn run_installer(action: &str, script_root: &Path) -> i32 {
    let (source_dir, install_dir) = resolve_installer_paths(action, script_root);

    #[cfg(windows)]
    {
        let script = source_dir.join("install.ps1");
        if !script.exists() {
            return missing_installer_message("install.ps1", &source_dir);
        }
        let status = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .arg(action)
            .arg("-InstallPrefix")
            .arg(&install_dir)
            .status();
        return status.map_or(127, |s| s.code().unwrap_or(1));
    }

    #[cfg(not(windows))]
    run_staged_unix_installer(action, &source_dir, &install_dir, None)
}

/// Resolve the active install directory for the bridge.
///
/// Mirrors Python `find_install_dir(script_root)`.
pub fn find_install_dir(script_root: &Path) -> PathBuf {
    if script_root.join("install.sh").exists() || script_root.join("install.ps1").exists() {
        return script_root.to_path_buf();
    }
    for candidate in install_dir_candidates() {
        if installed_candidate(&candidate) {
            return candidate;
        }
    }
    script_root.to_path_buf()
}

/// Return true when `script_root` looks like a source repo checkout.
///
/// Mirrors Python `is_source_repo_root(script_root)`.
pub fn is_source_repo_root(script_root: &Path) -> bool {
    let root = expand_path(script_root);
    root.join("install.sh").exists() && root.join(".git").exists()
}

fn run_staged_unix_installer(
    action: &str,
    source_dir: &Path,
    install_dir: &Path,
    extra_env: Option<HashMap<String, String>>,
) -> i32 {
    let source_dir = expand_path(source_dir);
    let script = source_dir.join("install.sh");
    if !script.exists() {
        return missing_installer_message("install.sh", &source_dir);
    }

    let temp_base = pick_temp_base_dir(install_dir);
    let (staging_root, staged_source) = stage_unix_installer_tree(&source_dir, &temp_base)
        .unwrap_or_else(|_| {
            // Fall back to executing directly from the source directory. This
            // matches the Python fallback behavior when temp space is unusable.
            (source_dir.clone(), source_dir.clone())
        });
    let staged_script = staged_source.join("install.sh");

    let mut env = build_unix_installer_env(install_dir, &source_dir);
    if let Some(extra) = extra_env {
        env.extend(extra);
    }

    let code = Command::new("bash")
        .arg(&staged_script)
        .arg(action)
        .envs(&env)
        .current_dir(&staged_source)
        .status()
        .map_or(127, |s| s.code().unwrap_or(1));

    let _ = std::fs::remove_dir_all(&staging_root);
    code
}

fn missing_installer_message(script_name: &str, install_dir: &Path) -> i32 {
    eprintln!(
        "❌ {script_name} not found in {install_dir}",
        script_name = script_name,
        install_dir = install_dir.display()
    );
    1
}

fn stage_unix_installer_tree(
    source_dir: &Path,
    temp_base: &Path,
) -> std::io::Result<(PathBuf, PathBuf)> {
    let staging_root = tempfile::Builder::new()
        .prefix("ccb-installer-")
        .tempdir_in(temp_base)?
        .keep();
    let staged_source = staging_root.join(
        source_dir
            .file_name()
            .unwrap_or_else(|| OsStr::new("source")),
    );
    copy_source_tree(source_dir, &staged_source)?;
    normalize_text_files(&staged_source)?;
    Ok((staging_root, staged_source))
}

fn copy_source_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    const IGNORED: &[&str] = &[
        ".git",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".venv",
    ];
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if IGNORED.iter().any(|ignored| name == *ignored) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if src_path.is_dir() {
            copy_source_tree(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn normalize_text_files(root: &Path) -> std::io::Result<()> {
    normalize_text_files_recursive(root, root)
}

fn normalize_text_files_recursive(dir: &Path, root: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            normalize_text_files_recursive(&path, root)?;
        } else if path.is_file() && !path.is_symlink() {
            let rel = path.strip_prefix(root).unwrap_or_else(|_| Path::new(""));
            if should_normalize_unix_text(rel) {
                let original = std::fs::read(&path)?;
                if !is_probably_binary(&original) {
                    let normalized = normalize_lf_bytes(&original);
                    if normalized != original {
                        std::fs::write(&path, normalized)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn should_normalize_unix_text(rel: &Path) -> bool {
    if let Some(name) = rel.file_name().and_then(|n| n.to_str()) {
        if name == "install.sh" || name == "ccb" {
            return true;
        }
    }
    if let Some(ext) = rel.extension().and_then(|e| e.to_str()) {
        let ext = ext.to_lowercase();
        if ext == "py" || ext == "sh" || ext == "yml" || ext == "yaml" {
            return true;
        }
    }
    rel.components().count() >= 2
        && rel.components().next().and_then(|c| c.as_os_str().to_str()) == Some("bin")
}

fn is_probably_binary(content: &[u8]) -> bool {
    let sample = &content[..content.len().min(8192)];
    if sample.contains(&0) {
        return true;
    }
    sample.starts_with(b"\x7fELF")
        || sample.starts_with(b"\xca\xfe\xba\xbe")
        || sample.starts_with(b"\xcf\xfa\xed\xfe")
        || sample.starts_with(b"\xfe\xed\xfa\xcf")
}

fn normalize_lf_bytes(content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len());
    let mut i = 0;
    while i < content.len() {
        if content[i] == b'\r' {
            if i + 1 < content.len() && content[i + 1] == b'\n' {
                out.push(b'\n');
                i += 2;
                continue;
            }
            out.push(b'\n');
            i += 1;
            continue;
        }
        out.push(content[i]);
        i += 1;
    }
    out
}

fn detect_git_head(source_dir: &Path) -> (Option<String>, Option<String>) {
    let output = match Command::new("git")
        .arg("-C")
        .arg(source_dir)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
    {
        Ok(o) => o,
        Err(_) => return (None, None),
    };
    if !output.status.success() {
        return (None, None);
    }

    let commit = run_git_log(source_dir, "%h");
    let date = run_git_log(source_dir, "%cs");
    (commit, date)
}

fn run_git_log(source_dir: &Path, format: &str) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(source_dir)
        .arg("log")
        .arg("-1")
        .arg(format!("--format={}", format))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn pick_temp_base_dir(install_dir: &Path) -> PathBuf {
    for key in ["CCB_TMPDIR", "TMPDIR", "TEMP", "TMP"] {
        if let Ok(value) = std::env::var(key) {
            let candidate = PathBuf::from(expand_user(value.trim()));
            if probe_temp_base(&candidate) {
                return candidate;
            }
        }
    }

    let system_temp = std::env::temp_dir();
    if probe_temp_base(&system_temp) {
        return system_temp;
    }

    let fallback_candidates: Vec<PathBuf> = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/usr/tmp"),
        home_dir().join(".cache/ccb/tmp"),
        install_dir.join(".tmp"),
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".tmp"),
    ];
    for candidate in fallback_candidates {
        if probe_temp_base(&candidate) {
            return candidate;
        }
    }

    system_temp
}

fn probe_temp_base(base: &Path) -> bool {
    if std::fs::create_dir_all(base).is_err() {
        return false;
    }
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let probe = base.join(format!(".ccb_tmp_probe_{}_{}", std::process::id(), millis));
    let ok = std::fs::write(&probe, b"1").is_ok() && std::fs::remove_file(&probe).is_ok();
    if !ok {
        let _ = std::fs::remove_file(&probe);
    }
    ok
}

fn env_install_prefix() -> Option<PathBuf> {
    let env_prefix = std::env::var("CODEX_INSTALL_PREFIX").ok()?;
    let trimmed = env_prefix.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_user(trimmed)))
}

fn default_install_dir() -> PathBuf {
    if let Some(prefix) = env_install_prefix() {
        return prefix;
    }
    if cfg!(windows) {
        return windows_install_dir_candidates()[0].clone();
    }
    home_dir().join(".local/share/codex-dual")
}

fn install_dir_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![default_install_dir()];
    if cfg!(windows) {
        for candidate in windows_install_dir_candidates() {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

#[cfg(windows)]
fn windows_install_dir_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let base = PathBuf::from(localappdata);
        candidates.push(base.join("codex-dual"));
        candidates.push(base.join("claude-code-bridge"));
    }
    candidates.push(home_dir().join("AppData/Local/codex-dual"));
    candidates
}

#[cfg(not(windows))]
fn windows_install_dir_candidates() -> Vec<PathBuf> {
    Vec::new()
}

fn installed_candidate(candidate: &Path) -> bool {
    !candidate.as_os_str().is_empty() && candidate.join("ccb").exists()
}

fn home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        if !userprofile.is_empty() {
            return PathBuf::from(userprofile);
        }
    }
    PathBuf::from("/")
}

fn expand_path(path: &Path) -> PathBuf {
    PathBuf::from(expand_user(&path.to_string_lossy()))
}

fn expand_user(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(sep) = rest.chars().next() {
            if sep == '/' || sep == '\\' {
                return format!("{}{}", home_dir().display(), rest);
            }
        }
    }
    path.to_string()
}
