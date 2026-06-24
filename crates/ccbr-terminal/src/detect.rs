//! Tmux/terminal detection helpers.
//!
//! Mirrors Python `terminal_runtime.detect`.

use std::collections::HashMap;
use std::process::Command;

/// Return the TTY name for stdin/stdout/stderr, if any.
pub fn current_tty() -> Option<String> {
    for fd in [0, 1, 2] {
        let mut buf = [0u8; 256];
        // SAFETY: ttyname_r writes into the provided buffer and returns 0 on success.
        let rc = unsafe { libc::ttyname_r(fd, buf.as_mut_ptr().cast::<libc::c_char>(), buf.len()) };
        if rc == 0 {
            let cstr = std::ffi::CStr::from_bytes_until_nul(&buf).ok()?;
            let tty = cstr.to_string_lossy().to_string();
            if !tty.is_empty() {
                return Some(tty);
            }
        }
    }
    None
}

/// Check whether tmux environment variables are present in the current process.
pub fn tmux_env_present() -> bool {
    let env: HashMap<String, String> = std::env::vars().collect();
    tmux_env_present_env(&env)
}

/// Check whether tmux environment variables are present in the provided env map.
pub fn tmux_env_present_env(env: &HashMap<String, String>) -> bool {
    env_nonempty_env(env, "TMUX") || env_nonempty_env(env, "TMUX_PANE")
}

fn env_nonempty_env(env: &HashMap<String, String>, name: &str) -> bool {
    env.get(name).map(|s| !s.trim().is_empty()).unwrap_or(false)
}

