//! Mirrors Python `lib/cli/kill_runtime/processes.py`.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Send a termination signal to a PID.
///
/// Mirrors Python `kill_pid(pid, force)`. Returns `false` for non-positive PIDs.
/// On Windows, shells out to `taskkill`; on POSIX, uses `kill(2)` directly so
/// that `is_pid_alive` can distinguish ESRCH from EPERM via errno.
pub fn kill_pid(pid: i64, force: bool) -> bool {
    if pid <= 0 {
        return false;
    }
    if cfg!(windows) {
        return kill_pid_windows(pid, force);
    }
    let signum = if force { libc::SIGKILL } else { libc::SIGTERM };
    // Safety: `kill(2)` is async-signal-safe; we pass a validated i64 as pid
    // and a well-known signal constant. The only side effect is signal delivery.
    let rc = unsafe { libc::kill(pid as libc::pid_t, signum) };
    if rc == 0 {
        return true;
    }
    // EPERM: process exists but we lack permission — treat as "sent".
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn kill_pid_windows(pid: i64, force: bool) -> bool {
    use std::process::Command;
    let mut args = vec!["/PID".to_string(), pid.to_string()];
    if force {
        args.insert(0, "/F".to_string());
    }
    Command::new("taskkill")
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn kill_pid_windows(_pid: i64, _force: bool) -> bool {
    false
}

/// Return `true` when `pid` is alive (and not a zombie).
///
/// Mirrors Python `is_pid_alive(pid)`. Uses `kill(pid, 0)` so ESRCH (no such
/// process) and EPERM (no permission, but alive) are distinguished via errno.
pub fn is_pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    if cfg!(windows) {
        return is_pid_alive_windows(pid);
    }
    // Safety: signal 0 performs no signal delivery, only an existence check.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return proc_pid_state(pid).as_deref() != Some("Z");
    }
    matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    )
}

#[cfg(windows)]
fn is_pid_alive_windows(pid: i64) -> bool {
    use std::process::Command;
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_pid_alive_windows(_pid: i64) -> bool {
    false
}

/// Best-effort terminate a PID, retrying with force after a grace timeout.
///
/// Mirrors Python `terminate_pid_tree(pid, timeout_s, is_pid_alive_fn)`.
/// Note: process-group signaling (`killpg`) is omitted — it requires
/// `getpgid`/`getpgrp` plumbing that adds little over per-PID signaling here
/// and will land alongside the daemon runtime integration.
pub fn terminate_pid_tree<F>(pid: i64, timeout_s: f64, is_pid_alive_fn: F) -> bool
where
    F: Fn(i64) -> bool,
{
    if pid <= 0 {
        return false;
    }
    if !is_pid_alive_fn(pid) {
        return true;
    }
    if kill_pid(pid, false) && wait_for_pid_exit(pid, timeout_s, &is_pid_alive_fn) {
        return true;
    }
    if !is_pid_alive_fn(pid) {
        return true;
    }
    let force_timeout = timeout_s.max(0.2);
    if kill_pid(pid, true) && wait_for_pid_exit(pid, force_timeout, &is_pid_alive_fn) {
        return true;
    }
    !is_pid_alive_fn(pid)
}

fn wait_for_pid_exit<F>(pid: i64, timeout_s: f64, is_pid_alive_fn: &F) -> bool
where
    F: Fn(i64) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_s.max(0.0));
    while Instant::now() < deadline {
        if !is_pid_alive_fn(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    !is_pid_alive_fn(pid)
}

/// Read the single-letter state field from `/proc/{pid}/stat`.
///
/// Mirrors Python `_proc_pid_state(pid)` / `_parse_proc_stat_state(text)`.
fn proc_pid_state(pid: i64) -> Option<String> {
    if cfg!(windows) || pid <= 0 {
        return None;
    }
    let path = Path::new("/proc").join(pid.to_string()).join("stat");
    let text = std::fs::read_to_string(&path).ok()?;
    parse_proc_stat_state(&text)
}

fn parse_proc_stat_state(text: &str) -> Option<String> {
    let after_comm = text.rsplit_once(") ").map(|(_, rest)| rest)?;
    let mut fields = after_comm.split_whitespace();
    let state = fields.next()?.trim();
    let first = state.chars().next()?;
    Some(first.to_string())
}
