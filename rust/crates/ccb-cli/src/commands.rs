use crate::context::CliContext;
use crate::parser::{
    FaultAction, ParsedAck, ParsedAsk, ParsedAutonew, ParsedCancel, ParsedClear,
    ParsedConfigValidate, ParsedCtxTransfer, ParsedDoctor, ParsedFault, ParsedInbox, ParsedLogs,
    ParsedMaintenance, ParsedPend, ParsedQueue, ParsedReload, ParsedRepair, ParsedRestart,
    ParsedResubmit, ParsedRetry, ParsedRoles, ParsedStart, ParsedTools, ParsedTrace, ParsedWait,
    ParsedWatch, RepairAction, RolesAction, ToolsAction,
};
use crate::render::{
    render_ack, render_agent_status, render_ask_receipt, render_attach, render_cancel,
    render_cleanup, render_clear, render_config_validate, render_doctor, render_inbox, render_logs,
    render_maintenance, render_pend, render_ping, render_project_view, render_queue, render_reload,
    render_restart, render_resubmit, render_retry, render_roles, render_shutdown, render_start,
    render_stop, render_tools, render_trace, render_wait_ready, render_watch, ProjectView,
};
use crate::services::{socket_path_for_project, DaemonClient};
use ccb_terminal::backend::TerminalBackend;
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, Instant};

/// Start project agents through the daemon.
pub fn start(client: &dyn DaemonClient, cmd: &ParsedStart) -> Result<String, String> {
    let params = serde_json::json!({
        "agent_names": cmd.agent_names,
        "restore": cmd.restore,
        "auto_permission": cmd.auto_permission,
    });
    let result = client.call("start", params)?;
    Ok(render_start(&result))
}

/// Stop all agents (and optionally force cleanup).
pub fn stop(client: &dyn DaemonClient, force: bool) -> Result<String, String> {
    let params = serde_json::json!({"force": force});
    let result = client.call("stop-all", params)?;
    Ok(render_stop(&result))
}

/// Show project status / project view.
pub fn status(client: &dyn DaemonClient) -> Result<String, String> {
    let result = client.call("project_view", serde_json::json!({"schema_version": 1}))?;
    let view: ProjectView =
        serde_json::from_value(result).map_err(|e| format!("invalid project view: {}", e))?;
    Ok(render_project_view(&view))
}

/// Show compact agent status (`ps`).
pub fn ps(client: &dyn DaemonClient, _alive_only: bool) -> Result<String, String> {
    let result = client.call("project_view", serde_json::json!({"schema_version": 1}))?;
    let view: ProjectView =
        serde_json::from_value(result).map_err(|e| format!("invalid project view: {}", e))?;
    Ok(render_agent_status(&view.agents))
}

/// Attach an external runtime/agent to the daemon.
pub fn attach(
    client: &dyn DaemonClient,
    agent_name: &str,
    project_root: &Path,
) -> Result<String, String> {
    let workspace = project_root.to_string_lossy().to_string();
    let params = serde_json::json!({
        "agent_name": agent_name,
        "workspace_path": workspace,
        "backend_type": "tmux",
    });
    let result = client.call("attach", params)?;
    Ok(render_attach(&result))
}

/// Submit an ask message.
pub fn ask(client: &dyn DaemonClient, cmd: &ParsedAsk, project_id: &str) -> Result<String, String> {
    if cmd.target.is_empty() {
        return Err("ask requires a target agent".to_string());
    }
    if cmd.message.is_empty() {
        return Err("ask requires a message".to_string());
    }
    let from = cmd.sender.clone().unwrap_or_else(|| "user".to_string());
    let params = serde_json::json!({
        "project_id": project_id,
        "to_agent": cmd.target,
        "from_actor": from,
        "body": cmd.message,
        "task_id": cmd.task_id,
    });
    // TODO(phase2-protocol): align with Python v7.5.2 by switching to `submit`
    // once dispatcher/async delivery matches the Python semantics.
    let result = client.call("ask", params)?;
    Ok(render_ask_receipt(&result))
}

