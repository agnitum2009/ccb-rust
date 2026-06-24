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
    is_pid_alive_with_state(pid, proc_pid_state)
}

/// Variant that reads process state from a custom procfs root.
///
/// Useful for tests that need to simulate `/proc/{pid}/stat` contents.
pub fn is_pid_alive_at(pid: i64, proc_root: &Path) -> bool {
    is_pid_alive_with_state(pid, |pid| proc_pid_state_at(pid, proc_root))
}

/// Internal variant that accepts a state resolver for testability.
pub(crate) fn is_pid_alive_with_state(
    pid: i64,
    state_fn: impl FnOnce(i64) -> Option<String>,
) -> bool {
    if pid <= 0 {
        return false;
    }
    if cfg!(windows) {
        return is_pid_alive_windows(pid);
    }
    // Safety: signal 0 performs no signal delivery, only an existence check.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return state_fn(pid).as_deref() != Some("Z");
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

/// Best-effort terminate a PID tree, retrying with force after a grace timeout.
///
/// Mirrors Python `terminate_pid_tree(pid, timeout_s, is_pid_alive_fn)`.
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
    if kill_pid_tree_once(pid, false) && wait_for_pid_exit(pid, timeout_s, &is_pid_alive_fn) {
        return true;
    }
    if !is_pid_alive_fn(pid) {
        return true;
    }
    let force_timeout = timeout_s.max(0.2);
    if kill_pid_tree_once(pid, true) && wait_for_pid_exit(pid, force_timeout, &is_pid_alive_fn) {
        return true;
    }
    !is_pid_alive_fn(pid)
}

/// Single attempt to kill a PID tree.
///
/// Mirrors Python `_kill_pid_tree_once(pid, force)`.
/// On POSIX this prefers signaling the process group; on Windows it uses
/// `taskkill /T`.
pub fn kill_pid_tree_once(pid: i64, force: bool) -> bool {
    if pid <= 0 {
        return false;
    }
    kill_pid_tree_once_with(
        pid,
        force,
        safe_getpgid,
        safe_getpgrp,
        |pgid, sig| unsafe { libc::killpg(pgid as libc::pid_t, sig) == 0 },
        kill_pid,
    )
}

fn kill_pid_tree_once_with(
    pid: i64,
    force: bool,
    getpgid: impl FnOnce(i64) -> Option<i64>,
    getpgrp: impl FnOnce() -> Option<i64>,
    killpg: impl FnOnce(i64, i32) -> bool,
    kill_pid_fn: impl FnOnce(i64, bool) -> bool,
) -> bool {
    if cfg!(windows) {
        return kill_pid_tree_windows(pid, force);
    }
    let signum = if force { libc::SIGKILL } else { libc::SIGTERM };
    if kill_process_group(pid, signum, getpgid, getpgrp, killpg) {
        return true;
    }
    kill_pid_fn(pid, force)
}

#[cfg(windows)]
fn kill_pid_tree_windows(pid: i64, force: bool) -> bool {
    use std::process::Command;
    let mut args = vec!["/T".to_string(), "/PID".to_string(), pid.to_string()];
    if force {
        args.insert(1, "/F".to_string());
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
fn kill_pid_tree_windows(_pid: i64, _force: bool) -> bool {
    false
}

fn kill_process_group(
    pid: i64,
    signum: i32,
    getpgid: impl FnOnce(i64) -> Option<i64>,
    getpgrp: impl FnOnce() -> Option<i64>,
    killpg: impl FnOnce(i64, i32) -> bool,
) -> bool {
    let pgid = match getpgid(pid) {
        Some(pgid) => pgid,
        None => return false,
    };
    let current_pgid = match getpgrp() {
        Some(pgid) => pgid,
        None => return false,
    };
    if pgid <= 1 || pgid == current_pgid {
        return false;
    }
    killpg(pgid, signum)
}

fn safe_getpgid(pid: i64) -> Option<i64> {
    if pid <= 0 {
        return None;
    }
    let pgid = unsafe { libc::getpgid(pid as libc::pid_t) };
    if pgid < 0 {
        return None;
    }
    Some(pgid as i64)
}

fn safe_getpgrp() -> Option<i64> {
    let pgid = unsafe { libc::getpgrp() };
    if pgid < 0 {
        return None;
    }
    Some(pgid as i64)
}

pub fn wait_for_pid_exit<F>(pid: i64, timeout_s: f64, is_pid_alive_fn: &F) -> bool
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
/// Mirrors Python `_proc_pid_state(pid)`.
pub fn proc_pid_state(pid: i64) -> Option<String> {
    proc_pid_state_at(pid, Path::new("/proc"))
}

/// Variant that accepts a custom `/proc` root for testing.
pub fn proc_pid_state_at(pid: i64, proc_root: &Path) -> Option<String> {
    if cfg!(windows) || pid <= 0 {
        return None;
    }
    let path = proc_root.join(pid.to_string()).join("stat");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_proc_stat_state_extracts_first_letter() {
        let text = "123 (sleep) S 1 2 3 4 5 ...";
        assert_eq!(parse_proc_stat_state(text), Some("S".to_string()));
    }

    #[test]
    fn kill_pid_tree_once_with_prefers_process_group() {
        let mut signaled: Vec<(i64, i32)> = Vec::new();
        let mut killed_pid: Option<(i64, bool)> = None;
        let result = kill_pid_tree_once_with(
            123,
            false,
            |_pid| Some(900),
            || Some(901),
            |pgid, sig| {
                signaled.push((pgid, sig));
                true
            },
            |pid, force| {
                killed_pid = Some((pid, force));
                true
            },
        );
        assert!(result);
        assert_eq!(signaled, vec![(900, libc::SIGTERM)]);
        assert!(killed_pid.is_none());
    }

    #[test]
    fn kill_pid_tree_once_with_falls_back_when_same_pgrp() {
        let mut killed_pid: Option<(i64, bool)> = None;
        let result = kill_pid_tree_once_with(
            123,
            true,
            |_pid| Some(42),
            || Some(42),
            |_pgid, _sig| unreachable!("killpg should not be called"),
            |pid, force| {
                killed_pid = Some((pid, force));
                true
            },
        );
        assert!(result);
        assert_eq!(killed_pid, Some((123, true)));
    }

    #[test]
    fn is_pid_alive_with_state_treats_zombie_as_dead() {
        let current = std::process::id() as i64;
        assert!(!is_pid_alive_with_state(current, |_pid| Some(
            "Z".to_string()
        )));
    }

    #[test]
    fn is_pid_alive_with_state_treats_uninterruptible_as_alive() {
        let current = std::process::id() as i64;
        assert!(is_pid_alive_with_state(current, |_pid| Some(
            "D".to_string()
        )));
    }
}
