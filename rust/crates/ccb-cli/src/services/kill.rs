//! Mirrors Python `lib/cli/services/kill.py`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_storage::paths::PathLayout;

use crate::context::CliContext;
use crate::kill_runtime::processes::{is_pid_alive, terminate_pid_tree, wait_for_pid_exit};
use crate::models::ParsedKillCommand;
use crate::services::daemon::{
    connect_mounted_daemon, inspect_daemon_phase, record_shutdown_intent,
};
use crate::services::daemon_runtime::models::KillSummary;
use crate::services::kill_runtime::agent_cleanup::{prepare_local_shutdown, KillPreparation};
use crate::services::kill_runtime::finalize::finalize_kill;
use crate::services::kill_runtime::lifecycle::destroy_project_namespace;
use crate::services::kill_runtime::pid_cleanup::{
    collect_agent_pid_candidates, collect_project_authority_pid_candidates,
    collect_project_process_candidates, path_within, pid_matches_project, read_proc_cmdline,
    read_proc_path, remove_pid_files, terminate_runtime_pids,
};
use crate::services::kill_runtime::remote::{
    await_remote_shutdown, request_remote_stop, resolve_shutdown_summary, StopAllClient,
};
use crate::services::kill_runtime::reporting::{merge_cleanup_summaries, record_kill_report};
use crate::services::maintenance::stop_maintenance_heartbeat_runner;
use crate::services::tmux_cleanup_history::TmuxCleanupHistoryStore;
use crate::services::tmux_project_cleanup::cleanup_project_tmux_orphans_by_socket;
use crate::services::tmux_ui::set_tmux_ui_active;
use crate::services::UnixDaemonClient;

const STOP_ALL_TIMEOUT_S: f64 = 12.0;

/// Summary of worktree guard warnings returned by `inspect_kill_worktrees`.
pub struct WorktreeGuardSummary {
    pub warnings: Vec<String>,
}

fn is_pid_alive_u32(pid: u32) -> bool {
    is_pid_alive(pid as i64)
}

fn terminate_pid_tree_u32_wrapper(
    pid: u32,
    timeout_s: f64,
    is_pid_alive_fn: &dyn Fn(u32) -> bool,
) -> bool {
    terminate_pid_tree(pid as i64, timeout_s, |p| is_pid_alive_fn(p as u32))
}

fn pid_matches_project_wrapper(pid: u32, project_root: &Path, hint_paths: &[PathBuf]) -> bool {
    pid_matches_project(
        pid,
        project_root,
        hint_paths,
        read_proc_path,
        read_proc_cmdline,
        path_within,
        std::env::consts::OS,
    )
}

fn terminate_runtime_pids_wrapper(
    project_root: &Path,
    pid_candidates: &HashMap<u32, Vec<PathBuf>>,
) -> anyhow::Result<()> {
    terminate_runtime_pids(
        project_root,
        pid_candidates,
        is_pid_alive_u32,
        pid_matches_project_wrapper,
        terminate_pid_tree_u32_wrapper,
        remove_pid_files,
        Some(collect_project_process_candidates),
    );
    Ok(())
}

/// Default project kill orchestration.
pub fn kill_project(
    context: &CliContext,
    command: &ParsedKillCommand,
) -> anyhow::Result<KillSummary> {
    kill_project_with(
        context,
        command,
        stop_maintenance_heartbeat_runner,
        collect_project_authority_pid_candidates,
        default_request_remote_stop,
        |paths, force, control_plane_pid_candidates| {
            prepare_local_shutdown(
                paths,
                force,
                collect_agent_pid_candidates,
                Some(collect_project_authority_pid_candidates),
                control_plane_pid_candidates,
            )
        },
        destroy_project_namespace,
        |ctx, remote, force, preparation| {
            default_resolve_shutdown_summary(ctx, remote, force, preparation)
        },
        |paths, project_id, force, preparation, remote, summary| {
            default_finalize_kill(paths, project_id, force, preparation, remote, summary)
        },
        |_project_root, _workspaces_dir| {},
        |_project_root| WorktreeGuardSummary {
            warnings: Vec::new(),
        },
    )
}

