//! Mirrors Python `test/test_v2_kill_service.py` agent-coverage subset.

use camino::Utf8PathBuf;
use ccbr_agents::models::{AgentRuntime, AgentState};
use ccbr_agents::store::AgentRuntimeStore;
use ccbr_cli::services::kill_runtime::agent_cleanup::{
    collect_candidate_tmux_sockets, prepare_local_shutdown,
};
use ccbr_storage::paths::PathLayout;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn make_paths(tmp: &tempfile::TempDir) -> PathLayout {
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    let paths = PathLayout::new(root);
    std::fs::create_dir_all(paths.ccbr_dir()).unwrap();
    std::fs::write(paths.ccbr_dir().join("ccbr.config"), "demo:codex\n").unwrap();
    paths
}

fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
    let originals: Vec<(String, Option<String>)> = vars
        .iter()
        .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for (k, v) in vars {
        match v {
            Some(val) => std::env::set_var(k, val),
            None => std::env::remove_var(k),
        }
    }
    let _guard = Guard(originals);
    f();
}

struct Guard(Vec<(String, Option<String>)>);

impl Drop for Guard {
    fn drop(&mut self) {
        for (k, v) in &self.0 {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}

fn stopped_runtime(agent_name: &str, paths: &PathLayout) -> AgentRuntime {
    AgentRuntime {
        agent_name: agent_name.into(),
        state: AgentState::Idle,
        runtime_ref: Some("tmux:%1".into()),
        tmux_socket_path: Some("/tmp/ccb project/tmux.sock".into()),
        project_id: paths.project_id().into(),
        backend_type: "pane-backed".into(),
        ..Default::default()
    }
}

fn no_agent_pids(
    _agent_dir: &std::path::Path,
    _runtime: Option<&AgentRuntime>,
    _force: bool,
) -> HashMap<u32, Vec<PathBuf>> {
    HashMap::new()
}

fn no_authority_pids(_project_root: &std::path::Path) -> HashMap<u32, Vec<PathBuf>> {
    HashMap::new()
}

#[test]
fn test_collect_candidate_tmux_sockets_preserves_tmux_socket_path() {
    with_env(
        &[
            ("CCBR_TMUX_SOCKET", None),
            ("CCBR_TMUX_SOCKET_PATH", None),
            ("TMUX", Some("/tmp/ccb project/tmux.sock,123,0")),
        ],
        || {
            let sockets = collect_candidate_tmux_sockets();
            assert_eq!(
                sockets,
                HashSet::from([Some("/tmp/ccb project/tmux.sock".into())])
            );
        },
    );
}

#[test]
fn test_collect_candidate_tmux_sockets_defaults_to_none() {
    with_env(
        &[
            ("CCBR_TMUX_SOCKET", None),
            ("CCBR_TMUX_SOCKET_PATH", None),
            ("TMUX", None),
        ],
        || {
            let sockets = collect_candidate_tmux_sockets();
            assert_eq!(sockets, HashSet::from([None]));
        },
    );
}

#[test]
fn test_prepare_local_shutdown_captures_runtime_tmux_socket_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let runtime = stopped_runtime("demo", &paths);
    AgentRuntimeStore::new(paths.clone())
        .save(&runtime)
        .unwrap();

    with_env(
        &[
            ("TMUX", None),
            ("CCBR_TMUX_SOCKET", None),
            ("CCBR_TMUX_SOCKET_PATH", None),
        ],
        || {
            let preparation =
                prepare_local_shutdown(&paths, false, no_agent_pids, Some(no_authority_pids), None)
                    .unwrap();

            assert!(preparation
                .tmux_sockets
                .contains(&Some("/tmp/ccb project/tmux.sock".into())));
            let stored = AgentRuntimeStore::new(paths.clone())
                .load("demo")
                .unwrap()
                .unwrap();
            assert_eq!(stored.state, AgentState::Stopped);
            assert!(stored.runtime_ref.is_none());
            assert_eq!(stored.desired_state.as_deref(), Some("stopped"));
            assert_eq!(stored.reconcile_state.as_deref(), Some("stopped"));
        },
    );
}

#[test]
fn test_prepare_local_shutdown_lists_configured_and_extra_agents() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    std::fs::write(
        paths.ccbr_dir().join("ccbr.config"),
        "alpha:codex,beta:claude\n",
    )
    .unwrap();
    std::fs::create_dir_all(paths.agent_dir("alpha").as_std_path()).unwrap();
    std::fs::create_dir_all(paths.agent_dir("beta").as_std_path()).unwrap();
    std::fs::create_dir_all(paths.agent_dir("gamma").as_std_path()).unwrap();

