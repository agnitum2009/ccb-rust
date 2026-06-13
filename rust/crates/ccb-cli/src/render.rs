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
}