/// Kill orchestration with fully injected dependencies. Used by tests.
#[allow(clippy::too_many_arguments)]
pub fn kill_project_with<MH, A, R, P, D, S, F, P1, P2>(
    context: &CliContext,
    command: &ParsedKillCommand,
    stop_maintenance_heartbeat_runner_fn: MH,
    collect_project_authority_pid_candidates_fn: A,
    request_remote_stop_fn: R,
    prepare_local_shutdown_fn: P,
    destroy_project_namespace_fn: D,
    resolve_shutdown_summary_fn: S,
    finalize_kill_fn: F,
    prune_missing_worktrees_fn: P1,
    inspect_kill_worktrees_fn: P2,
) -> anyhow::Result<KillSummary>
where
    MH: FnOnce(&CliContext, &str),
    A: FnOnce(&Path) -> HashMap<u32, Vec<PathBuf>>,
    R: FnOnce(&CliContext, bool) -> anyhow::Result<Option<KillSummary>>,
    P: FnOnce(
        &PathLayout,
        bool,
        Option<HashMap<u32, Vec<PathBuf>>>,
    ) -> anyhow::Result<KillPreparation>,
    D: FnOnce(&PathLayout, &str, bool) -> anyhow::Result<()>,
    S: FnOnce(
        &CliContext,
        Option<&KillSummary>,
        bool,
        &KillPreparation,
    ) -> anyhow::Result<KillSummary>,
    F: FnOnce(
        &PathLayout,
        &str,
        bool,
        &KillPreparation,
        Option<&KillSummary>,
        &KillSummary,
    ) -> anyhow::Result<KillSummary>,
    P1: FnOnce(&Path, &Path),
    P2: FnOnce(&Path) -> WorktreeGuardSummary,
{
    stop_maintenance_heartbeat_runner_fn(context, "kill");

    let control_plane_pid_candidates = Some(collect_project_authority_pid_candidates_fn(
        context.paths.project_root.as_std_path(),
    ));
    let remote_summary = request_remote_stop_fn(context, command.force)?;
    let preparation =
        prepare_local_shutdown_fn(&context.paths, command.force, control_plane_pid_candidates)?;
    destroy_project_namespace_fn(&context.paths, &context.project.project_id, command.force)?;
    let summary = resolve_shutdown_summary_fn(
        context,
        remote_summary.as_ref(),
        command.force,
        &preparation,
    )?;
    let mut final_summary = finalize_kill_fn(
        &context.paths,
        &context.project.project_id,
        command.force,
        &preparation,
        remote_summary.as_ref(),
        &summary,
    )?;

    if command.force {
        prune_missing_worktrees_fn(
            context.paths.project_root.as_std_path(),
            context.paths.workspaces_dir().as_std_path(),
        );
    }

    let guard = inspect_kill_worktrees_fn(context.paths.project_root.as_std_path());
    if !guard.warnings.is_empty() {
        final_summary.worktree_warnings = guard
            .warnings
            .into_iter()
            .map(serde_json::Value::String)
            .collect();
    }

    Ok(final_summary)
}

fn default_request_remote_stop(
    context: &CliContext,
    force: bool,
) -> anyhow::Result<Option<KillSummary>> {
    request_remote_stop(
        context,
        force,
        connect_mounted_daemon,
        record_shutdown_intent,
        |socket_path, _timeout_s| {
            Ok(Box::new(UnixDaemonClient::new(
                socket_path.to_string_lossy().to_string(),
            )) as Box<dyn StopAllClient>)
        },
        STOP_ALL_TIMEOUT_S,
    )
}

fn default_resolve_shutdown_summary(
    context: &CliContext,
    remote_summary: Option<&KillSummary>,
    force: bool,
    preparation: &KillPreparation,
) -> anyhow::Result<KillSummary> {
    resolve_shutdown_summary(
        context,
        remote_summary,
        force,
        crate::kill_runtime::shutdown::shutdown_daemon,
        |ctx, force| default_await_remote_shutdown(ctx, force, preparation),
    )
}

fn default_await_remote_shutdown(
    context: &CliContext,
    force: bool,
    preparation: &KillPreparation,
) -> anyhow::Result<KillSummary> {
    await_remote_shutdown(
        context,
        force,
        STOP_ALL_TIMEOUT_S,
        &preparation.control_plane_pids,
        |_ctx| inspect_daemon_phase(context),
        is_pid_alive_u32,
        |pid, timeout, is_alive| terminate_pid_tree_u32_wrapper(pid, timeout, &is_alive),
        |pid, timeout| wait_for_pid_exit(pid as i64, timeout, &|p| is_pid_alive(p)),
        |_timeout| true,
    )
}

fn default_finalize_kill(
    paths: &PathLayout,
    project_id: &str,
    force: bool,
    preparation: &KillPreparation,
    remote_summary: Option<&KillSummary>,
    summary: &KillSummary,
) -> anyhow::Result<KillSummary> {
    let backend = ccb_terminal::backend::TmuxBackend::new(None, None);
    let current_pane_id = std::env::var("TMUX_PANE").ok().filter(|s| !s.is_empty());

    finalize_kill(
        paths,
        project_id,
        force,
        preparation,
        remote_summary,
        summary,
        set_tmux_ui_active,
        |project_id, active_panes_by_socket| {
            cleanup_project_tmux_orphans_by_socket(
                project_id,
                active_panes_by_socket,
                &backend,
                current_pane_id.as_deref(),
            )
        },
        terminate_runtime_pids_wrapper,
        |event| {
            TmuxCleanupHistoryStore::new(paths.clone()).append(event)?;
            Ok(())
        },
        merge_cleanup_summaries,
        |trigger, forced, summaries| {
            record_kill_report(
                paths,
                trigger,
                forced,
                summaries,
                &preparation.configured_agent_names,
                &preparation.extra_agent_names,
            )
        },
        || chrono::Utc::now().to_rfc3339(),
    )
}
