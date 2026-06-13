use crate::parser::{ParsedAsk, ParsedStart};
use crate::render::{
    render_agent_status, render_ask_receipt, render_attach, render_ping, render_project_view,
    render_shutdown, render_start, render_stop, ProjectView,
};
use crate::services::DaemonClient;
use serde_json::Value;
use std::path::Path;

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
    let result = client.call("submit", params)?;
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

/// Helper to extract a string field from a JSON payload.
pub fn json_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|v| v.as_str())
}
