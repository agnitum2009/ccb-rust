use std::path::{Path, PathBuf};

/// Compute the managed Droid home directory for a runtime directory.
///
/// Mirrors Python `provider_backends.droid.home.managed_droid_home_for_runtime`.
pub fn managed_droid_home_for_runtime(runtime_dir: &Path) -> PathBuf {
    let runtime_dir = expand_tilde_path(runtime_dir);
    if runtime_dir.parent().and_then(|p| p.file_name())
        == Some(std::ffi::OsStr::new("provider-runtime"))
    {
        runtime_dir
            .parent()
            .and_then(Path::parent)
            .map(|p| p.join("provider-state").join("droid").join("home"))
            .unwrap_or_else(|| runtime_dir.join("droid-home"))
    } else {
        runtime_dir.join("droid-home")
    }
}

/// Resolve the default Droid sessions root from the environment.
///
/// Mirrors Python `provider_backends.droid.comm_runtime.log_reader.default_sessions_root`.
pub fn default_sessions_root() -> PathBuf {
    let override_root = std::env::var("DROID_SESSIONS_ROOT")
        .or_else(|_| std::env::var("FACTORY_SESSIONS_ROOT"))
        .ok()
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        });
    if let Some(root) = override_root {
        return expand_tilde_path(&root);
    }
    let factory_home = std::env::var("FACTORY_HOME")
        .or_else(|_| std::env::var("FACTORY_ROOT"))
        .ok()
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        });
    let base = factory_home
        .as_deref()
        .map(expand_tilde_path)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".factory"))
                .unwrap_or_else(|| PathBuf::from(".factory"))
        });
    base.join("sessions")
}

/// Materialize a Droid home directory with inherited skills projection.
///
/// Mirrors Python `provider_backends.droid.home.materialize_droid_home_config`.
pub fn materialize_droid_home_config(
    target_home: &Path,
    profile_inherit_skills: Option<bool>,
    source_home: Option<&Path>,
) -> PathBuf {
    let target_home = expand_tilde_path(target_home);
    let source_home = source_home
        .map(expand_tilde_path)
        .unwrap_or_else(system_factory_home);
    std::fs::create_dir_all(&target_home).ok();
    std::fs::create_dir_all(target_home.join("sessions")).ok();
    route_inherited_tree(
        &source_home.join("skills"),
        &target_home.join("skills"),
        profile_inherit_skills.unwrap_or(true),
    );
    target_home
}

fn route_inherited_tree(source: &Path, target: &Path, enabled: bool) {
    // Minimal projection: copy enabled skill files into the target tree.
    if !enabled {
        return;
    }
    if !source.is_dir() {
        return;
    }
    std::fs::create_dir_all(target).ok();
    let Ok(entries) = std::fs::read_dir(source) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().unwrap_or_default();
            let dest = target.join(name);
            let _ = std::fs::copy(&path, &dest);
        }
    }
}

fn system_factory_home() -> PathBuf {
    if std::env::var("CCB_SOURCE_HOME").is_ok() {
        return current_provider_source_home().join(".factory");
    }
    for name in ["FACTORY_HOME", "FACTORY_ROOT"] {
        if let Ok(raw) = std::env::var(name) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let candidate = expand_tilde_path(&PathBuf::from(trimmed));
                if !looks_like_ccb_provider_home(&candidate) {
                    return candidate;
                }
            }
        }
    }
    current_provider_source_home().join(".factory")
}

fn current_provider_source_home() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn looks_like_ccb_provider_home(path: &Path) -> bool {
    let parts: Vec<_> = path.iter().collect();
    for index in 0..parts.len().saturating_sub(4) {
        if parts[index] != std::ffi::OsStr::new("agents") {
            continue;
        }
        if parts.get(index + 2) == Some(&std::ffi::OsStr::new("provider-state"))
            && parts.get(index + 4) == Some(&std::ffi::OsStr::new("home"))
        {
            return true;
        }
    }
    false
}

pub(crate) fn expand_tilde_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    PathBuf::from(expand_tilde(&s))
}

pub(crate) fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