    let preparation =
        prepare_local_shutdown(&paths, false, no_agent_pids, Some(no_authority_pids), None)
            .unwrap();

    assert_eq!(preparation.configured_agent_names, vec!["alpha", "beta"]);
    assert_eq!(preparation.extra_agent_names, vec!["gamma"]);
}

#[test]
fn test_prepare_local_shutdown_merges_pid_candidates() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let runtime = stopped_runtime("demo", &paths);
    AgentRuntimeStore::new(paths.clone())
        .save(&runtime)
        .unwrap();

    let authority_source: PathBuf = paths.project_root.as_std_path().join("ccbd.pid");
    let agent_source: PathBuf = paths.agent_dir("demo").as_std_path().join("runtime.pid");
    let mut control_plane: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    control_plane.insert(100, vec![authority_source.clone()]);

    let agent_source2 = agent_source.clone();
    let preparation = prepare_local_shutdown(
        &paths,
        false,
        move |agent_dir, _, _| {
            assert!(agent_dir.ends_with("agents/demo"));
            let mut map: HashMap<u32, Vec<PathBuf>> = HashMap::new();
            map.insert(200, vec![agent_source2.clone()]);
            map
        },
        None::<fn(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>>,
        Some(control_plane),
    )
    .unwrap();

    assert_eq!(preparation.control_plane_pids, vec![100]);
    assert_eq!(
        preparation.pid_candidates.get(&100).unwrap(),
        &vec![authority_source]
    );
    assert_eq!(
        preparation.pid_candidates.get(&200).unwrap(),
        &vec![agent_source]
    );
}

#[test]
fn test_prepare_local_shutdown_uses_project_authority_closure_when_no_override() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);

    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let seen_clone = seen.clone();
    let authority_source: PathBuf = paths.project_root.as_std_path().join("authority.pid");
    let authority_source2 = authority_source.clone();

    let preparation = prepare_local_shutdown(
        &paths,
        false,
        no_agent_pids,
        Some(move |project_root: &std::path::Path| {
            *seen_clone.lock().unwrap() = Some(project_root.to_path_buf());
            let mut map: HashMap<u32, Vec<PathBuf>> = HashMap::new();
            map.insert(42, vec![authority_source2.clone()]);
            map
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        *seen.lock().unwrap(),
        Some(paths.project_root.as_std_path().to_path_buf())
    );
    assert_eq!(preparation.control_plane_pids, vec![42]);
    assert_eq!(
        preparation.pid_candidates.get(&42).unwrap(),
        &vec![authority_source]
    );
}

#[test]
fn test_prepare_local_shutdown_passes_force_to_agent_pid_collector() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    let runtime = stopped_runtime("demo", &paths);
    AgentRuntimeStore::new(paths.clone())
        .save(&runtime)
        .unwrap();

    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let seen_clone = seen.clone();

    let preparation = prepare_local_shutdown(
        &paths,
        true,
        move |_, _, force| {
            *seen_clone.lock().unwrap() = Some(force);
            HashMap::new()
        },
        Some(no_authority_pids),
        None,
    )
    .unwrap();

    assert_eq!(*seen.lock().unwrap(), Some(true));
    assert_eq!(preparation.configured_agent_names, vec!["demo"]);
}

#[test]
fn test_prepare_local_shutdown_handles_configured_agent_without_runtime_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    std::fs::write(
        paths.ccbr_dir().join("ccbr.config"),
        "alpha:codex,beta:claude\n",
    )
    .unwrap();
    std::fs::create_dir_all(paths.agent_dir("alpha").as_std_path()).unwrap();

    let preparation = prepare_local_shutdown(
        &paths,
        false,
        |_, _, _| HashMap::new(),
        Some(no_authority_pids),
        None,
    )
    .unwrap();

    assert_eq!(preparation.configured_agent_names, vec!["alpha", "beta"]);
    assert_eq!(preparation.extra_agent_names, Vec::<String>::new());
}

#[test]
fn test_prepare_local_shutdown_falls_back_to_env_tmux_socket_when_no_runtime() {
    let tmp = tempfile::TempDir::new().unwrap();
    let paths = make_paths(&tmp);
    std::fs::write(paths.ccbr_dir().join("ccbr.config"), "alpha:codex\n").unwrap();

    with_env(
        &[
            ("TMUX", None),
            ("CCBR_TMUX_SOCKET", None),
            ("CCBR_TMUX_SOCKET_PATH", Some("/env/ccb.sock")),
        ],
        || {
            let preparation =
                prepare_local_shutdown(&paths, false, no_agent_pids, Some(no_authority_pids), None)
                    .unwrap();
            assert!(preparation
                .tmux_sockets
                .contains(&Some("/env/ccb.sock".into())));
        },
    );
}
