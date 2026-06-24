use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Project view rendered for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectView {
    pub project_root: String,
    pub project_slug: String,
    pub agents: Vec<AgentView>,
    pub daemon_status: String,
}

/// Agent view used for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView {
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub health: String,
    pub pane_id: Option<String>,
    pub workspace_path: Option<String>,
}

/// Render a project view for terminal output.
pub fn render_project_view(view: &ProjectView) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Project: {} ({}):\n",
        view.project_slug, view.project_root
    ));
    out.push_str(&format!("Daemon: {}\n", view.daemon_status));
    out.push_str(&format!("Agents ({}):\n", view.agents.len()));
    if view.agents.is_empty() {
        out.push_str("  (none)\n");
    }
    for agent in &view.agents {
        let pane = agent.pane_id.as_deref().unwrap_or("-");
        let state = if agent.state.is_empty() {
            &agent.health
        } else {
            &agent.state
        };
        out.push_str(&format!(
            "  {} [{}] {} ({}):\n",
            agent.name, state, agent.provider, pane
        ));
    }
    out
}

/// Render agent status for `ps` command.
pub fn render_agent_status(agents: &[AgentView]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<20} {:<12} {:<10} {:<10}\n",
        "NAME", "PROVIDER", "STATE", "PANE"
    ));
    out.push_str(&format!(
        "{:<20} {:<12} {:<10} {:<10}\n",
        "----", "--------", "-----", "----"
    ));
    for agent in agents {
        let pane = agent.pane_id.as_deref().unwrap_or("-");
        let state = if agent.state.is_empty() {
            &agent.health
        } else {
            &agent.state
        };
        out.push_str(&format!(
            "{:<20} {:<12} {:<10} {:<10}\n",
            agent.name, agent.provider, state, pane
        ));
    }
    out
}

/// Render the result of a `start` command.
pub fn render_start(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let agents: Vec<&str> = result
        .get("agent_results")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("agent_name").and_then(|v| v.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let mut out = format!("Start status: {}\n", status);
    if agents.is_empty() {
        out.push_str("No agents started.\n");
    } else {
        out.push_str(&format!("Started agents: {}\n", agents.join(", ")));
    }
    out
}

/// Render the result of a `stop` / `stop-all` / `kill` command.
pub fn render_stop(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let forced = result
        .get("forced")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let agents: Vec<&str> = result
        .get("stopped_agents")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|a| a.as_str()).collect())
        .unwrap_or_default();
    let mut out = format!("Stop status: {} (forced: {})\n", status, forced);
    if agents.is_empty() {
        out.push_str("No agents were running.\n");
    } else {
        out.push_str(&format!("Stopped agents: {}\n", agents.join(", ")));
    }
    out
}

/// Render a ping response.
pub fn render_ping(target: &str, result: &Value) -> String {
    let pong = result
        .get("pong")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    format!(
        "Ping {}: {} (status: {})\n",
        target,
        if pong { "pong" } else { "no response" },
        status
    )
}

/// Render an ask/submit receipt.
pub fn render_ask_receipt(result: &Value) -> String {
    let job_id = result
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("submitted");
    format!("Ask submitted ({}): {}\n", status, job_id)
}

/// Render an attach response.
pub fn render_attach(result: &Value) -> String {
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    let pane = result
        .get("pane_id")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    format!("Attached {} (status: {}, pane: {})\n", agent, status, pane)
}

/// Render a shutdown response.
pub fn render_shutdown(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    format!("Shutdown requested (status: {})\n", status)
}

