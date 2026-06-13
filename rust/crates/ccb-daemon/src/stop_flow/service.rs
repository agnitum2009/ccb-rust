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

/// Trait abstracting tmux (or other terminal backend) stop execution.
pub trait StopBackend: Send + Sync {
    fn kill_session(&self, socket_path: &str, session_name: &str) -> Result<(), String>;
}

/// Real tmux backend.
#[derive(Debug, Clone, Default)]
pub struct TmuxStopBackend;

impl StopBackend for TmuxStopBackend {
    fn kill_session(&self, socket_path: &str, session_name: &str) -> Result<(), String> {
        let mut cmd = std::process::Command::new("tmux");
        cmd.args(["-S", socket_path, "kill-session", "-t", session_name]);
        let out = cmd.output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        Ok(())
    }
}

/// Stub backend for tests.
#[derive(Debug, Clone, Default)]
pub struct StubStopBackend;

impl StopBackend for StubStopBackend {
    fn kill_session(&self, _socket_path: &str, _session_name: &str) -> Result<(), String> {
        Ok(())
    }
}

pub struct StopFlowService {
    backend: Box<dyn StopBackend>,
}

impl StopFlowService {
    pub fn new(backend: Box<dyn StopBackend>) -> Self {
        Self { backend }
    }

    pub fn with_tmux() -> Self {
        Self::new(Box::new(TmuxStopBackend))
    }

    pub fn with_stub() -> Self {
        Self::new(Box::new(StubStopBackend))
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

        if let (Some(socket), Some(session)) = (socket_path, session_name) {
            for agent_name in agent_names {
                if let Some(entry) = registry.get(agent_name) {
                    if let Some(pane) = &entry.pane_id {
                        killed_panes.push(pane.clone());
                    }
                }
            }
            if let Err(e) = self.backend.kill_session(socket, session) {
                errors.push(e);
            }
        }

        for agent_name in agent_names {
            registry.remove(agent_name);
        }

        let mut actions_taken = vec!["stop_flow_executed".to_string()];
        if force {
            actions_taken.push("forced_cleanup".to_string());
        }

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
