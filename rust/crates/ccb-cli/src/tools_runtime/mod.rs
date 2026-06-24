//! Neovim/LazyVim tool provisioning — doctor (status) path.
//!
//! Mirrors the status/doctor portion of Python `cli.tools_runtime.neovim`.
//! Resolves the CCB-managed Neovim install layout (XDG-based), detects the
//! active nvim binary, reads the manifest, and reports status. The heavy
//! download/extract/LazyVim-sync installer path is intentionally not ported
//! here (it requires HTTP + tarball + git orchestration); `install` stays
//! guided toward the install script.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Resolved CCB-managed Neovim layout.
///
/// Mirrors `neovim._paths`.
#[derive(Debug, Clone)]
pub struct NvimPaths {
    pub root: PathBuf,
    pub bin_dir: PathBuf,
    pub wrapper: PathBuf,
    pub bin_link: PathBuf,
    pub profile: PathBuf,
    pub config_nvim: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub marker: PathBuf,
    pub manifest: PathBuf,
    pub managed_nvim: PathBuf,
}

/// Resolve the CCB-managed Neovim paths from XDG/home env.
///
/// Mirrors `neovim._paths`.
pub fn paths() -> NvimPaths {
    let data_home = xdg_or("XDG_DATA_HOME", ".local/share");
    let state_home = xdg_or("XDG_STATE_HOME", ".local/state");
    let cache_home = xdg_or("XDG_CACHE_HOME", ".cache");
    let root = data_home.join("ccb").join("tools").join("neovim");
    let profile = root.join("lazyvim").join("profile");
    let bin_link = std::env::var("CODEX_BIN_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local").join("bin"))
        .join("ccb-nvim");
    NvimPaths {
        bin_dir: root.join("bin"),
        wrapper: root.join("bin").join("ccb-nvim"),
        managed_nvim: root.join("bin").join("nvim"),
        config_nvim: profile.join("config").join("nvim"),
        data: profile.join("share"),
        state: state_home
            .join("ccb")
            .join("tools")
            .join("neovim")
            .join("xdg-state"),
        cache: cache_home
            .join("ccb")
            .join("tools")
            .join("neovim")
            .join("xdg-cache"),
        marker: profile.join(".ccbr-managed-lazyvim"),
        manifest: root.join("manifest.json"),
        profile,
        bin_link,
        root,
    }
}

/// Resolve the active nvim binary: managed install first, else `which nvim`.
///
/// Mirrors `neovim._resolve_nvim`.
pub fn resolve_nvim(paths: &NvimPaths) -> Option<PathBuf> {
    if is_executable_file(&paths.managed_nvim) {
        return Some(paths.managed_nvim.clone());
    }
    which("nvim")
}