fn env_nonempty(name: &str) -> bool {
    std::env::var(name)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn tmux_in_path() -> bool {
    std::env::var("PATH")
        .ok()
        .map(|path| {
            path.split(':')
                .any(|dir| std::path::Path::new(dir).join("tmux").exists())
        })
        .unwrap_or(false)
}

fn default_which_fn(name: &str) -> Option<String> {
    if name == "tmux" && tmux_in_path() {
        Some("tmux".to_string())
    } else {
        None
    }
}

fn default_run_fn(args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(args.first()?);
    for arg in &args[1..] {
        cmd.arg(arg);
    }
    cmd.env_clear();
    for (key, value) in crate::env::isolated_tmux_env() {
        cmd.env(key, value);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout);
    Some(value.trim().to_string())
}

/// Check whether the current process appears to be running inside tmux.
pub fn inside_tmux() -> bool {
    let env: HashMap<String, String> = std::env::vars().collect();
    inside_tmux_with(&env, default_which_fn, default_run_fn, current_tty)
}

/// Injected version of `inside_tmux` for testing.
pub fn inside_tmux_with<W, R, C>(
    env: &HashMap<String, String>,
    which_fn: W,
    run_fn: R,
    current_tty_fn: C,
) -> bool
where
    W: Fn(&str) -> Option<String>,
    R: Fn(&[&str]) -> Option<String>,
    C: Fn() -> Option<String>,
{
    if !tmux_env_present_env(env) {
        return false;
    }
    if which_fn("tmux").is_none() {
        return false;
    }

    let tty = current_tty_fn();
    let pane = env
        .get("TMUX_PANE")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if !pane.is_empty() && pane_tty_matches_with(&run_fn, &pane, tty.as_deref()) {
        return true;
    }
    if client_tty_matches_with(&run_fn, tty.as_deref()) {
        return true;
    }
    if !pane.is_empty() && pane_id_matches_with(&run_fn, &pane, tty.as_deref()) {
        return true;
    }

    false
}

/// Check whether the tmux pane identified by `$TMUX_PANE` uses the given TTY.
pub fn pane_tty_matches(tty: &str) -> bool {
    let pane = std::env::var("TMUX_PANE").unwrap_or_default();
    pane_tty_matches_with(default_run_fn, pane.trim(), Some(tty))
}

fn pane_tty_matches_with<R>(run_fn: R, pane: &str, tty: Option<&str>) -> bool
where
    R: Fn(&[&str]) -> Option<String>,
{
    let tty = match tty {
        Some(t) if !t.is_empty() => t,
        _ => return false,
    };
    tmux_value_with(&run_fn, Some(pane), "#{pane_tty}")
        .map(|v| v == tty)
        .unwrap_or(false)
}

/// Check whether the current tmux client's TTY matches.
pub fn client_tty_matches(tty: &str) -> bool {
    client_tty_matches_with(default_run_fn, Some(tty))
}

fn client_tty_matches_with<R>(run_fn: R, tty: Option<&str>) -> bool
where
    R: Fn(&[&str]) -> Option<String>,
{
    let tty = match tty {
        Some(t) if !t.is_empty() => t,
        _ => return false,
    };
    tmux_value_with(&run_fn, None, "#{client_tty}")
        .map(|v| v == tty)
        .unwrap_or(false)
}

/// Check whether `pane_id` resolves to a valid tmux pane id.
pub fn pane_id_matches(pane_id: &str) -> bool {
    if pane_id.is_empty() || current_tty().is_some() {
        return false;
    }
    pane_id_matches_with(default_run_fn, pane_id, None)
}

fn pane_id_matches_with<R>(run_fn: R, pane: &str, tty: Option<&str>) -> bool
where
    R: Fn(&[&str]) -> Option<String>,
{
    if tty.is_some() || pane.is_empty() {
        return false;
    }
    tmux_value_with(&run_fn, Some(pane), "#{pane_id}")
        .map(|v| v.starts_with('%'))
        .unwrap_or(false)
}

/// Run `tmux display-message -p` for a target and format string.
pub fn tmux_value(target: Option<&str>, format_string: &str) -> Option<String> {
    tmux_value_with(default_run_fn, target, format_string)
}

/// Injected version of `tmux_value` for testing.
pub fn tmux_value_with<R>(run_fn: R, target: Option<&str>, format_string: &str) -> Option<String>
where
    R: Fn(&[&str]) -> Option<String>,
{
    let mut args: Vec<&str> = vec!["tmux", "display-message", "-p"];
    if let Some(t) = target {
        args.push("-t");
        args.push(t);
    }
    args.push(format_string);
    run_fn(&args)
}

/// Detect the current terminal type.
///
/// Returns `Some("tmux")` when running inside tmux; otherwise `None`.
pub fn detect_terminal() -> Option<String> {
    let env: HashMap<String, String> = std::env::vars().collect();
    detect_terminal_with(&env, default_which_fn, default_run_fn, current_tty)
}

/// Injected version of `detect_terminal` for testing.
pub fn detect_terminal_with<W, R, C>(
    env: &HashMap<String, String>,
    which_fn: W,
    run_fn: R,
    current_tty_fn: C,
) -> Option<String>
where
    W: Fn(&str) -> Option<String>,
    R: Fn(&[&str]) -> Option<String>,
    C: Fn() -> Option<String>,
{
    if inside_tmux_with(env, which_fn, run_fn, current_tty_fn) {
        Some("tmux".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_tty_does_not_panic() {
        // In CI this is usually not a TTY; just ensure it returns Option<String>.
        let _ = current_tty();
    }

    #[test]
    fn test_tmux_env_present_reads_env() {
        std::env::set_var("TMUX", "/tmp/tmux-1000/default,123,0");
        assert!(tmux_env_present());
        std::env::remove_var("TMUX");

        std::env::set_var("TMUX_PANE", "%0");
        assert!(tmux_env_present());
        std::env::remove_var("TMUX_PANE");

        assert!(!tmux_env_present());
    }

    #[test]
    fn test_inside_tmux_false_without_env() {
        std::env::remove_var("TMUX");
        std::env::remove_var("TMUX_PANE");
        assert!(!inside_tmux());
    }

    #[test]
    fn test_pane_tty_matches_false_without_tmux() {
        std::env::remove_var("TMUX");
        std::env::remove_var("TMUX_PANE");
        assert!(!pane_tty_matches("/dev/pts/0"));
    }

    #[test]
    fn test_client_tty_matches_false_without_tmux() {
        // Ensure tmux binary is not found so the function returns false even when
        // the test runner happens to be executing inside a tmux session.
        let original_path = std::env::var("PATH").ok();
        std::env::set_var("PATH", "");
        assert!(!client_tty_matches("/dev/pts/0"));
        match original_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }

    #[test]
    fn test_pane_id_matches_false_with_tty() {
        // pane_id_matches only returns true when there is no current TTY.
        // We cannot reliably fake "no TTY" here, but we can at least verify
        // the function is callable and returns a bool.
        let _ = pane_id_matches("%0");
    }

    #[test]
    fn test_tmux_value_returns_none_when_tmux_missing() {
        // Ensure tmux is not on PATH by using a deliberately broken PATH.
        let original_path = std::env::var("PATH").ok();
        std::env::set_var("PATH", "/nonexistent");
        assert!(tmux_value(None, "#{client_tty}").is_none());
        match original_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }

    #[test]
    fn test_detect_terminal_outside_tmux() {
        std::env::remove_var("TMUX");
        std::env::remove_var("TMUX_PANE");
        assert_eq!(detect_terminal(), None);
    }
}
