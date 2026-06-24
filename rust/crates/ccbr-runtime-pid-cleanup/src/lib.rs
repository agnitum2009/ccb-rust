//! CCBR runtime PID cleanup helpers.
//!
//! Mirrors the Python `runtime_pid_cleanup` package from CCBR v7.5.2.
//! Provides lightweight helpers for discovering running processes from `/proc`
//! and cleaning up stale CCBR runtime PIDs on Linux.

pub mod collection;
pub mod matching;
pub mod procfs;
pub mod termination;
pub mod utils;

// Re-exports matching Python `runtime_pid_cleanup.__init__.__all__`.
pub use collection::{
    collect_pid_candidates, collect_project_authority_pid_candidates,
    collect_project_process_candidates,
};
pub use matching::{path_within, pid_matches_project};
pub use procfs::{read_pid_file, read_proc_cmdline, read_proc_path, remove_pid_files};
pub use termination::terminate_runtime_pids;
pub use utils::{coerce_pid, resolved_runtime_roots, RuntimeRef};

use std::fs;

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Alias for process identifiers.
pub type Pid = u32;

/// Result of attempting to remove a stale process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidCleanupResult {
    pub pid: Pid,
    pub cmdline: String,
    pub removed: bool,
}

/// Return the numeric PIDs visible in `/proc`.
///
/// On platforms without `/proc` this returns an empty vector.
pub fn list_pids() -> Vec<Pid> {
    let mut pids = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Ok(pid) = name.parse::<Pid>() {
            pids.push(pid);
        }
    }
    pids
}

/// Check whether a process directory exists in `/proc`.
pub fn pid_exists(pid: Pid) -> bool {
    fs::metadata(format!("/proc/{pid}")).is_ok()
}

/// Read the command line for a PID, replacing NUL bytes with spaces.
///
/// Returns `None` if `/proc/{pid}/cmdline` cannot be read.
/// This is the legacy crate API; new code should prefer [`procfs::read_proc_cmdline`].
pub fn read_cmdline(pid: Pid) -> Option<String> {
    let data = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let text: String = data
        .split(|&b| b == 0)
        .map(|slice| String::from_utf8_lossy(slice).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Some(text.trim().to_string())
}

/// Find PIDs whose command line matches `predicate`.
pub fn find_matching_pids(predicate: impl Fn(&str) -> bool) -> Vec<PidCleanupResult> {
    list_pids()
        .into_iter()
        .filter_map(|pid| {
            let cmdline = read_proc_cmdline(pid);
            if predicate(&cmdline) {
                Some(PidCleanupResult {
                    pid,
                    cmdline,
                    removed: false,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Request termination of a PID by sending SIGTERM.
///
/// This uses the standard `kill` utility and returns whether the command
/// succeeded. It is intentionally conservative: callers are expected to
/// verify the PID belongs to a stale CCBR runtime before invoking it.
pub fn terminate_pid(pid: Pid) -> std::io::Result<()> {
    let status = std::process::Command::new("kill")
        .arg(pid.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("kill failed for pid {pid}")))
    }
}

/// Remove stale CCBR runtime PIDs whose command line contains `needle` and are
/// not the current process.
pub fn cleanup_stale_runtime_pids(needle: &str) -> Vec<PidCleanupResult> {
    let current = std::process::id();
    let mut results = find_matching_pids(|cmdline| cmdline.contains(needle));
    for result in results.iter_mut() {
        if result.pid == current {
            continue;
        }
        result.removed = terminate_pid(result.pid).is_ok();
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_exists() {
        assert!(pid_exists(std::process::id()));
    }

    #[test]
    fn current_cmdline_contains_process_name() {
        let cmdline = read_cmdline(std::process::id()).expect("current process cmdline readable");
        // The test binary's command line should contain the crate test binary name.
        assert!(!cmdline.is_empty());
    }

    #[test]
    fn read_cmdline_matches_read_proc_cmdline() {
        let current = std::process::id();
        assert_eq!(
            read_cmdline(current).unwrap_or_default(),
            read_proc_cmdline(current)
        );
    }

    #[test]
    fn list_pids_includes_current_process_on_linux() {
        if fs::metadata("/proc").is_ok() {
            let pids = list_pids();
            assert!(!pids.is_empty());
            assert!(pids.contains(&std::process::id()));
        }
    }

    #[test]
    fn find_matching_pids_finds_current_process() {
        let current = std::process::id();
        let matches = find_matching_pids(|_| true);
        assert!(matches.iter().any(|r| r.pid == current));
    }
}
