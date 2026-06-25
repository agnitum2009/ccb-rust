use crate::app::CcbdApp;
use serde_json::{json, Value};

pub fn handle_project_restart_agent(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if agent_name.is_empty() {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "reason": "missing_agent",
            "error": "restart requires exactly one agent_name",
        }));
    }
    if agent_name.to_lowercase() == "all" {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "reason": "restart_all_unsupported",
            "error": "restart all is not supported; restart exactly one configured agent",
        }));
    }

    // Force a fresh start flow for the whole project.  run_start_flow loads
    // the existing namespace, detects stale/dead panes, and recreates the
    // topology before launching provider CLIs again.  We restart all agents
    // rather than just the requested one because the layout namespace must
    // remain consistent across all panes after an external kill-pane.
    let auto_permission = payload
        .get("auto_permission")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let config_windows = app.current_config.as_ref().and_then(|c| c.windows.clone());
    let all_agents: Vec<String> = app.agent_names();
    if !all_agents.contains(&agent_name.to_string()) {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "agent_name": agent_name,
            "reason": "agent not configured",
            "recreate_reason": "manual_restart_agent",
        }));
    }
    let result = app.run_start_flow(
        &all_agents,
        false, // restore=false: do not restore sessions, respawn fresh
        auto_permission,
        config_windows,
        None,
        None,
        &[],
    )?;

    let agent_result = result
        .agent_results
        .into_iter()
        .find(|a| a.agent_name == agent_name)
        .ok_or_else(|| format!("restart produced no result for agent {}", agent_name))?;

    if agent_result.status == "failed" {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "agent_name": agent_name,
            "reason": agent_result.reason.unwrap_or_else(|| "unknown".to_string()),
            "recreate_reason": "manual_restart_agent",
        }));
    }

    Ok(json!({
        "status": "ok",
        "restart_status": "ok",
        "agent_name": agent_name,
        "pane_id": agent_result.pane_id,
        "recreate_reason": "manual_restart_agent",
    }))
}

pub fn handle_project_restart_panes(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let all_agents: Vec<String> = app.agent_names();
    if all_agents.is_empty() {
        return Ok(json!({
            "status": "failed",
            "restart_status": "failed",
            "reason": "no_agents_configured",
            "agent_names": [],
            "restart_mode": "recreate_topology",
            "recreate_reason": "manual_restart_panes",
        }));
    }

    let auto_permission = payload
        .get("auto_permission")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let config_windows = app.current_config.as_ref().and_then(|c| c.windows.clone());
    let result = app.run_start_flow(
        &all_agents,
        false,
        auto_permission,
        config_windows,
        None,
        None,
        &[],
    )?;
    let any_failed = result
        .agent_results
        .iter()
        .any(|agent| agent.status == "failed");
    let agent_results: Vec<Value> = result
        .agent_results
        .into_iter()
        .map(|agent| {
            json!({
                "agent_name": agent.agent_name,
                "status": agent.status,
                "reason": agent.reason,
                "pane_id": agent.pane_id,
            })
        })
        .collect();

    Ok(json!({
        "status": if any_failed { "failed" } else { "ok" },
        "restart_status": if any_failed { "failed" } else { "ok" },
        "reason": if any_failed { "agent_restart_failed" } else { "" },
        "agent_names": all_agents,
        "agent_results": agent_results,
        "restart_mode": "recreate_topology",
        "actions_taken": result.actions_taken,
        "recreate_reason": "manual_restart_panes",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CcbdApp;
    use crate::services::registry::AgentRuntimeEntry;
    use crate::start_flow::service::StartFlowService;
    use crate::stop_flow::service::StopFlowService;
    use serde_json::json;
    use tempfile::TempDir;

    fn stub_app(dir: &TempDir) -> CcbdApp {
        CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        )
    }

    #[test]
    fn test_restart_agent_missing_name_fails() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let result = handle_project_restart_agent(&mut app, &json!({}))
            .expect("handler returns structured failure");
        assert_eq!(result["status"], "failed");
        assert_eq!(result["reason"], "missing_agent");
    }

    #[test]
    fn test_restart_all_unsupported() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let result = handle_project_restart_agent(&mut app, &json!({"agent_name": "all"}))
            .expect("handler returns structured failure");
        assert_eq!(result["status"], "failed");
        assert_eq!(result["reason"], "restart_all_unsupported");
    }

    #[test]
    fn test_restart_unknown_agent_fails() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let result = handle_project_restart_agent(&mut app, &json!({"agent_name": "ghost"}))
            .expect("handler returns structured failure");
        assert_eq!(result["status"], "failed");
        assert_eq!(result["reason"], "agent not configured");
    }

    #[test]
    fn test_restart_known_agent_triggers_start_flow() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent1".into(),
            provider: "codex".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%0".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        let result = handle_project_restart_agent(&mut app, &json!({"agent_name": "agent1"}))
            .expect("handler succeeds");
        assert_eq!(result["status"], "ok");
        assert_eq!(result["restart_status"], "ok");
        assert_eq!(result["agent_name"], "agent1");
        assert!(result["pane_id"].is_string());
    }

    #[test]
    fn test_restart_panes_without_agents_fails_loudly() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        let result = handle_project_restart_panes(&mut app, &json!({}))
            .expect("handler returns structured failure");
        assert_eq!(result["status"], "failed");
        assert_eq!(result["restart_status"], "failed");
        assert_eq!(result["reason"], "no_agents_configured");
        assert_eq!(result["restart_mode"], "recreate_topology");
    }

    #[test]
    fn test_restart_panes_triggers_start_flow_for_all_agents() {
        let dir = TempDir::new().unwrap();
        let mut app = stub_app(&dir);
        for (index, agent_name) in ["agent1", "agent2"].into_iter().enumerate() {
            app.registry.register(AgentRuntimeEntry {
                agent_name: agent_name.into(),
                provider: "codex".into(),
                state: "idle".into(),
                health: "healthy".into(),
                pane_id: Some(format!("%{index}")),
                workspace_path: None,
                runtime_pid: None,
                session_id: None,
                restart_count: 0,
            });
        }

        let result = handle_project_restart_panes(&mut app, &json!({}))
            .expect("handler should run start flow");
        assert_eq!(result["status"], "ok");
        assert_eq!(result["restart_status"], "ok");
        assert_eq!(result["restart_mode"], "recreate_topology");
        assert_eq!(result["agent_results"].as_array().unwrap().len(), 2);
        assert_ne!(result["status"], "scheduled");
    }
}
