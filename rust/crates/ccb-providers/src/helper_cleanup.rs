//! Mirrors Python `lib/provider_runtime/helper_cleanup.py`.

use camino::Utf8Path;
use std::time::{Duration, Instant};

use ccb_agents::models::AgentState;
use ccb_storage::paths::PathLayout;

use crate::helper_manifest::{
    canonical_runtime_generation, clear_helper_manifest, load_helper_manifest,
    ProviderHelperManifest, ProviderRuntimeView,
};

const ACTIVE_STATES: &[AgentState] = &[
    AgentState::Starting,
    AgentState::Idle,
    AgentState::Busy,
    AgentState::Degraded,
];

/// Remove a stale helper manifest (and terminate its process group) when the
/// current runtime does not own it.
pub fn cleanup_stale_runtime_helper(layout: &PathLayout, runtime: &ProviderRuntimeView) -> bool {
    let helper_path = layout.agent_helper_path(&runtime.agent_name);
    let manifest = match load_helper_manifest_best_effort(&helper_path) {
        Some(m) => m,
        None => return false,
    };
    if runtime_owns_helper(runtime, &manifest) {
        return false;
    }
    terminate_helper_manifest_path(&helper_path)
}

/// Terminate the helper described by the manifest at `path` and clear the file.
pub fn terminate_helper_manifest_path(path: &Utf8Path) -> bool {
    let manifest = match load_helper_manifest_best_effort(path) {
        Some(m) => m,
        None => return false,
    };
    if terminate_helper_manifest(&manifest) {
        clear_helper_manifest(path);
        true
    } else {
        false
    }
}

fn runtime_owns_helper(runtime: &ProviderRuntimeView, manifest: &ProviderHelperManifest) -> bool {
    let provider = runtime.provider.trim().to_lowercase();
    if provider != "codex" {
        return false;
    }
    let state = match runtime.state {
        Some(s) => s,
        None => return false,
    };
    if !ACTIVE_STATES.contains(&state) {
        return false;
    }
    let runtime_root = runtime.runtime_root.trim();
    if runtime_root.is_empty() {
        return false;
    }
    let current_generation = match canonical_runtime_generation(runtime.runtime_generation) {
        Some(g) if g > 0 => g,
        _ => return false,
    };
    runtime.agent_name.trim() == manifest.agent_name.trim()
        && current_generation == manifest.runtime_generation
}

fn terminate_helper_manifest(manifest: &ProviderHelperManifest) -> bool {
    let pgid = manifest.pgid.unwrap_or(0);
    let leader_pid = manifest.leader_pid;
    if pgid > 1 && kill_helper_group(pgid, libc::SIGTERM) {
        if wait_for_helper_exit(leader_pid, Duration::from_millis(200)) {
            return true;
        }
        if kill_helper_group(pgid, libc::SIGKILL) {
            return wait_for_helper_exit(leader_pid, Duration::from_millis(200));
        }
    }
    if leader_pid > 0 {
        return terminate_pid_tree(leader_pid);
    }
    false
}

fn kill_helper_group(pgid: i64, sig: i32) -> bool {
    #[cfg(unix)]
    {
        if pgid <= 1 {
            return false;
        }
        let current_pgid = safe_getpgrp();
        if current_pgid.map(|p| p == pgid).unwrap_or(false) {
            return false;
        }
        unsafe {
            if libc::killpg(pgid as i32, sig) == 0 {
                return true;
            }
            let errno = *libc::__errno_location();
            errno == libc::ESRCH
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pgid, sig);
        false
    }
}

fn wait_for_helper_exit(leader_pid: i64, timeout: Duration) -> bool {
    if leader_pid <= 0 {
        return true;
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !is_pid_alive(leader_pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !is_pid_alive(leader_pid)
}

fn terminate_pid_tree(pid: i64) -> bool {
    #[cfg(not(unix))]
    {
        let _ = pid;
        return false;
    }
    #[cfg(unix)]
    {
        if pid <= 0 {
            return false;
        }
        if !is_pid_alive(pid) {
            return true;
        }
        if kill_helper_group(pid, libc::SIGTERM)
            && wait_for_helper_exit(pid, Duration::from_millis(200))
        {
            return true;
        }
        for sig in [libc::SIGTERM, libc::SIGKILL] {
            unsafe {
                libc::kill(pid as i32, sig);
            }
            if wait_for_helper_exit(pid, Duration::from_millis(200)) {
                return true;
            }
        }
        !is_pid_alive(pid)
    }
}

fn is_pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    #[cfg(unix)]
    unsafe {
        if libc::kill(pid as i32, 0) == 0 {
            return true;
        }
        let errno = *libc::__errno_location();
        // EPERM means the process exists but we lack permission; everything
        // else (ESRCH, EINVAL) means it is not alive.
        errno == libc::EPERM
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn safe_getpgrp() -> Option<i64> {
    #[cfg(unix)]
    {
        let pgid = unsafe { libc::getpgrp() };
        if pgid > 0 {
            Some(pgid as i64)
        } else {
            None
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn load_helper_manifest_best_effort(path: &Utf8Path) -> Option<ProviderHelperManifest> {
    load_helper_manifest(path)
}