/// Ping a target (agent or `ccbd`).
pub fn ping(client: &dyn DaemonClient, target: &str) -> Result<String, String> {
    let params = serde_json::json!({"target": target});
    let result = client.call("ping", params)?;
    Ok(render_ping(target, &result))
}

/// Request daemon shutdown.
pub fn shutdown(client: &dyn DaemonClient) -> Result<String, String> {
    let result = client.call("shutdown", serde_json::json!({}))?;
    Ok(render_shutdown(&result))
}

/// Wait for a target to become ready.
pub fn wait(client: &dyn DaemonClient, cmd: &ParsedWait) -> Result<String, String> {
    if cmd.target.is_empty() {
        return Err("wait requires a target".to_string());
    }
    let timeout = cmd.timeout_s.unwrap_or(60.0);
    let interval = Duration::from_millis(500);
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < timeout {
        if target_ready(client, &cmd.target)? {
            return Ok(render_wait_ready(&cmd.target));
        }
        std::thread::sleep(interval);
    }
    Err(format!("timeout waiting for {}", cmd.target))
}

fn target_ready(client: &dyn DaemonClient, target: &str) -> Result<bool, String> {
    if target == "ccbd" {
        let result = client.call("ping", serde_json::json!({"target": "ccbd"}))?;
        return Ok(result
            .get("pong")
            .and_then(|v| v.as_bool())
            .unwrap_or(false));
    }
    let result = client.call("project_view", serde_json::json!({"schema_version": 1}))?;
    let view: ProjectView =
        serde_json::from_value(result).map_err(|e| format!("invalid project view: {}", e))?;
    if target == "all" {
        return if view.agents.is_empty() {
            Ok(false)
        } else {
            Ok(view
                .agents
                .iter()
                .all(|a| !a.state.is_empty() && a.state != "starting"))
        };
    }
    Ok(view
        .agents
        .iter()
        .any(|a| a.name == target && !a.state.is_empty() && a.state != "starting"))
}

/// Watch a target's output stream, polling until terminal or timeout.
///
/// Mirrors `services.watch_runtime.watch_target`: repeatedly calls the daemon
/// `watch` RPC with an advancing `start_line` cursor, accumulating new output
/// until the batch is `terminal` or the wall-clock timeout elapses. Timeout and
/// poll interval are env-overridable (`CCB_WATCH_TIMEOUT_S`, default 10s;
/// `CCB_WATCH_POLL_INTERVAL_S`, default 0.1s).
pub fn watch(client: &dyn DaemonClient, cmd: &ParsedWatch) -> Result<String, String> {
    if cmd.target.is_empty() {
        return Err("watch requires a target".to_string());
    }
    let timeout = Duration::from_secs_f64(watch_timeout_s());
    let interval = Duration::from_secs_f64(watch_poll_interval_s());
    let deadline = Instant::now() + timeout;
    let mut cursor: u64 = 0;
    let mut out = String::new();
    loop {
        let result = client.call(
            "watch",
            serde_json::json!({"target": cmd.target, "start_line": cursor}),
        )?;
        let new_cursor = result
            .get("cursor")
            .and_then(|v| v.as_u64())
            .unwrap_or(cursor);
        let terminal = result
            .get("terminal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if new_cursor > cursor {
            out.push_str(&render_watch(&result));
        }
        cursor = cursor.max(new_cursor);
        if terminal || Instant::now() >= deadline {
            return Ok(out);
        }
        std::thread::sleep(interval);
    }
}

fn watch_timeout_s() -> f64 {
    std::env::var("CCB_WATCH_TIMEOUT_S")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(10.0)
}

fn watch_poll_interval_s() -> f64 {
    std::env::var("CCB_WATCH_POLL_INTERVAL_S")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(0.1)
}

/// Cancel a job.
pub fn cancel(client: &dyn DaemonClient, cmd: &ParsedCancel) -> Result<String, String> {
    if cmd.job_id.is_empty() {
        return Err("cancel requires a job_id".to_string());
    }
    let params = serde_json::json!({"job_id": cmd.job_id});
    let result = client.call("cancel", params)?;
    Ok(render_cancel(&result))
}

