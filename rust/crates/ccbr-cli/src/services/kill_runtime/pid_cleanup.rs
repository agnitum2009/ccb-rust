//! Mirrors Python `lib/cli/services/kill_runtime/pid_cleanup.py`.
//!
//! Thin wrappers around `ccbr_runtime_pid_cleanup` so the CLI kill service
//! receives Python-compatible signatures and can pass `AgentRuntime` values
//! directly.

use ccbr_agents::models::AgentRuntime;
use ccbr_runtime_pid_cleanup::RuntimeRef;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use ccbr_runtime_pid_cleanup::{
    coerce_pid, collect_project_authority_pid_candidates, path_within, pid_matches_project,
    read_pid_file, read_proc_cmdline, read_proc_path, remove_pid_files, terminate_runtime_pids,
};

struct AgentRuntimeRef<'a>(&'a AgentRuntime);

impl<'a> RuntimeRef for AgentRuntimeRef<'a> {
    fn runtime_pid(&self) -> Option<u32> {
        self.0.runtime_pid.and_then(|p| u32::try_from(p).ok())
    }

    fn pid(&self) -> Option<u32> {
        self.0.pid.and_then(|p| u32::try_from(p).ok())
    }

    fn runtime_root(&self) -> Option<&str> {
        self.0.runtime_root.as_deref()
    }
}

/// Collect PID candidates for a single agent directory.
///
/// Mirrors Python `collect_agent_pid_candidates(agent_dir, *, runtime, fallback_to_agent_dir)`.
pub fn collect_agent_pid_candidates(
    agent_dir: &Path,
    runtime: Option<&AgentRuntime>,
    fallback_to_agent_dir: bool,
) -> HashMap<u32, Vec<PathBuf>> {
    let runtime_ref = runtime.map(AgentRuntimeRef);
    let dyn_ref = runtime_ref.as_ref().map(|r| r as &dyn RuntimeRef);
    ccbr_runtime_pid_cleanup::collect_pid_candidates(agent_dir, dyn_ref, fallback_to_agent_dir)
}

/// Convenience one-argument wrapper used by `terminate_runtime_pids` callbacks.
pub fn collect_project_process_candidates(project_root: &Path) -> HashMap<u32, Vec<PathBuf>> {
    ccbr_runtime_pid_cleanup::collect_project_process_candidates(
        project_root,
        Path::new("/proc"),
        ccbr_runtime_pid_cleanup::read_proc_cmdline,
        Some(std::process::id()),
    )
}
