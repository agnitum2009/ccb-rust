//! Mirrors Python `lib/cli/services/kill_runtime/agent_cleanup.py`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use ccb_agents::config::load_project_config;
use ccb_agents::models::{AgentRuntime, AgentState};
use ccb_agents::store::AgentRuntimeStore;
use ccb_storage::paths::PathLayout;
use ccb_terminal::tmux::{normalize_socket_name, socket_ref_from_tmux_env};

#[derive(Debug, Clone, PartialEq)]
pub struct KillPreparation {
    pub configured_agent_names: Vec<String>,
    pub extra_agent_names: Vec<String>,
    pub tmux_sockets: Vec<Option<String>>,
    pub pid_candidates: HashMap<u32, Vec<PathBuf>>,
    pub control_plane_pids: Vec<u32>,
}

/// Collect candidate tmux sockets from environment variables.
pub fn collect_candidate_tmux_sockets() -> HashSet<Option<String>> {
    let mut sockets: HashSet<Option<String>> = HashSet::new();
    for name in [
        std::env::var("CCB_TMUX_SOCKET_PATH")
            .ok()
            .filter(|s| !s.is_empty()),
        normalize_socket_name(std::env::var("CCB_TMUX_SOCKET").ok().as_deref()),
        socket_ref_from_tmux_env(std::env::var("TMUX").ok().as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        sockets.insert(Some(name));
    }
    if sockets.is_empty() {
        sockets.insert(None);
    }
    sockets
}

/// Prepare local shutdown state: gather agent names, tmux sockets, and pid candidates.
#[allow(clippy::too_many_arguments)]
pub fn prepare_local_shutdown<
    F: FnMut(&std::path::Path, Option<&AgentRuntime>, bool) -> HashMap<u32, Vec<PathBuf>>,
    G: FnMut(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>,
>(
    paths: &PathLayout,
    _force: bool,
    mut collect_agent_pid_candidates_fn: F,
    mut collect_project_authority_pid_candidates_fn: Option<G>,
    control_plane_pid_candidates: Option<HashMap<u32, Vec<PathBuf>>>,
) -> anyhow::Result<KillPreparation> {
    let store = AgentRuntimeStore::new(paths.clone());
    let mut tmux_sockets = collect_candidate_tmux_sockets();
    let configured_agent_names = configured_agent_names(paths)?;
    let extra_agent_names = extra_agent_dir_names(paths, &configured_agent_names);
    let mut pid_candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let mut control_plane_pids: Vec<u32> = Vec::new();

    let authority_candidates = match control_plane_pid_candidates {
        Some(candidates) => candidates,
        None => match collect_project_authority_pid_candidates_fn.as_mut() {
            Some(f) => f(paths.project_root.as_std_path()),
            None => HashMap::new(),
        },
    };

    if !authority_candidates.is_empty() {
        control_plane_pids = authority_candidates.keys().copied().collect();
        control_plane_pids.sort();
        for (pid, sources) in authority_candidates {
            pid_candidates.entry(pid).or_default().extend(sources);
        }
    }

    for agent_name in configured_agent_names
        .iter()
        .chain(extra_agent_names.iter())
    {
        let runtime = store.load(agent_name).ok().flatten();
        if let Some(ref runtime) = runtime {
            capture_runtime_tmux_socket(&mut tmux_sockets, runtime);
        }
        let agent_dir = paths.agent_dir(agent_name);
        let fallback = false; // Python uses `force` here; tests pass force=False.
        let mut candidates =
            collect_agent_pid_candidates_fn(agent_dir.as_std_path(), runtime.as_ref(), fallback);
        for (pid, sources) in candidates.drain() {
            pid_candidates.entry(pid).or_default().extend(sources);
        }
        if let Some(runtime) = runtime {
            let stopped = stopped_runtime(&runtime);
            store.save(&stopped)?;
        }
    }

    let tmux_sockets: Vec<Option<String>> = if tmux_sockets.is_empty() {
        vec![None]
    } else {
        tmux_sockets.into_iter().collect()
    };

    Ok(KillPreparation {
        configured_agent_names,
        extra_agent_names,
        tmux_sockets,
        pid_candidates,
        control_plane_pids,
    })
}

fn configured_agent_names(paths: &PathLayout) -> anyhow::Result<Vec<String>> {
    let mut names: Vec<String> = match load_project_config(paths) {
        Ok(result) => result.config.agents.keys().cloned().collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    Ok(names)
}

fn extra_agent_dir_names(paths: &PathLayout, configured_agent_names: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    let known: HashSet<&str> = configured_agent_names.iter().map(|s| s.as_str()).collect();
    let agents_dir = paths.agents_dir();
    let Ok(entries) = std::fs::read_dir(agents_dir.as_std_path()) else {
        return names;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if known.contains(name) || names.contains(&name.to_string()) {
            continue;
        }
        names.push(name.to_string());
    }
    names.sort();
    names
}

fn capture_runtime_tmux_socket(tmux_sockets: &mut HashSet<Option<String>>, runtime: &AgentRuntime) {
    let runtime_ref = runtime.runtime_ref.as_deref().unwrap_or("");
    if !runtime_ref.starts_with("tmux:") {
        return;
    }
    if let Some(ref socket_path) = runtime.tmux_socket_path {
        let trimmed = socket_path.trim();
        if !trimmed.is_empty() {
            tmux_sockets.insert(Some(trimmed.to_string()));
        }
    }
    if let Some(name) = normalize_socket_name(runtime.tmux_socket_name.as_deref()) {
        tmux_sockets.insert(Some(name));
    }
}

fn stopped_runtime(runtime: &AgentRuntime) -> AgentRuntime {
    let mut stopped = runtime.clone();
    stopped.state = AgentState::Stopped;
    stopped.pid = None;
    stopped.runtime_ref = None;
    stopped.session_ref = None;
    stopped.queue_depth = 0;
    stopped.socket_path = None;
    stopped.health = "stopped".into();
    stopped.runtime_pid = None;
    stopped.runtime_root = None;
    stopped.pane_id = None;
    stopped.active_pane_id = None;
    stopped.pane_title_marker = None;
    stopped.pane_state = None;
    stopped.tmux_socket_name = None;
    stopped.tmux_socket_path = None;
    stopped.session_file = None;
    stopped.session_id = None;
    stopped.lifecycle_state = Some("stopped".into());
    stopped.desired_state = Some("stopped".into());
    stopped.reconcile_state = Some("stopped".into());
    stopped.last_failure_reason = None;
    stopped
}