/// Clear agent context(s).
pub fn clear(client: &dyn DaemonClient, cmd: &ParsedClear) -> Result<String, String> {
    let names: Vec<String> = cmd.agent_names.clone();
    let params = serde_json::json!({"agent_names": names});
    let result = client.call("project_clear_context", params)?;
    Ok(render_clear(&result))
}

/// Show the message queue for a target.
pub fn queue(client: &dyn DaemonClient, cmd: &ParsedQueue) -> Result<String, String> {
    let target = if cmd.target.is_empty() {
        "all".to_string()
    } else {
        cmd.target.clone()
    };
    let params = serde_json::json!({"target": target, "detail": cmd.detail});
    let result = client.call("queue", params)?;
    Ok(render_queue(&result))
}

/// Trace jobs for a target.
pub fn trace(client: &dyn DaemonClient, cmd: &ParsedTrace) -> Result<String, String> {
    let target = if cmd.target.is_empty() {
        "all".to_string()
    } else {
        cmd.target.clone()
    };
    let params = serde_json::json!({"target": target});
    let result = client.call("trace", params)?;
    Ok(render_trace(&result))
}

/// Resubmit a message.
pub fn resubmit(client: &dyn DaemonClient, cmd: &ParsedResubmit) -> Result<String, String> {
    if cmd.message_id.is_empty() {
        return Err("resubmit requires a message_id".to_string());
    }
    let params = serde_json::json!({"message_id": cmd.message_id});
    let result = client.call("resubmit", params)?;
    Ok(render_resubmit(&result))
}

/// Retry a target.
pub fn retry(client: &dyn DaemonClient, cmd: &ParsedRetry) -> Result<String, String> {
    let target = if cmd.target.is_empty() {
        "all".to_string()
    } else {
        cmd.target.clone()
    };
    let params = serde_json::json!({"target": target});
    let result = client.call("retry", params)?;
    Ok(render_retry(&result))
}

/// Show an agent's inbox.
pub fn inbox(client: &dyn DaemonClient, cmd: &ParsedInbox) -> Result<String, String> {
    if cmd.agent_name.is_empty() {
        return Err("inbox requires an agent_name".to_string());
    }
    let params = serde_json::json!({"agent_name": cmd.agent_name, "detail": cmd.detail});
    let result = client.call("inbox", params)?;
    Ok(render_inbox(&result))
}

/// Acknowledge an inbound event.
pub fn ack(client: &dyn DaemonClient, cmd: &ParsedAck) -> Result<String, String> {
    if cmd.agent_name.is_empty() {
        return Err("ack requires an agent_name".to_string());
    }
    let params = serde_json::json!({
        "agent_name": cmd.agent_name,
        "event_id": cmd.event_id,
    });
    let result = client.call("ack", params)?;
    Ok(render_ack(&result))
}

/// Reload configuration.
pub fn reload(client: &dyn DaemonClient, cmd: &ParsedReload) -> Result<String, String> {
    let params = serde_json::json!({"dry_run": cmd.dry_run});
    let result = client.call("project_reload_config", params)?;
    Ok(render_reload(&result))
}

/// Restart an agent.
pub fn restart(client: &dyn DaemonClient, cmd: &ParsedRestart) -> Result<String, String> {
    if cmd.agent_name.is_empty() {
        return Err("restart requires an agent_name".to_string());
    }
    let params = serde_json::json!({"agent_name": cmd.agent_name});
    let result = client.call("project_restart_agent", params)?;
    Ok(render_restart(&result))
}

