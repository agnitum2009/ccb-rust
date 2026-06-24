//! Mirrors Python `lib/cli/services/start.py`.
//!
//! High-level wrapper around `start_runtime::start_agents` that wires production
//! daemon lifecycle, workspace reconciliation, and maintenance heartbeat.

use crate::context::CliContext;
use crate::models_start::ParsedStartCommand;
use crate::services::maintenance::startup_ensure_maintenance_heartbeat;
use crate::services::start_runtime::{
    start_agents as start_agents_impl, JsonStartupReportStore, StartSummary,
};
use crate::services::{socket_path_for_project, DaemonClient, UnixDaemonClient};
use ccb_workspace::reconcile::WorkspaceGuardSummary;
use serde_json::{json, Map, Value};

/// Start project agents through the daemon.
///
/// Mirrors Python `cli.services.start.start_agents(context, command, *, terminal_size=None)`.
pub fn start_agents(
    context: &CliContext,
    command: &ParsedStartCommand,
    terminal_size: Option<(u32, u32)>,
) -> Result<StartSummary, String> {
    let ensure_daemon_started_fn = |ctx: &CliContext| -> Result<
        crate::services::daemon_runtime::models::DaemonHandle,
        crate::services::daemon_runtime::models::CcbdServiceError,
    > { crate::services::daemon::ensure_daemon_started(ctx) };

    let start_rpc_fn = |_: &crate::services::daemon_runtime::models::DaemonHandle,
                        timeout: Option<f64>| {
        let socket_path = socket_path_for_project(context.paths.project_root.as_std_path());
        let client = UnixDaemonClient::new(socket_path);
        let client = if let Some(t) = timeout {
            client.with_timeout(t)
        } else {
            client
        };

        let mut params = Map::new();
        params.insert(
            "agent_names".into(),
            Value::Array(
                command
                    .agent_names
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
        params.insert("restore".into(), Value::Bool(command.restore));
        params.insert(
            "auto_permission".into(),
            Value::Bool(command.auto_permission),
        );
        if let Some((cols, rows)) = terminal_size {
            params.insert("terminal_width".into(), Value::Number(cols.into()));
            params.insert("terminal_height".into(), Value::Number(rows.into()));
        }

        client
            .call("start", Value::Object(params))
            .map_err(|e| e.to_string())
    };

    let before_client_start_fn = |ctx: &CliContext| -> Result<WorkspaceGuardSummary, String> {
        let config_result = ccb_agents::config::load_project_config(&ctx.paths)
            .map_err(|e| format!("failed to load project config: {e}"))?;
        ccb_workspace::reconcile::reconcile_start_workspaces(
            &ctx.project.project_root,
            &config_result.config,
        )
        .map_err(|e| e.to_string())
    };

    let enrich_summary_fn =
        |mut summary: StartSummary, guard: WorkspaceGuardSummary| -> StartSummary {
            summary.worktree_warnings = guard
                .warnings
                .iter()
                .map(|w| {
                    json!({
                        "agent_name": w.agent_name,
                        "branch_name": w.branch_name,
                        "workspace_path": w.workspace_path,
                        "dirty": w.dirty,
                        "merged": w.merged,
                        "registered": w.registered,
                        "exists": w.exists,
                        "reason": w.reason,
                    })
                })
                .collect();
            summary.worktree_retired = guard
                .retired
                .iter()
                .map(|r| {
                    json!({
                        "agent_name": r.agent_name,
                        "branch_name": r.branch_name,
                        "workspace_path": r.workspace_path,
                        "reason": r.reason,
                        "removed_agent_state": r.removed_agent_state,
                    })
                })
                .collect();
            summary
        };

    let heartbeat_fn =
        |ctx: &CliContext| -> Option<Value> { startup_ensure_maintenance_heartbeat(ctx) };

    start_agents_impl(
        context,
        command,
        terminal_size,
        ensure_daemon_started_fn,
        start_rpc_fn,
        &JsonStartupReportStore,
        before_client_start_fn,
        enrich_summary_fn,
        heartbeat_fn,
    )
}
