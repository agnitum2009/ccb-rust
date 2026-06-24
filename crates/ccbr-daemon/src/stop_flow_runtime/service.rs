//! Mirrors Python `lib/ccbrd/stop_flow_runtime/service.py`.
//! 1:1 file alignment stub.

use crate::Result;
use crate::stop_flow_runtime::models::{StopAllExecution, StopAllSummary};
use crate::stop_flow_runtime::pid_cleanup::{collect_pid_candidates, terminate_runtime_pids};
use crate::stop_flow_runtime::runtime_records::{best_effort_runtime, extra_agent_dir_names};
use crate::stop_flow_runtime::tmux_cleanup::cleanup_stop_tmux_orphans;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Stop all agents in a project
pub fn stop_all_project(
    project_root: &Path,
    project_id: &str,
    paths: &ProjectPaths,
    registry: &dyn Registry,
    project_namespace: Option<&dyn ProjectNamespace>,
    clock: &dyn Clock,
    force: bool,
) -> Result<StopAllExecution> {
    let mut tmux_sockets: HashSet<Option<String>> = HashSet::new();
    let mut pid_candidates: HashMap<u32, Vec<std::path::PathBuf>> = HashMap::new();
    let mut stopped_agents: Vec<String> = Vec::new();

    let configured_agent_names: Vec<String> = registry.list_known_agents()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let extra_agent_names = extra_agent_dir_names(paths, &configured_agent_names)?;

    let mut actions_taken: Vec<String> = Vec::new();
    let mut deferred_actions: Vec<Box<dyn Fn() -> Result<()>>> = Vec::new();

    if let Some(namespace) = project_namespace {
        actions_taken.push("destroy_namespace:deferred".to_string());
        deferred_actions.push(Box::new(move || {
            namespace.destroy("stop_all", force)?;
            Ok(())
        }));
    }

    for agent_name in configured_agent_names.iter().chain(extra_agent_names.iter()) {
        let runtime = best_effort_runtime(
            agent_name,
            &configured_agent_names,
            registry,
            paths,
        )?;

        // Collect tmux sockets
        if let Some(rt) = &runtime {
            if let Some(runtime_ref) = &rt.runtime_ref {
                if runtime_ref.starts_with("tmux:") && rt.tmux_socket_path.is_none() {
                    if let Some(socket_name) = &rt.tmux_socket_name {
                        tmux_sockets.insert(Some(socket_name.clone()));
                    }
                }
            }
        }

        // Collect PID candidates
        let agent_dir = paths.agent_dir(agent_name);
        for (pid, sources) in collect_pid_candidates(&agent_dir, runtime.as_ref(), force)? {
            pid_candidates.entry(pid).or_default().extend(sources);
        }

        // Update registry
        if runtime.is_none() || !configured_agent_names.contains(agent_name) {
            continue;
        }

        let stopped_state = build_stopped_state(runtime.as_ref().unwrap());
        registry.upsert_authority(&stopped_state)?;

        stopped_agents.push(agent_name.clone());
        actions_taken.push(format!("mark_runtime_stopped:{}", agent_name));
    }

    // Cleanup tmux orphans
    let cleanup_summaries = cleanup_stop_tmux_orphans(
        project_id,
        paths,
        &tmux_sockets,
        clock,
        &mut actions_taken,
    )?;

    // Terminate PIDs
    terminate_runtime_pids(project_root, &pid_candidates)?;

    // Clear helper manifests
    for agent_name in configured_agent_names.iter().chain(extra_agent_names.iter()) {
        let helper_path = paths.agent_helper_path(agent_name);
        clear_helper_manifest(&helper_path)?;
    }

    actions_taken.push(format!("terminate_runtime_pids:{}", pid_candidates.len()));

    let summary = StopAllSummary {
        project_id: project_id.to_string(),
        state: "unmounted".to_string(),
        socket_path: paths.ccbrd_socket_path().display().to_string(),
        forced: force,
        stopped_agents: stopped_atoms.clone().into_iter().collect(),
        cleanup_summaries: cleanup_summaries.clone(),
    };

    Ok(StopAllExecution {
        summary,
        stopped_agents: stopped_agents.into_iter().collect(),
        actions_taken: actions_taken.into_iter().collect(),
        cleanup_summaries,
        deferred_actions: deferred_actions.into_iter().map(|f| f()).collect::<Result<Vec<()>>>()?,
    })
}