/// Maintenance operations.
pub fn maintenance(
    client: &dyn DaemonClient,
    cmd: &ParsedMaintenance,
    context: &CliContext,
) -> Result<String, String> {
    let result = match cmd.action.as_str() {
        "status" => crate::services::maintenance::maintenance_status(context),
        "tick" => {
            let force = cmd.args.iter().any(|a| a == "--force");
            let no_dispatch = cmd.args.iter().any(|a| a == "--no-dispatch");
            let now = crate::services::maintenance::utc_now_iso();
            crate::services::maintenance::maintenance_tick(
                context,
                client,
                force,
                no_dispatch,
                &now,
            )
        }
        "schedule" => {
            let after = parse_schedule_after(&cmd.args)
                .unwrap_or(crate::services::maintenance::DEFAULT_INTERVAL_S);
            let reason =
                parse_schedule_reason(&cmd.args).unwrap_or_else(|| "manual_schedule".to_string());
            let now = crate::services::maintenance::utc_now_iso();
            crate::services::maintenance::maintenance_schedule(context, after, &reason, &now)
        }
        "runner" => {
            let runner_id = parse_runner_id(&cmd.args)
                .unwrap_or_else(|| format!("ccb-runner-{}", std::process::id()));
            let max_iterations = parse_max_iterations(&cmd.args).unwrap_or(0);
            let sleep_cap_s = parse_sleep_cap(&cmd.args)
                .unwrap_or(crate::services::maintenance::DEFAULT_RUNNER_SLEEP_CAP_S);
            let no_dispatch = cmd.args.iter().any(|a| a == "--no-dispatch");
            let now = crate::services::maintenance::utc_now_iso();
            crate::services::maintenance::maintenance_runner(
                context,
                client,
                &runner_id,
                max_iterations,
                sleep_cap_s,
                no_dispatch,
                &now,
            )
        }
        other => return Err(format!("maintenance action '{}' not supported", other)),
    };
    Ok(render_maintenance(&result))
}

fn parse_schedule_after(args: &[String]) -> Option<u32> {
    args.windows(2)
        .find(|w| w[0] == "--after")
        .and_then(|w| w[1].parse::<u32>().ok())
}

fn parse_schedule_reason(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == "--reason")
        .map(|w| w[1].clone())
}

fn parse_runner_id(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == "--runner-id")
        .map(|w| w[1].clone())
}

fn parse_max_iterations(args: &[String]) -> Option<u32> {
    args.windows(2)
        .find(|w| w[0] == "--max-iterations")
        .and_then(|w| w[1].parse::<u32>().ok())
}

fn parse_sleep_cap(args: &[String]) -> Option<u64> {
    args.windows(2)
        .find(|w| w[0] == "--sleep-cap")
        .and_then(|w| w[1].trim_end_matches('s').parse::<u64>().ok())
}

/// Show recent logs for an agent.
pub fn logs(client: &dyn DaemonClient, cmd: &ParsedLogs) -> Result<String, String> {
    if cmd.agent_name.is_empty() {
        return Err("logs requires an agent_name".to_string());
    }
    let params = serde_json::json!({"agent_name": cmd.agent_name, "tail": 50});
    let result = client.call("logs", params)?;
    Ok(render_logs(&result))
}

/// Clean up workspace artifacts and orphaned job records.
pub fn cleanup(client: &dyn DaemonClient) -> Result<String, String> {
    let params = serde_json::json!({"dry_run": false});
    let result = client.call("cleanup", params)?;
    Ok(render_cleanup(&result))
}

/// Run a basic diagnostic check on the project.
pub fn doctor(
    client: &dyn DaemonClient,
    cmd: &ParsedDoctor,
    project_root: &Path,
) -> Result<String, String> {
    let layout = ccb_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root).unwrap_or(camino::Utf8Path::new("/")),
    );

    let mut daemon_ok = false;
    let mut daemon_error = None;
    match client.call("ping", serde_json::json!({"target": "ccbd"})) {
        Ok(result) => {
            daemon_ok = result
                .get("pong")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }
        Err(err) => daemon_error = Some(err),
    }

    let tmux_socket_path = layout.ccbd_tmux_socket_path();
    let tmux_present = tmux_socket_path.exists();

    let config_result = ccb_agents::config::load_project_config(&layout);
    let config_ok = config_result.is_ok();
    let config_error = config_result.err().map(|e| e.to_string());

    let mut issues: Vec<String> = Vec::new();
    if !daemon_ok {
        issues.push(format!(
            "daemon not reachable{}; run `ccb start`",
            daemon_error.map(|e| format!(": {e}")).unwrap_or_default()
        ));
    }
    if !tmux_present {
        issues.push("tmux socket not present; agents may not be running".to_string());
    }
    if !config_ok {
        issues.push(format!(
            "project config invalid{}",
            config_error.map(|e| format!(": {e}")).unwrap_or_default()
        ));
    }

    let summary = serde_json::json!({
        "project_root": project_root.to_string_lossy().to_string(),
        "project_id": layout.project_id(),
        "daemon_ok": daemon_ok,
        "tmux_present": tmux_present,
        "config_ok": config_ok,
        "issues": issues,
    });

    if cmd.json_output {
        return serde_json::to_string_pretty(&summary).map_err(|e| e.to_string());
    }
    Ok(render_doctor(&summary))
}