/// Render a queue summary response.
pub fn render_queue(result: &Value) -> String {
    let target = result
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let mut out = format!("Queue for {}:\n", target);
    let agents = result
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    if agents.is_empty() {
        out.push_str("  (no agents)\n");
    }
    for agent in agents {
        let name = agent
            .get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let depth = agent
            .get("queue_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let active = agent
            .get("active_job_id")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        out.push_str(&format!("  {}: depth={} active={}\n", name, depth, active));
    }
    out
}

/// Render a trace response.
pub fn render_trace(result: &Value) -> String {
    let target = result
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let mut out = format!("Trace for {}:\n", target);
    let jobs = result
        .get("jobs")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    if jobs.is_empty() {
        out.push_str("  (no jobs)\n");
    }
    for job in jobs {
        let job_id = job.get("job_id").and_then(|v| v.as_str()).unwrap_or("-");
        let agent = job
            .get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        let status = job.get("status").and_then(|v| v.as_str()).unwrap_or("-");
        out.push_str(&format!("  {} [{}] {}\n", job_id, agent, status));
    }
    out
}

/// Render a watch response.
pub fn render_watch(result: &Value) -> String {
    let target = result
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let cursor = result.get("cursor").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut out = format!("Watch {} (cursor={}):\n", target, cursor);
    let lines = result
        .get("lines")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    if lines.is_empty() {
        out.push_str("  (no new output)\n");
    }
    for line in lines {
        if let Some(s) = line.as_str() {
            out.push_str(&format!("  {}\n", s));
        }
    }
    if result.get("eof").and_then(|v| v.as_bool()).unwrap_or(false) {
        out.push_str("  (eof)\n");
    }
    out
}

/// Render an inbox response.
pub fn render_inbox(result: &Value) -> String {
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let pending = result
        .get("pending_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let mut out = format!("Inbox for {} (pending={}):\n", agent, pending);
    let events = result
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    if events.is_empty() {
        out.push_str("  (no events)\n");
    }
    for event in events {
        out.push_str(&format!("  {}\n", event));
    }
    out
}

/// Render an ack response.
pub fn render_ack(result: &Value) -> String {
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let event_id = result
        .get("inbound_event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    format!(
        "Acknowledged {} event {} (status: {})\n",
        agent, event_id, status
    )
}

/// Render a cancel response.
pub fn render_cancel(result: &Value) -> String {
    let job_id = result.get("job_id").and_then(|v| v.as_str()).unwrap_or("-");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("cancelled");
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    format!("Cancelled {} for {} (status: {})\n", job_id, agent, status)
}

/// Render a resubmit response.
pub fn render_resubmit(result: &Value) -> String {
    let message_id = result
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("resubmitted");
    format!("Resubmitted {} (status: {})\n", message_id, status)
}

/// Render a retry response.
pub fn render_retry(result: &Value) -> String {
    let target = result
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("all");
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("retried");
    format!("Retried {} (status: {})\n", target, status)
}

/// Render a reload response.
pub fn render_reload(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let dry_run = result
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut out = format!("Reload status: {} (dry_run: {})\n", status, dry_run);
    for key in [
        "added_agents",
        "removed_agents",
        "modified_agents",
        "unchanged_agents",
    ] {
        let items: Vec<&str> = result
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        if !items.is_empty() {
            out.push_str(&format!("  {}: {}\n", key, items.join(", ")));
        }
    }
    out
}

/// Render a restart response.
pub fn render_restart(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let restart_status = result
        .get("restart_status")
        .and_then(|v| v.as_str())
        .unwrap_or(status);
    if agent == "-" {
        format!("Restart status: {}\n", restart_status)
    } else {
        format!("Restart {} (status: {})\n", agent, restart_status)
    }
}

/// Render a clear response.
pub fn render_clear(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    let names: Vec<&str> = result
        .get("agent_names")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    if names.is_empty() {
        format!("Clear status: {}\n", status)
    } else {
        format!("Clear status: {} for {}\n", status, names.join(", "))
    }
}

/// Render a maintenance response.
pub fn render_maintenance(result: &Value) -> String {
    if let Some(ticked) = result.get("ticked").and_then(|v| v.as_bool()) {
        let agents = result
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        return format!("Maintenance tick: {} ({} agents)\n", ticked, agents.len());
    }
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    format!("Maintenance status: {}\n", status)
}

/// Render a logs response.
pub fn render_logs(result: &Value) -> String {
    let mut out = String::new();
    let agent = result
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    out.push_str(&format!("Logs for {}:\n", agent));

    let entries = result.get("entries").and_then(|v| v.as_array());
    let entries = match entries {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            out.push_str("  <none>\n");
            return out;
        }
    };

    for entry in entries {
        let source = entry
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("log");
        let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or("-");
        out.push_str(&format!("  [{}] {}\n", source, path));
        if let Some(lines) = entry.get("lines").and_then(|v| v.as_array()) {
            for line in lines.iter().filter_map(|v| v.as_str()) {
                out.push_str(&format!("    {}\n", line));
            }
        }
    }
    out
}

/// Render a cleanup response.
pub fn render_cleanup(result: &Value) -> String {
    let dry_run = result
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let orphaned = result
        .get("orphaned_jobs_removed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let files = result
        .get("stale_files_removed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let errors = result
        .get("errors")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut out = String::new();
    out.push_str(&format!(
        "Cleanup status: {} (dry_run={})\n",
        result
            .get("cleanup_status")
            .and_then(|v| v.as_str())
            .unwrap_or("ok"),
        dry_run
    ));
    out.push_str(&format!("  orphaned_jobs_removed: {}\n", orphaned));
    out.push_str(&format!("  stale_files_removed: {}\n", files));
    if !errors.is_empty() {
        out.push_str("  errors:\n");
        for err in errors {
            out.push_str(&format!("    - {}\n", err));
        }
    }
    out
}

/// Render a wait-ready message.
pub fn render_wait_ready(target: &str) -> String {
    format!("{} is ready\n", target)
}

/// Render doctor diagnostic summary.
pub fn render_doctor(result: &Value) -> String {
    let project = result
        .get("project_root")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let daemon = if result
        .get("daemon_ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "ok"
    } else {
        "not reachable"
    };
    let tmux = if result
        .get("tmux_present")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "present"
    } else {
        "missing"
    };
    let config = if result
        .get("config_ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "ok"
    } else {
        "invalid"
    };
    let mut out = format!("CCB Doctor for {}\n", project);
    out.push_str(&format!("  daemon: {}\n", daemon));
    out.push_str(&format!("  tmux socket: {}\n", tmux));
    out.push_str(&format!("  config: {}\n", config));
    let issues = result
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    if !issues.is_empty() {
        out.push_str("Issues:\n");
        for issue in issues {
            if let Some(s) = issue.as_str() {
                out.push_str(&format!("  - {}\n", s));
            }
        }
    }
    out
}

/// Render config validation summary.
pub fn render_config_validate(result: &Value) -> String {
    let mut out = String::from("Config validation: ok\n");
    if let Some(source) = result.get("source_path").and_then(|v| v.as_str()) {
        out.push_str(&format!("  source: {}\n", source));
    } else {
        out.push_str("  source: built-in default\n");
    }
    out.push_str(&format!(
        "  source_kind: {}\n",
        result
            .get("source_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("-")
    ));
    out.push_str(&format!(
        "  agents: {}\n",
        result
            .get("agent_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    ));
    out.push_str(&format!(
        "  default_agents: {:?}\n",
        result.get("default_agents").and_then(|v| v.as_array())
    ));
    out
}

/// Render pend results.
pub fn render_pend(target: &str, result: &Value) -> String {
    let mut out = format!("Pend for {}:\n", target);
    if let Some(status) = result.get("status").and_then(|v| v.as_str()) {
        out.push_str(&format!("  status: {}\n", status));
    }
    if let Some(job_id) = result.get("job_id").and_then(|v| v.as_str()) {
        out.push_str(&format!("  job_id: {}\n", job_id));
    }
    if let Some(agent) = result.get("agent_name").and_then(|v| v.as_str()) {
        out.push_str(&format!("  agent: {}\n", agent));
    }
    out
}

/// Render tools stubs.
pub fn render_tools(tool: &str, action: &str, status: &str) -> String {
    format!("tools {} {}: {}\n", action, tool, status)
}

/// Render roles/fault generic record output.
pub fn render_roles(result: &Value) -> String {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("ok");
    let mut out = format!("roles_status: {}\n", status);
    if let Some(action) = result.get("action").and_then(|v| v.as_str()) {
        out.push_str(&format!("action: {}\n", action));
    }
    if let Some(spec) = result.get("spec").and_then(|v| v.as_str()) {
        out.push_str(&format!("spec: {}\n", spec));
    }
    if let Some(path) = result.get("path").and_then(|v| v.as_str()) {
        out.push_str(&format!("path: {}\n", path));
    }
    if let Some(rules) = result.get("rules").and_then(|v| v.as_array()) {
        for rule in rules {
            if let Some(obj) = rule.as_object() {
                let id = obj.get("rule_id").and_then(|v| v.as_str()).unwrap_or("-");
                let agent = obj
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let reason = obj.get("reason").and_then(|v| v.as_str()).unwrap_or("-");
                out.push_str(&format!("rule: {} {} {}\n", id, agent, reason));
            }
        }
    }
    if let Some(roles) = result.get("roles").and_then(|v| v.as_array()) {
        for role in roles {
            if let Some(obj) = role.as_object() {
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                let version = obj.get("version").and_then(|v| v.as_str()).unwrap_or("-");
                out.push_str(&format!("role: {} {} {}\n", id, name, version));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_project_view() {
        let view = ProjectView {
            project_root: "/project".into(),
            project_slug: "my-project".into(),
            agents: vec![AgentView {
                name: "agent-a".into(),
                provider: "claude".into(),
                state: "idle".into(),
                health: "healthy".into(),
                pane_id: Some("%0".into()),
                workspace_path: None,
            }],
            daemon_status: "running".into(),
        };
        let output = render_project_view(&view);
        assert!(output.contains("my-project"));
        assert!(output.contains("agent-a"));
        assert!(output.contains("idle"));
    }

    #[test]
    fn test_render_agent_status() {
        let agents = vec![AgentView {
            name: "agent-a".into(),
            provider: "claude".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%0".into()),
            workspace_path: None,
        }];
        let output = render_agent_status(&agents);
        assert!(output.contains("NAME"));
        assert!(output.contains("agent-a"));
    }

    #[test]
    fn test_render_start() {
        let value = serde_json::json!({
            "status": "ok",
            "agent_results": [
                {"agent_name": "claude", "status": "ok"},
                {"agent_name": "gemini", "status": "ok"},
            ]
        });
        let out = render_start(&value);
        assert!(out.contains("ok"));
        assert!(out.contains("claude"));
        assert!(out.contains("gemini"));
    }

    #[test]
    fn test_render_stop() {
        let value =
            serde_json::json!({"status": "ok", "forced": true, "stopped_agents": ["claude"]});
        let out = render_stop(&value);
        assert!(out.contains("claude"));
        assert!(out.contains("forced: true"));
    }

    #[test]
    fn test_render_ask_receipt() {
        let value = serde_json::json!({"job_id": "job-123", "status": "queued"});
        let out = render_ask_receipt(&value);
        assert!(out.contains("job-123"));
    }

    #[test]
    fn test_render_queue() {
        let value = serde_json::json!({
            "target": "claude",
            "agents": [
                {"agent_name": "claude", "queue_depth": 2, "active_job_id": "job-1"}
            ]
        });
        let out = render_queue(&value);
        assert!(out.contains("Queue for claude"));
        assert!(out.contains("depth=2"));
        assert!(out.contains("active=job-1"));
    }

    #[test]
    fn test_render_trace() {
        let value = serde_json::json!({
            "target": "claude",
            "jobs": [
                {"job_id": "job-1", "agent_name": "claude", "status": "running"}
            ]
        });
        let out = render_trace(&value);
        assert!(out.contains("Trace for claude"));
        assert!(out.contains("job-1"));
        assert!(out.contains("running"));
    }

    #[test]
    fn test_render_watch() {
        let value =
            serde_json::json!({"target": "claude", "cursor": 5, "lines": ["line1"], "eof": true});
        let out = render_watch(&value);
        assert!(out.contains("Watch claude"));
        assert!(out.contains("line1"));
    }

    #[test]
    fn test_render_inbox() {
        let value = serde_json::json!({"agent_name": "claude", "pending_count": 1, "events": []});
        let out = render_inbox(&value);
        assert!(out.contains("Inbox for claude"));
        assert!(out.contains("pending=1"));
    }

    #[test]
    fn test_render_ack() {
        let value = serde_json::json!({"agent_name": "claude", "inbound_event_id": "evt-1", "status": "acked"});
        let out = render_ack(&value);
        assert!(out.contains("Acknowledged claude event evt-1"));
    }

    #[test]
    fn test_render_cancel() {
        let value =
            serde_json::json!({"job_id": "job-1", "agent_name": "claude", "status": "cancelled"});
        let out = render_cancel(&value);
        assert!(out.contains("Cancelled job-1 for claude"));
    }

    #[test]
    fn test_render_resubmit() {
        let value = serde_json::json!({"message_id": "msg-1", "status": "resubmitted"});
        let out = render_resubmit(&value);
        assert!(out.contains("Resubmitted msg-1"));
    }

    #[test]
    fn test_render_retry() {
        let value = serde_json::json!({"target": "claude", "status": "retried"});
        let out = render_retry(&value);
        assert!(out.contains("Retried claude"));
    }

    #[test]
    fn test_render_reload() {
        let value = serde_json::json!({
            "status": "ok", "dry_run": true,
            "added_agents": ["a"], "removed_agents": [], "modified_agents": ["b"], "unchanged_agents": []
        });
        let out = render_reload(&value);
        assert!(out.contains("Reload status: ok"));
        assert!(out.contains("added_agents: a"));
    }

    #[test]
    fn test_render_restart() {
        let value =
            serde_json::json!({"status": "ok", "restart_status": "ok", "agent_name": "claude"});
        let out = render_restart(&value);
        assert!(out.contains("Restart claude"));
    }

    #[test]
    fn test_render_clear() {
        let value = serde_json::json!({"status": "ok", "agent_names": ["claude", "gemini"]});
        let out = render_clear(&value);
        assert!(out.contains("Clear status: ok for claude, gemini"));
    }

    #[test]
    fn test_render_maintenance_tick() {
        let value = serde_json::json!({"ticked": true, "agents": ["claude", "gemini"]});
        let out = render_maintenance(&value);
        assert!(out.contains("Maintenance tick: true"));
        assert!(out.contains("2 agents"));
    }

    #[test]
    fn test_render_logs() {
        let value = serde_json::json!({
            "agent_name": "codex",
            "entries": [
                {"source": "session", "path": "/tmp/codex-session.jsonl", "lines": ["line1", "line2"]}
            ]
        });
        let out = render_logs(&value);
        assert!(out.contains("Logs for codex"));
        assert!(out.contains("line1"));
        assert!(out.contains("line2"));
    }

    #[test]
    fn test_render_logs_none() {
        let value = serde_json::json!({"agent_name": "codex", "entries": []});
        let out = render_logs(&value);
        assert!(out.contains("Logs for codex"));
        assert!(out.contains("<none>"));
    }

    #[test]
    fn test_render_cleanup() {
        let value = serde_json::json!({
            "cleanup_status": "ok",
            "dry_run": false,
            "orphaned_jobs_removed": 2,
            "stale_files_removed": 1,
            "errors": []
        });
        let out = render_cleanup(&value);
        assert!(out.contains("Cleanup status: ok"));
        assert!(out.contains("orphaned_jobs_removed: 2"));
        assert!(out.contains("stale_files_removed: 1"));
    }

    #[test]
    fn test_render_doctor() {
        let value = serde_json::json!({
            "project_root": "/project",
            "daemon_ok": true,
            "tmux_present": false,
            "config_ok": true,
            "issues": ["tmux socket missing"]
        });
        let out = render_doctor(&value);
        assert!(out.contains("CCB Doctor"));
        assert!(out.contains("daemon: ok"));
        assert!(out.contains("tmux socket: missing"));
        assert!(out.contains("tmux socket missing"));
    }

    #[test]
    fn test_render_config_validate() {
        let value = serde_json::json!({
            "source_path": "/project/.ccbr/ccbr.config",
            "source_kind": "project",
            "agent_count": 2,
            "default_agents": ["claude"]
        });
        let out = render_config_validate(&value);
        assert!(out.contains("Config validation: ok"));
        assert!(out.contains("agents: 2"));
    }

    #[test]
    fn test_render_pend() {
        let value =
            serde_json::json!({"status": "running", "job_id": "job-1", "agent_name": "claude"});
        let out = render_pend("claude", &value);
        assert!(out.contains("Pend for claude"));
        assert!(out.contains("running"));
        assert!(out.contains("job-1"));
    }

    #[test]
    fn test_render_tools() {
        let out = render_tools("neovim", "doctor", "ok");
        assert!(out.contains("tools doctor neovim"));
    }

    #[test]
    fn test_render_roles() {
        let value = serde_json::json!({
            "status": "ok",
            "roles": [
                {"id": "agentroles.archi", "name": "Architect", "version": "1.0.0"}
            ]
        });
        let out = render_roles(&value);
        assert!(out.contains("agentroles.archi"));
        assert!(out.contains("Architect"));
    }
}
