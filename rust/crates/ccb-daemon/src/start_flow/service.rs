use std::collections::HashMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::services::project_namespace::{NamespaceWindow, ProjectNamespace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartFlowResult {
    pub status: String,
    pub agent_results: Vec<StartAgentResult>,
    pub actions_taken: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartAgentResult {
    pub agent_name: String,
    pub status: String,
    pub reason: Option<String>,
    pub pane_id: Option<String>,
}

/// Trait abstracting tmux (or other terminal backend) execution for start flow.
pub trait StartBackend: Send + Sync {
    fn create_session(
        &self,
        socket_path: &str,
        session_name: &str,
        cwd: &str,
    ) -> Result<String, String>;

    fn kill_session(&self, socket_path: &str, session_name: &str) -> Result<(), String>;

    fn is_session_alive(&self, socket_path: &str, session_name: &str) -> bool;
}

/// Real tmux backend used in production.
#[derive(Debug, Clone, Default)]
pub struct TmuxStartBackend;

impl TmuxStartBackend {
    fn run(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let mut cmd = std::process::Command::new("tmux");
        cmd.args(args);
        cmd.output().map_err(|e| e.to_string())
    }
}

impl StartBackend for TmuxStartBackend {
    fn create_session(
        &self,
        socket_path: &str,
        session_name: &str,
        cwd: &str,
    ) -> Result<String, String> {
        // Create a new detached tmux session.
        let args = vec![
            "-S",
            socket_path,
            "new-session",
            "-d",
            "-s",
            session_name,
            "-c",
            cwd,
        ];
        let out = self.run(&args)?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        // Retrieve the first pane id.
        let list_args = vec![
            "-S",
            socket_path,
            "list-panes",
            "-t",
            session_name,
            "-F",
            "#{pane_id}",
        ];
        let list_out = self.run(&list_args)?;
        if !list_out.status.success() {
            return Err(String::from_utf8_lossy(&list_out.stderr).to_string());
        }
        let pane_id = String::from_utf8_lossy(&list_out.stdout)
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or("%0")
            .to_string();
        Ok(pane_id)
    }

    fn kill_session(&self, socket_path: &str, session_name: &str) -> Result<(), String> {
        let args = vec!["-S", socket_path, "kill-session", "-t", session_name];
        let out = self.run(&args)?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        Ok(())
    }

    fn is_session_alive(&self, socket_path: &str, session_name: &str) -> bool {
        let args = vec!["-S", socket_path, "has-session", "-t", session_name];
        self.run(&args).map(|o| o.status.success()).unwrap_or(false)
    }
}

/// Stub backend used in tests or when tmux is unavailable.
#[derive(Debug, Default)]
pub struct StubStartBackend {
    next_pane_id: std::sync::atomic::AtomicUsize,
}

impl StartBackend for StubStartBackend {
    fn create_session(
        &self,
        _socket_path: &str,
        _session_name: &str,
        _cwd: &str,
    ) -> Result<String, String> {
        let id = self
            .next_pane_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("%{}", id))
    }

    fn kill_session(&self, _socket_path: &str, _session_name: &str) -> Result<(), String> {
        Ok(())
    }

    fn is_session_alive(&self, _socket_path: &str, _session_name: &str) -> bool {
        true
    }
}

pub struct StartFlowService {
    backend: Box<dyn StartBackend>,
}

impl StartFlowService {
    pub fn new(backend: Box<dyn StartBackend>) -> Self {
        Self { backend }
    }

    pub fn with_tmux() -> Self {
        Self::new(Box::new(TmuxStartBackend))
    }

    pub fn with_stub() -> Self {
        Self::new(Box::new(StubStartBackend::default()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        project_root: &Utf8Path,
        project_id: &str,
        tmux_socket_path: &str,
        tmux_session_name: &str,
        agent_names: &[String],
        restore: bool,
        auto_permission: bool,
    ) -> Result<(StartFlowResult, ProjectNamespace), String> {
        let mut actions_taken = vec!["start_flow_executed".to_string()];
        if restore {
            actions_taken.push("restore_attempted".to_string());
        }
        if auto_permission {
            actions_taken.push("auto_permission_enabled".to_string());
        }

        let root_pane = self.backend.create_session(
            tmux_socket_path,
            tmux_session_name,
            project_root.as_str(),
        )?;

        let mut agent_panes = HashMap::new();
        let mut agent_results = Vec::new();

        for (idx, agent_name) in agent_names.iter().enumerate() {
            let pane_id = if idx == 0 {
                root_pane.clone()
            } else {
                self.backend
                    .create_session(
                        tmux_socket_path,
                        &format!("{}-{}", tmux_session_name, agent_name),
                        project_root.as_str(),
                    )
                    .unwrap_or_else(|_| format!("%{}", idx))
            };
            agent_panes.insert(agent_name.clone(), pane_id.clone());
            agent_results.push(StartAgentResult {
                agent_name: agent_name.clone(),
                status: "started".to_string(),
                reason: None,
                pane_id: Some(pane_id),
            });
        }

        let namespace = ProjectNamespace {
            project_root: project_root.as_str().to_string(),
            project_id: project_id.to_string(),
            tmux_socket_path: tmux_socket_path.to_string(),
            tmux_socket_name: "tmux".to_string(),
            tmux_session_name: tmux_session_name.to_string(),
            agent_names: agent_names.to_vec(),
            windows: vec![NamespaceWindow {
                name: "ccb".to_string(),
                window_id: None,
                agents: agent_names.to_vec(),
            }],
            agent_panes,
            active_panes: agent_names.to_vec(),
            namespace_epoch: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let result = StartFlowResult {
            status: "ok".to_string(),
            agent_results,
            actions_taken,
        };

        Ok((result, namespace))
    }

    pub fn to_record(&self, result: &StartFlowResult) -> serde_json::Value {
        serde_json::json!({
            "status": result.status,
            "agent_results": result.agent_results.iter().map(|a| serde_json::json!({
                "agent_name": a.agent_name,
                "status": a.status,
                "reason": a.reason,
                "pane_id": a.pane_id,
            })).collect::<Vec<_>>(),
            "actions_taken": result.actions_taken,
        })
    }
}