/// Validate the project configuration.
pub fn config_validate(_cmd: &ParsedConfigValidate, project_root: &Path) -> Result<String, String> {
    let layout = ccb_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root).unwrap_or(camino::Utf8Path::new("/")),
    );
    let result = ccb_agents::config::load_project_config(&layout)
        .map_err(|e| format!("configuration invalid: {e}"))?;
    let summary = serde_json::json!({
        "project_root": project_root.to_string_lossy().to_string(),
        "project_id": layout.project_id(),
        "source_path": result.source_path.map(|p| p.to_string()),
        "source_kind": result.source_kind,
        "used_default": result.used_default,
        "agent_count": result.config.agents.len(),
        "default_agents": result.config.default_agents,
        "layout_spec": result.config.layout_spec.unwrap_or_default(),
        "cmd_enabled": result.config.cmd_enabled,
    });
    Ok(render_config_validate(&summary))
}

/// Show pending state for a target.
pub fn pend(client: &dyn DaemonClient, cmd: &ParsedPend) -> Result<String, String> {
    if cmd.target.is_empty() {
        return Err("pend requires a target".to_string());
    }

    let result = if cmd.target.starts_with("job_") {
        client.call("get", serde_json::json!({"job_id": cmd.target}))?
    } else if cmd.queue {
        client.call(
            "queue",
            serde_json::json!({"target": cmd.target, "detail": cmd.detail}),
        )?
    } else if cmd.inbox {
        client.call(
            "inbox",
            serde_json::json!({"agent_name": cmd.target, "detail": cmd.detail}),
        )?
    } else {
        client.call("get", serde_json::json!({"agent_name": cmd.target}))?
    };

    Ok(render_pend(&cmd.target, &result))
}

/// Tool management stubs.
pub fn tools(cmd: &ParsedTools) -> Result<String, String> {
    match &cmd.action {
        ToolsAction::Doctor { tool } => {
            if tool == "neovim" {
                let status = crate::tools_runtime::neovim_status();
                Ok(crate::tools_runtime::render_neovim_status(&status))
            } else {
                Ok(render_tools(
                    tool,
                    "doctor",
                    "unsupported tool (supported: neovim)",
                ))
            }
        }
        ToolsAction::Install { tool } => Ok(render_tools(
            tool,
            "install",
            "guided: use the CCB install script (release-tarball downloader is not bundled in this build)",
        )),
    }
}

