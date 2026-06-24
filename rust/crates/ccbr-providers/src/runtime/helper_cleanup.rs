use camino::Utf8Path;

use super::helper_manifest::{
    clear_helper_manifest, load_helper_manifest, ProviderHelperManifest, RuntimeInfo,
};

const ACTIVE_STATES: &[&str] = &["starting", "idle", "busy", "degraded"];

/// Clean up a stale runtime helper manifest for a runtime.
pub fn cleanup_stale_runtime_helper(path: &Utf8Path, runtime: &RuntimeInfo) -> bool {
    let Some(manifest) = load_helper_manifest(path) else {
        return false;
    };
    if runtime_owns_helper(runtime, &manifest) {
        return false;
    }
    terminate_helper_manifest_path(path)
}

/// Terminate a helper manifest and remove its path.
pub fn terminate_helper_manifest_path(path: &Utf8Path) -> bool {
    let Some(manifest) = load_helper_manifest(path) else {
        return false;
    };
    if terminate_helper_manifest(&manifest) {
        clear_helper_manifest(path);
        true
    } else {
        false
    }
}

fn runtime_owns_helper(runtime: &RuntimeInfo, manifest: &ProviderHelperManifest) -> bool {
    if runtime.provider != "codex" {
        return false;
    }
    if !ACTIVE_STATES.contains(&runtime.state.as_str()) {
        return false;
    }
    let Some(runtime_root) = runtime.runtime_root.as_deref() else {
        return false;
    };
    if runtime_root.trim().is_empty() {
        return false;
    }
    let Some(current_generation) = runtime.runtime_generation else {
        return false;
    };
    if current_generation == 0 {
        return false;
    }
    runtime.agent_name == manifest.agent_name && current_generation == manifest.runtime_generation
}

fn terminate_helper_manifest(manifest: &ProviderHelperManifest) -> bool {
    let pgid = manifest.pgid.unwrap_or(0);
    let leader_pid = manifest.leader_pid;
    if pgid > 1 && kill_helper_group(pgid as i32, libc::SIGTERM) {
        if wait_for_helper_exit(leader_pid as i32, 0.2) {
            return true;
        }
        if kill_helper_group(pgid as i32, libc::SIGKILL) {
            return wait_for_helper_exit(leader_pid as i32, 0.2);
        }
    }
    if leader_pid > 0 {
        return terminate_pid_tree(leader_pid as i32);
    }
    false
}

fn kill_helper_group(pgid: i32, sig: i32) -> bool {
    if pgid <= 1 {
        return false;
    }
    let current_pgid = unsafe { libc::getpgrp() };
    if current_pgid == pgid {
        return false;
    }
    unsafe { libc::killpg(pgid, sig) == 0 }
}

fn wait_for_helper_exit(pid: i32, timeout_s: f64) -> bool {
    if pid <= 0 {
        return true;
    }
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_s.max(0.0));
    while std::time::Instant::now() < deadline {
        if !is_pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    !is_pid_alive(pid)
}

fn terminate_pid_tree(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    if !is_pid_alive(pid) {
        return true;
    }
    if kill_helper_group(pid, libc::SIGTERM) && wait_for_helper_exit(pid, 0.2) {
        return true;
    }
    for sig in [libc::SIGTERM, libc::SIGKILL] {
        unsafe {
            libc::kill(pid, sig);
        }
        if wait_for_helper_exit(pid, 0.2) {
            return true;
        }
    }
    !is_pid_alive(pid)
}

fn is_pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}
