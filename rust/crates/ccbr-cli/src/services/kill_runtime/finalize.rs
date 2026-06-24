//! Mirrors Python `lib/cli/services/kill_runtime/finalize.py`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_providers::helper_manifest::clear_helper_manifest;
use ccbr_storage::paths::PathLayout;

use super::agent_cleanup::KillPreparation;
use crate::services::daemon_runtime::models::KillSummary;
use crate::services::tmux_cleanup_history::TmuxCleanupEvent;
use crate::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;

/// Finalize the kill flow: clean up tmux orphans, terminate runtime PIDs, clear
/// helper manifests, record history, and return the final summary.
///
/// Mirrors Python `finalize_kill`.
#[allow(clippy::too_many_arguments)]
pub fn finalize_kill<F, G, H, I, J, K>(
    paths: &PathLayout,
    project_id: &str,
    force: bool,
    preparation: &KillPreparation,
    remote_summary: Option<&KillSummary>,
    summary: &KillSummary,
    set_tmux_ui_active_fn: F,
    cleanup_project_tmux_orphans_by_socket_fn: G,
    terminate_runtime_pids_fn: H,
    record_cleanup_event_fn: I,
    merge_cleanup_summaries_fn: J,
    record_kill_report_fn: K,
    clock_fn: fn() -> String,
) -> anyhow::Result<KillSummary>
where
    F: FnOnce(bool),
    G: FnOnce(&str, HashMap<Option<String>, Vec<String>>) -> Vec<ProjectTmuxCleanupSummary>,
    H: FnOnce(&Path, &HashMap<u32, Vec<PathBuf>>) -> anyhow::Result<()>,
    I: FnOnce(&TmuxCleanupEvent) -> anyhow::Result<()>,
    J: FnOnce(
        &[ProjectTmuxCleanupSummary],
        &[ProjectTmuxCleanupSummary],
    ) -> Vec<ProjectTmuxCleanupSummary>,
    K: FnOnce(&str, bool, &[ProjectTmuxCleanupSummary]) -> anyhow::Result<()>,
{
    set_tmux_ui_active_fn(false);

    let mut active_panes_by_socket: HashMap<Option<String>, Vec<String>> = HashMap::new();
    for socket in &preparation.tmux_sockets {
        active_panes_by_socket.entry(socket.clone()).or_default();
    }
    let cleanup_summaries =
        cleanup_project_tmux_orphans_by_socket_fn(project_id, active_panes_by_socket);

    terminate_runtime_pids_fn(
        paths.project_root.as_std_path(),
        &preparation.pid_candidates,
    )?;

    for agent_name in preparation
        .configured_agent_names
        .iter()
        .chain(preparation.extra_agent_names.iter())
    {
        clear_helper_manifest(&paths.agent_helper_path(agent_name));
    }

    if !cleanup_summaries.is_empty() {
        record_cleanup_event_fn(&TmuxCleanupEvent {
            event_kind: "kill".into(),
            project_id: project_id.into(),
            occurred_at: clock_fn(),
            summaries: cleanup_summaries.clone(),
        })?;
    }

    let remote_summaries = remote_summary
        .map(|s| s.cleanup_summaries.as_slice())
        .unwrap_or(&[]);
    let all_cleanup_summaries = merge_cleanup_summaries_fn(remote_summaries, &cleanup_summaries);

    let source = remote_summary.unwrap_or(summary);
    let final_summary = KillSummary {
        project_id: source.project_id.clone(),
        state: summary.state.clone(),
        socket_path: summary.socket_path.clone(),
        forced: force,
        cleanup_summaries: all_cleanup_summaries.clone(),
        worktree_warnings: Vec::new(),
    };

    let trigger = if remote_summary.is_some() {
        "kill"
    } else {
        "kill_fallback"
    };
    record_kill_report_fn(trigger, force, &all_cleanup_summaries)?;

    Ok(final_summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tmux_project_cleanup_runtime::models::ProjectTmuxCleanupSummary;
    use camino::Utf8PathBuf;
    use std::path::PathBuf;

    fn make_paths(tmp: &tempfile::TempDir) -> PathLayout {
        let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let paths = PathLayout::new(root);
        std::fs::create_dir_all(paths.ccbr_dir()).unwrap();
        paths
    }

    fn sample_summary(state: &str) -> KillSummary {
        KillSummary {
            project_id: "proj-1".into(),
            state: state.into(),
            socket_path: "/tmp/sock".into(),
            forced: false,
            cleanup_summaries: Vec::new(),
            worktree_warnings: Vec::new(),
        }
    }

    fn sample_preparation(paths: &PathLayout) -> KillPreparation {
        std::fs::create_dir_all(paths.agent_dir("demo").as_std_path()).unwrap();
        std::fs::create_dir_all(paths.agent_dir("extra").as_std_path()).unwrap();
        std::fs::write(paths.agent_helper_path("demo"), "{}").unwrap();
        std::fs::write(paths.agent_helper_path("extra"), "{}").unwrap();
        let mut pid_candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
        pid_candidates.insert(100, vec![paths.project_root.as_std_path().join("pidfile")]);
        KillPreparation {
            configured_agent_names: vec!["demo".into()],
            extra_agent_names: vec!["extra".into()],
            tmux_sockets: vec![Some("/tmp/ccbr.sock".into())],
            pid_candidates,
            control_plane_pids: vec![100],
        }
    }

    fn make_summary(socket: &str) -> ProjectTmuxCleanupSummary {
        ProjectTmuxCleanupSummary {
            socket_name: Some(socket.into()),
            owned_panes: vec!["%1".into()],
            active_panes: vec![],
            orphaned_panes: vec!["%1".into()],
            killed_panes: vec!["%1".into()],
        }
    }

    #[test]
    fn test_finalize_kill_merges_summaries_and_records_report() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let preparation = sample_preparation(&paths);

        let remote = KillSummary {
            cleanup_summaries: vec![make_summary("remote-sock")],
            ..sample_summary("unmounted")
        };
        let summary = sample_summary("unmounted");

        let ui_active = std::sync::Arc::new(std::sync::Mutex::new(None));
        let ui_active_clone = ui_active.clone();

        let received_project = std::sync::Arc::new(std::sync::Mutex::new(None));
        let received_project_clone = received_project.clone();

        let terminated = std::sync::Arc::new(std::sync::Mutex::new(None));
        let terminated_clone = terminated.clone();

        let recorded_event = std::sync::Arc::new(std::sync::Mutex::new(None));
        let recorded_event_clone = recorded_event.clone();

        let merged = std::sync::Arc::new(std::sync::Mutex::new(None));
        let merged_clone = merged.clone();

        let report = std::sync::Arc::new(std::sync::Mutex::new(None));
        let report_clone = report.clone();

        let local_summary = make_summary("local-sock");

        let result = finalize_kill(
            &paths,
            "proj-1",
            true,
            &preparation,
            Some(&remote),
            &summary,
            move |active| {
                *ui_active_clone.lock().unwrap() = Some(active);
            },
            move |project_id, active_map| {
                *received_project_clone.lock().unwrap() =
                    Some((project_id.to_string(), active_map.len()));
                vec![local_summary.clone()]
            },
            move |project_root, candidates| {
                *terminated_clone.lock().unwrap() =
                    Some((project_root.to_path_buf(), candidates.clone()));
                Ok(())
            },
            move |event| {
                *recorded_event_clone.lock().unwrap() = Some(event.clone());
                Ok(())
            },
            move |remote, local| {
                let mut out = remote.to_vec();
                out.extend_from_slice(local);
                *merged_clone.lock().unwrap() = Some((remote.len(), local.len()));
                out
            },
            move |trigger, forced, summaries| {
                *report_clone.lock().unwrap() =
                    Some((trigger.to_string(), forced, summaries.len()));
                Ok(())
            },
            || "2026-04-01T00:00:00Z".into(),
        )
        .unwrap();

        assert_eq!(*ui_active.lock().unwrap(), Some(false));
        assert_eq!(
            *received_project.lock().unwrap(),
            Some(("proj-1".into(), 1))
        );
        let (term_root, term_candidates) = terminated.lock().unwrap().clone().unwrap();
        assert_eq!(term_root, paths.project_root.as_std_path().to_path_buf());
        assert!(term_candidates.contains_key(&100));
        assert!(!paths.agent_helper_path("demo").exists());
        assert!(!paths.agent_helper_path("extra").exists());

        let event = recorded_event.lock().unwrap().clone().unwrap();
        assert_eq!(event.event_kind, "kill");
        assert_eq!(event.project_id, "proj-1");
        assert_eq!(event.summaries.len(), 1);

        assert_eq!(*merged.lock().unwrap(), Some((1, 1)));
        assert_eq!(result.cleanup_summaries.len(), 2);
        assert_eq!(*report.lock().unwrap(), Some(("kill".into(), true, 2)));
        assert_eq!(result.project_id, "proj-1");
        assert_eq!(result.state, "unmounted");
        assert!(result.forced);
    }

    #[test]
    fn test_finalize_kill_uses_fallback_trigger_without_remote_summary() {
        let tmp = tempfile::TempDir::new().unwrap();
        let paths = make_paths(&tmp);
        let preparation = sample_preparation(&paths);
        let summary = sample_summary("unmounted");

        let report = std::sync::Arc::new(std::sync::Mutex::new(None));
        let report_clone = report.clone();

        let _ = finalize_kill(
            &paths,
            "proj-1",
            false,
            &preparation,
            None,
            &summary,
            |_active| {},
            |_project_id, _active_map| Vec::new(),
            |_project_root, _candidates| Ok(()),
            |_event| Ok(()),
            |_remote, _local| Vec::new(),
            move |trigger, forced, _summaries| {
                *report_clone.lock().unwrap() = Some((trigger.to_string(), forced));
                Ok(())
            },
            || "2026-04-01T00:00:00Z".into(),
        )
        .unwrap();

        assert_eq!(
            *report.lock().unwrap(),
            Some(("kill_fallback".into(), false))
        );
    }
}