/// Role management: list, install, update, sync, doctor, add.
pub fn roles(cmd: &ParsedRoles, project_root: &Path) -> Result<String, String> {
    match &cmd.action {
        RolesAction::List => {
            let installed_root = ccb_agents::rolepacks::agent_roles_installed_root();
            let mut roles: Vec<ccb_agents::rolepacks::RoleManifest> = Vec::new();
            if installed_root.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&installed_root) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Ok(manifest) = ccb_agents::rolepacks::load_role_manifest(&path) {
                                roles.push(manifest);
                            }
                        }
                    }
                }
            }
            roles.sort_by(|a, b| a.id.cmp(&b.id));
            let summaries: Vec<serde_json::Map<String, serde_json::Value>> = roles
                .iter()
                .map(|r| {
                    let mut m = serde_json::Map::new();
                    m.insert("id".into(), r.id.clone().into());
                    m.insert("name".into(), r.name.clone().into());
                    m.insert("version".into(), r.version.clone().into());
                    m.insert("description".into(), r.description.clone().into());
                    m.insert(
                        "providers".into(),
                        serde_json::Value::Array(
                            r.providers().into_iter().map(|p| p.into()).collect(),
                        ),
                    );
                    m
                })
                .collect();
            Ok(render_roles(&serde_json::json!({
                "status": "ok",
                "roles": summaries,
            })))
        }
        RolesAction::Add { spec } => {
            let (role_id, provider) = match spec.split_once(':') {
                Some((r, p)) => (r.to_string(), Some(p.to_string())),
                None => (spec.clone(), None),
            };
            let payload = ccb_agents::rolepacks::add_role_to_project_config(
                project_root,
                &role_id,
                None,
                provider.as_deref(),
                None,
            )
            .map_err(|e| e.to_string())?;
            Ok(render_roles(&serde_json::Value::Object(payload)))
        }
        RolesAction::Update { path } => {
            let role_id = if path.is_empty() {
                None
            } else {
                Some(path.as_str())
            };
            let payload = ccb_agents::rolepacks::update_role(role_id, None, None, true)
                .map_err(|e| e.to_string())?;
            Ok(render_roles(&serde_json::Value::Object(payload)))
        }
        RolesAction::Install { path } => {
            let role_id = if path.is_empty() {
                None
            } else {
                Some(path.as_str())
            };
            let payload = ccb_agents::rolepacks::install_role(role_id, None, None, true)
                .map_err(|e| e.to_string())?;
            Ok(render_roles(&serde_json::Value::Object(payload)))
        }
        RolesAction::Sync { path } => {
            let sync_path = path.as_deref().unwrap_or(".");
            let payload =
                ccb_agents::rolepacks::sync_roles_from_path(std::path::Path::new(sync_path), false)
                    .map_err(|e| e.to_string())?;
            Ok(render_roles(&serde_json::Value::Object(payload)))
        }
        RolesAction::Doctor { path } => {
            let payload =
                ccb_agents::rolepacks::role_status(path, true).map_err(|e| e.to_string())?;
            Ok(render_roles(&serde_json::Value::Object(payload)))
        }
    }
}

/// Fault injection control.
pub fn fault(client: &dyn DaemonClient, cmd: &ParsedFault) -> Result<String, String> {
    match &cmd.action {
        FaultAction::List => {
            let result = client.call("fault_list", serde_json::json!({}))?;
            Ok(render_roles(&result)) // generic record renderer
        }
        FaultAction::Arm {
            agent_name,
            task_id,
            reason,
            count,
            error,
        } => {
            if agent_name.is_empty() {
                return Err("fault arm requires <agent_name>".to_string());
            }
            if task_id.is_empty() {
                return Err("fault arm requires --task-id".to_string());
            }
            let result = client.call(
                "fault_arm",
                serde_json::json!({
                    "agent_name": agent_name,
                    "task_id": task_id,
                    "reason": reason.as_deref().unwrap_or("api_error"),
                    "count": count,
                    "error_message": error.as_deref().unwrap_or("fault injection drill"),
                }),
            )?;
            Ok(render_roles(&result))
        }
        FaultAction::Clear { target } => {
            if target.is_empty() {
                return Err("fault clear requires <rule_id|all>".to_string());
            }
            let result = client.call("fault_clear", serde_json::json!({"target": target}))?;
            Ok(render_roles(&result))
        }
    }
}

/// Repair aliases for ack/retry/resubmit.
pub fn repair(client: &dyn DaemonClient, cmd: &ParsedRepair) -> Result<String, String> {
    match &cmd.action {
        RepairAction::Ack { target, event_id } => {
            if target.is_empty() {
                return Err("repair ack requires <agent_name>".to_string());
            }
            let result = client.call(
                "ack",
                serde_json::json!({
                    "agent_name": target,
                    "event_id": event_id,
                }),
            )?;
            Ok(render_ack(&result))
        }
        RepairAction::Retry { target } => {
            let result = client.call("retry", serde_json::json!({"target": target}))?;
            Ok(render_retry(&result))
        }
        RepairAction::Resubmit { target } => {
            let result = client.call("resubmit", serde_json::json!({"message_id": target}))?;
            Ok(render_resubmit(&result))
        }
    }
}