/// Read the CCB tools manifest (JSON object), or an empty map if absent/invalid.
///
/// Mirrors `neovim._read_manifest`.
pub fn read_manifest(paths: &NvimPaths) -> serde_json::Map<String, Value> {
    let text = match std::fs::read_to_string(&paths.manifest) {
        Ok(t) => t,
        Err(_) => return serde_json::Map::new(),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

/// Aggregated Neovim status for `ccb tools doctor neovim`.
///
/// Mirrors `neovim.neovim_status` (status/reason/binary/wrapper fields). The
/// LazyVim health deep-check (headless nvim) is approximated by marker presence.
pub fn neovim_status() -> NeovimStatus {
    let paths = paths();
    let wrapper_exists = paths.wrapper.is_file() && is_executable_file(&paths.wrapper);
    let nvim = resolve_nvim(&paths);
    let manifest = read_manifest(&paths);

    if !wrapper_exists {
        return NeovimStatus {
            status: "missing".to_string(),
            reason: Some("ccb-nvim wrapper is not installed".to_string()),
            binary: nvim,
            wrapper: Some(paths.wrapper.clone()),
            managed_neovim_version: manifest
                .get("managed_neovim_version")
                .and_then(|v| v.as_str())
                .map(String::from),
            lazyvim_enabled: false,
        };
    }

    let lazyvim_enabled = manifest
        .get("lazyvim_profile_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(paths.marker.exists());
    let health_ok = !lazyvim_enabled || paths.marker.exists();
    let status_value = if lazyvim_enabled && !health_ok {
        "degraded"
    } else {
        "ok"
    };
    NeovimStatus {
        status: status_value.to_string(),
        reason: if lazyvim_enabled && !health_ok {
            Some("LazyVim profile health check did not pass".to_string())
        } else {
            None
        },
        binary: nvim,
        wrapper: Some(paths.wrapper.clone()),
        managed_neovim_version: manifest
            .get("managed_neovim_version")
            .and_then(|v| v.as_str())
            .map(String::from),
        lazyvim_enabled,
    }
}

/// Rendered Neovim status for the CLI.
#[derive(Debug, Clone)]
pub struct NeovimStatus {
    pub status: String,
    pub reason: Option<String>,
    pub binary: Option<PathBuf>,
    pub wrapper: Option<PathBuf>,
    pub managed_neovim_version: Option<String>,
    pub lazyvim_enabled: bool,
}

/// Render the status as the `ccb tools doctor neovim` output.
pub fn render_neovim_status(status: &NeovimStatus) -> String {
    let mut out = format!("neovim_status: {}\n", status.status);
    if let Some(reason) = &status.reason {
        out.push_str(&format!("reason: {reason}\n"));
    }
    out.push_str(&format!(
        "binary: {}\n",
        status
            .binary
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    if let Some(wrapper) = &status.wrapper {
        out.push_str(&format!("wrapper: {}\n", wrapper.to_string_lossy()));
    }
    if let Some(version) = &status.managed_neovim_version {
        out.push_str(&format!("managed_neovim_version: {version}\n"));
    }
    out.push_str(&format!(
        "lazyvim_profile_enabled: {}\n",
        status.lazyvim_enabled
    ));
    out
}

// --- helpers ---------------------------------------------------------------

fn xdg_or(env_name: &str, default_suffix: &str) -> PathBuf {
    match std::env::var(env_name) {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => home_dir().join(default_suffix),
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate process-global env vars. `std::env::set_var`
    /// is not thread-safe; without this the default parallel runner races and
    /// produces flaky pass/fail.
    static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_paths_under_xdg_data_home() {
        let _env_lock = ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("XDG_DATA_HOME", "/tmp/ccb-test-data");
        let p = paths();
        assert!(p.root.starts_with("/tmp/ccb-test-data/ccb/tools/neovim"));
        assert_eq!(p.wrapper.file_name().unwrap(), "ccb-nvim");
        assert_eq!(p.managed_nvim.file_name().unwrap(), "nvim");
        std::env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn test_resolve_nvim_prefers_managed() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let nvim = bin.join("nvim");
        std::fs::write(&nvim, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&nvim).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&nvim, perms).unwrap();
        }
        let mut p = paths();
        p.managed_nvim = nvim.clone();
        assert_eq!(resolve_nvim(&p), Some(nvim));
    }

    #[test]
    fn test_read_manifest_missing_is_empty() {
        let p = paths();
        let m = read_manifest(&NvimPaths {
            manifest: PathBuf::from("/nonexistent/manifest.json"),
            ..p
        });
        assert!(m.is_empty());
    }

    #[test]
    fn test_read_manifest_parses() {
        let dir = tempfile::tempdir().unwrap();
        let mpath = dir.path().join("manifest.json");
        std::fs::write(
            &mpath,
            r#"{"managed_neovim_version":"0.10.0","lazyvim_profile_enabled":true}"#,
        )
        .unwrap();
        let mut p = paths();
        p.manifest = mpath;
        let m = read_manifest(&p);
        assert_eq!(
            m.get("managed_neovim_version").and_then(|v| v.as_str()),
            Some("0.10.0")
        );
        assert_eq!(
            m.get("lazyvim_profile_enabled").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_status_reports_missing_without_wrapper() {
        let _env_lock = ENV_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_DATA_HOME", dir.path());
        let status = neovim_status();
        assert_eq!(status.status, "missing");
        assert!(status.reason.unwrap().contains("wrapper"));
        std::env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn test_render_status_includes_fields() {
        let status = NeovimStatus {
            status: "ok".into(),
            reason: None,
            binary: Some(PathBuf::from("/usr/bin/nvim")),
            wrapper: Some(PathBuf::from("/x/ccb-nvim")),
            managed_neovim_version: Some("0.10.0".into()),
            lazyvim_enabled: true,
        };
        let rendered = render_neovim_status(&status);
        assert!(rendered.contains("neovim_status: ok"));
        assert!(rendered.contains("binary: /usr/bin/nvim"));
        assert!(rendered.contains("managed_neovim_version: 0.10.0"));
    }
}

pub mod neovim;