/// Build stopped state from runtime
fn build_stopped_state(runtime: &RuntimeState) -> RuntimeState {
    RuntimeState {
        state: "stopped".to_string(),
        pid: None,
        runtime_ref: None,
        session_ref: None,
        queue_depth: Some(0),
        socket_path: None,
        health: "stopped".to_string(),
        runtime_pid: None,
        runtime_root: None,
        pane_id: None,
        active_pane_id: None,
        pane_title_marker: None,
        pane_state: None,
        tmux_socket_name: None,
        tmux_socket_path: None,
        session_file: None,
        session_id: None,
        lifecycle_state: "stopped".to_string(),
        desired_state: "stopped".to_string(),
        reconcile_state: "stopped".to_string(),
        last_failure_reason: None,
        ..runtime.clone()
    }
}

/// Clear helper manifest file
fn clear_helper_manifest(path: &Path) -> Result<()> {
    // Stub: would remove or clear the manifest file
    Ok(())
}

// Traits for dependency injection

pub trait Registry {
    fn list_known_agents(&self) -> Vec<String>;
    fn upsert_authority(&self, state: &RuntimeState) -> Result<()>;
}

pub trait ProjectNamespace {
    fn destroy(&self, reason: &str, force: bool) -> Result<()>;
}

pub trait Clock {
    fn now(&self) -> String;
}

// Type definitions

#[derive(Debug, Clone)]
pub struct ProjectPaths {}

impl ProjectPaths {
    pub fn agent_dir(&self, agent_name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("/tmp/agents/{}", agent_name))
    }

    pub fn agent_helper_path(&self, agent_name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("/tmp/helpers/{}.json", agent_name))
    }

    pub fn ccbrd_socket_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from("/tmp/ccbrd.sock")
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub state: String,
    pub pid: Option<u32>,
    pub runtime_ref: Option<String>,
    pub session_ref: Option<String>,
    pub queue_depth: Option<usize>,
    pub socket_path: Option<String>,
    pub health: String,
    pub runtime_pid: Option<String>,
    pub runtime_root: Option<String>,
    pub pane_id: Option<String>,
    pub active_pane_id: Option<String>,
    pub pane_title_marker: Option<String>,
    pub pane_state: Option<String>,
    pub tmux_socket_name: Option<String>,
    pub tmux_socket_path: Option<String>,
    pub session_file: Option<String>,
    pub session_id: Option<String>,
    pub lifecycle_state: String,
    pub desired_state: String,
    pub reconcile_state: String,
    pub last_failure_reason: Option<String>,
}

// Stub implementations for imported modules

pub mod pid_cleanup {
    use super::*;
    use std::collections::HashMap;

    pub fn collect_pid_candidates(
        agent_dir: &Path,
        runtime: Option<&RuntimeState>,
        fallback_to_agent_dir: bool,
    ) -> Result<HashMap<u32, Vec<std::path::PathBuf>>> {
        Ok(HashMap::new())
    }

    pub fn terminate_runtime_pids(
        project_root: &Path,
        pid_candidates: &HashMap<u32, Vec<std::path::PathBuf>>,
    ) -> Result<()> {
        Ok(())
    }
}

pub mod runtime_records {
    use super::*;

    pub fn best_effort_runtime(
        agent_name: &str,
        configured_agent_names: &[String],
        registry: &dyn Registry,
        paths: &ProjectPaths,
    ) -> Result<Option<RuntimeState>> {
        Ok(None)
    }

    pub fn extra_agent_dir_names(
        paths: &ProjectPaths,
        configured_agent_names: &[String],
    ) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

pub mod tmux_cleanup {
    use super::*;

    pub fn cleanup_stop_tmux_orphans(
        project_id: &str,
        paths: &ProjectPaths,
        tmux_sockets: &HashSet<Option<String>>,
        clock: &dyn Clock,
        actions_taken: &mut Vec<String>,
    ) -> Result<Vec<CleanupSummary>> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct CleanupSummary {
    pub socket_path: Option<String>,
    pub windows_cleaned: usize,
    pub panes_cleaned: usize,
}