/// Print version information.
pub fn version() -> Result<String, String> {
    Ok(format!("ccb {}\n", crate::entry::VERSION))
}

/// Check for CCB updates against the published release index.
///
/// Mirrors the version-check portion of `management_runtime.commands_runtime.update.cmd_update`.
/// The tarball download/staged install itself is delegated to the install script
/// (the `install.py` installer is a separate translation); this command resolves the
/// current vs. latest version and reports update availability.
pub fn update() -> Result<String, String> {
    let current = crate::entry::VERSION;
    let versions = crate::versioning::get_available_versions();
    if versions.is_empty() {
        return Ok(format!(
            "ccb update: could not reach the release index (network/git unavailable); current v{current}.\n\
             Re-run later or apply updates via the CCB install script.\n"
        ));
    }
    let latest = crate::versioning::latest_version(&versions);
    Ok(match latest {
        Some(l) if crate::versioning::is_newer_version(&l, current) => format!(
            "📦 Update available: v{current} → v{l}\n\
             Apply with the CCB install script (release-tarball download/install is not bundled in this build).\n"
        ),
        Some(l) => format!("✅ Up to date (v{current}; latest tagged release v{l})\n"),
        None => format!(
            "ccb update: could not determine the latest version (current v{current})\n"
        ),
    })
}

/// Uninstall stub.
pub fn uninstall() -> Result<String, String> {
    Ok("ccb uninstall: not implemented in this build; remove the CCB installation directory and ~/.ccbr entries manually.\n".to_string())
}

/// Reinstall stub.
pub fn reinstall() -> Result<String, String> {
    Ok(
        "ccb reinstall: not implemented in this build; run the CCB install script to reinstall.\n"
            .to_string(),
    )
}

/// Send /new (or provider-specific reset command) to a provider pane.
pub fn autonew(cmd: &ParsedAutonew, project_root: &Path) -> Result<String, String> {
    let provider = cmd.provider.trim().to_lowercase();
    if provider.is_empty() || provider == "-h" || provider == "--help" {
        return Ok(autonew_usage());
    }

    let reset_cmd = match provider.as_str() {
        "gemini" => "/clear",
        "codex" | "opencode" | "droid" | "claude" => "/new",
        _ => {
            return Err(format!(
                "[ERROR] Unknown provider: {provider}\n\
                 [ERROR] Available: gemini, codex, opencode, droid, claude"
            ));
        }
    };

    let session_filename = match provider.as_str() {
        "claude" => ".claude-session",
        "codex" => ".codex-session",
        "gemini" => ".gemini-session",
        "opencode" => ".opencode-session",
        "droid" => ".droid-session",
        _ => unreachable!(),
    };

    let session_file =
        ccb_provider_sessions::files::find_project_session_file(project_root, session_filename)
            .ok_or_else(|| {
                format!("[ERROR] No active {provider} session found for this project.")
            })?;

    let raw = std::fs::read_to_string(&session_file)
        .map_err(|e| format!("[ERROR] Failed to read session file: {e}"))?;
    let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| format!("[ERROR] Failed to parse session file: {e}"))?;

    let pane_id = data
        .get("pane_id")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("[ERROR] No pane_id found for {provider}."))?;

    let socket_name = data
        .get("tmux_socket_name")
        .and_then(|v| v.as_str())
        .map(String::from);
    let socket_path = data
        .get("tmux_socket_path")
        .and_then(|v| v.as_str())
        .map(String::from);

    let backend = ccb_terminal::backend::TmuxBackend::new(socket_name, socket_path);

    if !backend
        .is_alive(pane_id)
        .map_err(|e| format!("[ERROR] Failed to check pane status: {e}"))?
    {
        return Err(format!("[ERROR] {provider} pane {pane_id} is not alive."));
    }

    backend
        .send_text(pane_id, reset_cmd)
        .map_err(|e| format!("[ERROR] Failed to send {reset_cmd}: {e}"))?;

    Ok(format!(
        "Sent {reset_cmd} to {provider} (pane: {pane_id})\n"
    ))
}

