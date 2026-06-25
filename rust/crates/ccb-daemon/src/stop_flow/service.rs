use ccb_terminal::TerminalBackend;
use serde::{Deserialize, Serialize};

use crate::services::registry::AgentRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopFlowResult {
    pub status: String,
    pub forced: bool,
    pub stopped_agents: Vec<String>,
    pub actions_taken: Vec<String>,
    pub cleanup_summaries: Vec<StopCleanupSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopCleanupSummary {
    pub socket_name: String,
    pub killed_panes: Vec<String>,
    pub errors: Vec<String>,
}

/// Backend mode for the stop flow.
#[derive(Debug, Clone, Copy)]
pub enum StopFlowMode {
    /// Use the real tmux backend via `ccb-terminal`.
    Tmux,
    /// No-op stub for tests.
    Stub,
}

pub struct StopFlowService {
    mode: StopFlowMode,
}

impl StopFlowService {
    pub fn new(mode: StopFlowMode) -> Self {
        Self { mode }
    }

    pub fn with_tmux() -> Self {
        Self::new(StopFlowMode::Tmux)
    }

    pub fn with_stub() -> Self {
        Self::new(StopFlowMode::Stub)
    }

    pub fn execute(
        &self,
        registry: &mut AgentRegistry,
        socket_path: Option<&str>,
        session_name: Option<&str>,
        agent_names: &[String],
        force: bool,
    ) -> StopFlowResult {
        let stopped_agents: Vec<String> = agent_names.to_vec();
        let mut killed_panes = Vec::new();
        let mut errors = Vec::new();

        if let Some(socket) = socket_path {
            for agent_name in agent_names {
                if let Some(entry) = registry.get(agent_name) {
                    if let Some(pane) = &entry.pane_id {
                        killed_panes.push(pane.clone());
                    }
                }
            }

            // Only terminate provider panes on forced stops. Graceful shutdown
            // preserves panes so a restarted daemon can adopt running jobs.
            if force {
                match self.mode {
                    StopFlowMode::Tmux => {
                        let backend =
                            ccb_terminal::TmuxBackend::new(None, Some(socket.to_string()));
                        for pane in &killed_panes {
                            if let Err(e) = backend.kill_pane(pane) {
                                errors.push(e.to_string());
                            }
                        }
                        // Fallback: if no pane ids were tracked but a session exists,
                        // tear down the whole session.
                        if killed_panes.is_empty() {
                            if let Some(session) = session_name {
                                if let Err(e) = backend.kill_pane(session) {
                                    errors.push(e.to_string());
                                }
                            }
                        }
                    }
                    StopFlowMode::Stub => {
                        // No-op.
                    }
                }
            }
        }

        let mut actions_taken = vec!["stop_flow_executed".to_string()];
        if force {
            actions_taken.push("forced_cleanup".to_string());
        }
        for agent_name in agent_names {
            registry.mark_stopped(agent_name);
            actions_taken.push(format!("mark_runtime_stopped:{agent_name}"));
        }
        actions_taken.push("terminate_runtime_pids:0".to_string());

        let cleanup_summaries = if killed_panes.is_empty() && errors.is_empty() {
            Vec::new()
        } else {
            vec![StopCleanupSummary {
                socket_name: socket_path.unwrap_or("").to_string(),
                killed_panes,
                errors,
            }]
        };

        StopFlowResult {
            status: "ok".to_string(),
            forced: force,
            stopped_agents,
            actions_taken,
            cleanup_summaries,
        }
    }

    pub fn to_record(&self, result: &StopFlowResult) -> serde_json::Value {
        serde_json::json!({
            "status": result.status,
            "forced": result.forced,
            "stopped_agents": result.stopped_agents,
            "actions_taken": result.actions_taken,
            "cleanup_summaries": result.cleanup_summaries.iter().map(|c| serde_json::json!({
                "socket_name": c.socket_name,
                "killed_panes": c.killed_panes,
                "errors": c.errors,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::registry::AgentRuntimeEntry;

    #[test]
    fn test_stub_stop_flow_marks_agents_stopped() {
        let mut registry = AgentRegistry::new();
        registry.register(AgentRuntimeEntry {
            agent_name: "claude".to_string(),
            provider: "claude".to_string(),
            state: "running".into(),
            health: "healthy".into(),
            pane_id: Some("%1".to_string()),
            workspace_path: None,
            runtime_pid: Some(1234),
            session_id: Some("sess-1".to_string()),
            restart_count: 0,
        });

        let service = StopFlowService::with_stub();
        let result = service.execute(
            &mut registry,
            Some("/tmp/tmux.sock"),
            Some("session"),
            &["claude".to_string()],
            false,
        );

        assert_eq!(result.stopped_agents, vec!["claude"]);
        assert!(!registry.is_empty(), "registry should retain stopped entry");
        let entry = registry.get("claude").unwrap();
        assert_eq!(entry.state, "stopped");
        assert_eq!(entry.health, "stopped");
        assert!(entry.pane_id.is_none());
        assert!(result
            .actions_taken
            .iter()
            .any(|a| a == "mark_runtime_stopped:claude"));
        assert!(result
            .actions_taken
            .iter()
            .any(|a| a == "terminate_runtime_pids:0"));
    }
}
