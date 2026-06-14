//! Backend environment detection for Windows/WSL runtime integration.
//!
//! Mirrors Python `terminal_runtime.backend_env`.

use std::path::Path;
use std::process::Command;

/// Get backend environment from explicit env or platform default.
///
/// Returns `"wsl"`, `"windows"`, or `None`.
pub fn get_backend_env() -> Option<String> {
    let v = std::env::var("CCB_BACKEND_ENV")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if v == "wsl" || v == "windows" {
        return Some(v);
    }
    if std::env::consts::OS == "windows" {
        Some("windows".to_string())
    } else {
        None
    }
}

/// Apply BackendEnv=wsl settings (set session root paths for Windows to access WSL).
///
/// Mirrors Python `apply_backend_env`.
pub fn apply_backend_env() {
    if std::env::consts::OS != "windows" || get_backend_env().as_deref() != Some("wsl") {
        return;
    }
    if std::env::var("CODEX_SESSION_ROOT").is_ok() && std::env::var("GEMINI_ROOT").is_ok() {
        return;
    }
    let (distro, home) = wsl_probe_distro_and_home();
    if apply_existing_wsl_session_roots(&distro, &home) {
        return;
    }
    apply_fallback_wsl_session_roots(&distro, &home);
}

fn run_wsl(args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = Command::new("wsl.exe");
    for arg in args {
        cmd.arg(arg);
    }
    cmd.env_clear();
    for (key, value) in crate::env::subprocess_kwargs() {
        if key == "creationflags" {
            if let Ok(_flags) = value.parse::<u32>() {
                #[cfg(windows)]
                cmd.creation_flags(_flags);
            }
        }
    }
    for (key, value) in crate::env::isolated_tmux_env() {
        cmd.env(key, value);
    }
    cmd.output().ok()
}

pub(crate) fn probe_wsl_env() -> Option<(String, String)> {
    let output = run_wsl(&["-e", "sh", "-lc", "echo $WSL_DISTRO_NAME; echo $HOME"])?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.trim().lines();
    let distro = lines.next()?.trim().to_string();
    let home = lines.next()?.trim().to_string();
    if distro.is_empty() || home.is_empty() {
        return None;
    }
    Some((distro, home))
}

pub(crate) fn probe_default_distro() -> String {
    let output = match run_wsl(&["-l", "-q"]) {
        Some(o) => o,
        None => return "Ubuntu".to_string(),
    };
    if !output.status.success() {
        return "Ubuntu".to_string();
    }
    let u16s: Vec<u16> = output
        .stdout
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let decoded = String::from_utf16(&u16s).unwrap_or_default();
    for line in decoded.lines() {
        let distro = line.trim().trim_matches('\0');
        if !distro.is_empty() {
            return distro.to_string();
        }
    }
    "Ubuntu".to_string()
}

pub(crate) fn probe_distro_home(distro: &str) -> String {
    let output = run_wsl(&["-d", distro, "-e", "sh", "-lc", "echo $HOME"]);
    match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "/root".to_string(),
    }
}

pub(crate) fn wsl_probe_distro_and_home() -> (String, String) {
    if let Some(probed) = probe_wsl_env() {
        return probed;
    }
    let distro = probe_default_distro();
    let home = probe_distro_home(&distro);
    (distro, home)
}

pub(crate) fn wsl_prefixes(distro: &str, home: &str) -> (String, String) {
    let suffix = home.replace('/', "\\");
    (
        format!(r"\\wsl.localhost\{distro}{suffix}"),
        format!(r"\\wsl$\{distro}{suffix}"),
    )
}

pub(crate) fn session_roots(prefix: &str) -> (String, String) {
    (
        format!(r"{prefix}\.codex\sessions"),
        format!(r"{prefix}\.gemini\tmp"),
    )
}

fn apply_existing_wsl_session_roots(distro: &str, home: &str) -> bool {
    let prefixes = wsl_prefixes(distro, home);
    for prefix in [prefixes.0, prefixes.1] {
        let (codex_path, gemini_path) = session_roots(&prefix);
        if Path::new(&codex_path).exists() || Path::new(&gemini_path).exists() {
            std::env::set_var("CODEX_SESSION_ROOT", codex_path);
            std::env::set_var("GEMINI_ROOT", gemini_path);
            return true;
        }
    }
    false
}

fn apply_fallback_wsl_session_roots(distro: &str, home: &str) {
    let prefix = wsl_prefixes(distro, home).0;
    let (codex_path, gemini_path) = session_roots(&prefix);
    std::env::set_var("CODEX_SESSION_ROOT", codex_path);
    std::env::set_var("GEMINI_ROOT", gemini_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_backend_env_from_env_var() {
        std::env::set_var("CCB_BACKEND_ENV", "wsl");
        assert_eq!(get_backend_env(), Some("wsl".to_string()));
        std::env::set_var("CCB_BACKEND_ENV", "WINDOWS");
        assert_eq!(get_backend_env(), Some("windows".to_string()));
        std::env::remove_var("CCB_BACKEND_ENV");
    }

    #[test]
    fn test_get_backend_env_platform_default() {
        std::env::remove_var("CCB_BACKEND_ENV");
        #[cfg(windows)]
        assert_eq!(get_backend_env(), Some("windows".to_string()));
        #[cfg(not(windows))]
        assert_eq!(get_backend_env(), None);
    }

    #[test]
    fn test_apply_backend_env_noop_on_linux() {
        std::env::remove_var("CCB_BACKEND_ENV");
        std::env::remove_var("CODEX_SESSION_ROOT");
        std::env::remove_var("GEMINI_ROOT");
        apply_backend_env();
        assert!(std::env::var("CODEX_SESSION_ROOT").is_err());
        assert!(std::env::var("GEMINI_ROOT").is_err());
    }

    #[test]
    fn test_session_roots_format() {
        let (codex, gemini) = session_roots(r"\\wsl.localhost\Ubuntu\home\user");
        assert_eq!(codex, r"\\wsl.localhost\Ubuntu\home\user\.codex\sessions");
        assert_eq!(gemini, r"\\wsl.localhost\Ubuntu\home\user\.gemini\tmp");
    }

    #[test]
    fn test_wsl_prefixes() {
        let (a, b) = wsl_prefixes("Ubuntu", "/home/user");
        assert_eq!(a, r"\\wsl.localhost\Ubuntu\home\user");
        assert_eq!(b, r"\\wsl$\Ubuntu\home\user");
    }

    #[test]
    #[ignore = "requires wsl.exe"]
    fn test_probe_wsl_env() {
        let result = probe_wsl_env();
        assert!(result.is_some());
    }

    #[test]
    #[ignore = "requires wsl.exe"]
    fn test_wsl_probe_distro_and_home() {
        let (distro, home) = wsl_probe_distro_and_home();
        assert!(!distro.is_empty());
        assert!(!home.is_empty());
    }
}