fn autonew_usage() -> String {
    "Usage: autonew <provider>\n\n\
     Providers:\n\
       gemini, codex, opencode, droid, claude\n\n\
     Sends /new to the provider's pane to start a new session.\n"
        .to_string()
}

/// Transfer conversation context between CCB agents.
pub fn ctx_transfer(cmd: &ParsedCtxTransfer, project_root: &Path) -> Result<String, String> {
    use crate::services::UnixDaemonClient;

    let transfer = ccb_memory::ContextTransfer::new(cmd.max_tokens as u32, project_root);
    let session_path = cmd.session_path.as_deref().map(Path::new);
    let context = transfer
        .extract_conversations(
            session_path,
            cmd.last,
            true,
            &cmd.source_provider,
            None,
            None,
        )
        .map_err(|e| format!("Session not found: {e}"))?;

    if context.conversations.is_empty() {
        return Err("No conversations found in session.".to_string());
    }

    let provider_label = context.source_provider.trim().to_lowercase();
    let provider_label = if provider_label.is_empty() {
        "auto"
    } else {
        provider_label.as_str()
    };
    let info = format!(
        "Extracted {} conversation(s) (~{} tokens) from {provider_label}\n",
        context.conversations.len(),
        context.token_estimate
    );

    let formatted = transfer.format_output(&context, &cmd.format, cmd.detailed);

    if cmd.dry_run {
        return Ok(formatted);
    }

    if let Some(output_path) = &cmd.output {
        let output_path = Path::new(output_path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(output_path, &formatted).map_err(|e| e.to_string())?;
        if cmd.quiet {
            return Ok(String::new());
        }
        return Ok(format!("Written to {}\n", output_path.display()));
    }

    if !cmd.send {
        let saved_path = transfer
            .save_transfer(&context, &cmd.format, None, None)
            .map_err(|e| e.to_string())?;
        if cmd.quiet {
            return Ok(String::new());
        }
        return Ok(format!("Saved to {}\n", saved_path.display()));
    }

    let agent_name = cmd
        .agent_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "--send requires --agent <agent_name>".to_string())?;

    let should_save = cmd.save || !cmd.no_save;
    let mut result = info;
    if should_save {
        let saved_path = transfer
            .save_transfer(&context, &cmd.format, Some(agent_name), None)
            .map_err(|e| e.to_string())?;
        if !cmd.quiet {
            result.push_str(&format!("Saved to {}\n", saved_path.display()));
        }
    }

    let socket_path = socket_path_for_project(project_root);
    let client = UnixDaemonClient::new(socket_path);
    let layout = ccb_storage::paths::PathLayout::new(
        camino::Utf8Path::from_path(project_root).unwrap_or(camino::Utf8Path::new("/")),
    );
    let ask_cmd = ParsedAsk {
        project: cmd.project.clone(),
        target: agent_name.to_string(),
        sender: Some("ctx-transfer".to_string()),
        message: formatted,
        task_id: None,
        compact: false,
        silence: cmd.quiet,
    };

    match ask(&client, &ask_cmd, layout.project_id()) {
        Ok(reply) => {
            if !cmd.quiet {
                result.push_str(&format!("Sent to {agent_name}\n"));
            }
            result.push_str(&reply);
            Ok(result)
        }
        Err(err) => Err(format!("Failed to send: {err}")),
    }
}

/// Helper to extract a string field from a JSON payload.
pub fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|v| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonew_rejects_unknown_provider() {
        let cmd = ParsedAutonew {
            project: None,
            provider: "bogus".to_string(),
        };
        let result = autonew(&cmd, Path::new("/tmp"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn autonew_returns_usage_for_help() {
        let cmd = ParsedAutonew {
            project: None,
            provider: "-h".to_string(),
        };
        let result = autonew(&cmd, Path::new("/tmp")).unwrap();
        assert!(result.contains("Usage: autonew"));
    }

    #[test]
    fn autonew_errors_when_no_session_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd = ParsedAutonew {
            project: None,
            provider: "claude".to_string(),
        };
        let result = autonew(&cmd, tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No active claude session"));
    }
}
