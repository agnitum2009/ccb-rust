use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::handlers::{build_registry, HandlerRegistry};
use crate::models::lifecycle::{
    CcbdShutdownReport, CcbdStartupAgentResult, CcbdStartupReport, CcbdTmuxCleanupSummary,
};
use crate::services::dispatcher::JobDispatcher;
use crate::services::health::HealthMonitor;
use crate::services::lifecycle::LifecycleService;
use crate::services::ownership::OwnershipService;
use crate::services::project_namespace::ProjectNamespaceController;
use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};
use crate::services::runtime::RuntimeService;

fn load_agent_registry(
    layout: &ccb_storage::paths::PathLayout,
    project_root: &std::path::Path,
) -> (AgentRegistry, Vec<String>) {
    let config_path = layout.ccb_dir().join("ccb.config");
    if !config_path.exists() {
        // No project config yet; start with an empty registry so the start
        // flow can create agents on demand.
        return (AgentRegistry::new(), Vec::new());
    }

    match ccb_agents::config::load_project_config(layout) {
        Ok(result) => {
            let names: Vec<String> = if result.config.default_agents.is_empty() {
                result.config.agents.keys().cloned().collect()
            } else {
                result.config.default_agents.clone()
            };
            let mut registry = AgentRegistry::new();
            for name in &names {
                let spec = result.config.agents.get(name);
                let workspace_path = spec
                    .and_then(|s| s.workspace_path.clone())
                    .unwrap_or_else(|| project_root.to_string_lossy().to_string());
                registry.register(AgentRuntimeEntry {
                    agent_name: name.clone(),
                    provider: spec.map(|s| s.provider.clone()).unwrap_or_default(),
                    state: "registered".into(),
                    health: "unknown".into(),
                    pane_id: None,
                    workspace_path: Some(workspace_path),
                    runtime_pid: None,
                    session_id: None,
                    restart_count: 0,
                });
            }
            (registry, names)
        }
        Err(e) => {
            tracing::warn!("failed to load project config: {}", e);
            let mut registry = AgentRegistry::new();
            registry.register(AgentRuntimeEntry {
                agent_name: "default".into(),
                provider: String::new(),
                state: "registered".into(),
                health: "unknown".into(),
                pane_id: None,
                workspace_path: Some(project_root.to_string_lossy().to_string()),
                runtime_pid: None,
                session_id: None,
                restart_count: 0,
            });
            (registry, vec!["default".into()])
        }
    }
}
use crate::services::start_policy::StartPolicyStore;
use crate::socket_server::protocol;
use crate::start_flow::service::{StartFlowResult, StartFlowService};
use crate::stop_flow::service::{StopCleanupSummary, StopFlowResult, StopFlowService};
use crate::supervision::loop_runner::SupervisionLoop;

/// The main CCB daemon application.
pub struct CcbdApp {
    pub project_root: PathBuf,
    pub layout: ccb_storage::paths::PathLayout,
    pub shutdown_requested: Arc<AtomicBool>,
    pub registry: AgentRegistry,
    pub dispatcher: JobDispatcher,
    pub handlers: Arc<HandlerRegistry>,
    pub project_namespace: ProjectNamespaceController,
    pub start_flow: StartFlowService,
    pub stop_flow: StopFlowService,
    pub health_monitor: HealthMonitor,
    pub lifecycle: LifecycleService,
    pub supervision: SupervisionLoop,
    pub runtime_service: RuntimeService,
    pub ownership: OwnershipService,
    pub start_policy_store: StartPolicyStore,
    pub last_startup_report: Option<CcbdStartupReport>,
    pub last_shutdown_report: Option<CcbdShutdownReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupReport {
    pub trigger: String,
    pub status: String,
    pub actions_taken: Vec<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownReport {
    pub trigger: String,
    pub status: String,
    pub forced: bool,
    pub reason: String,
    pub stopped_agents: Vec<String>,
    pub actions_taken: Vec<String>,
    pub failure_reason: Option<String>,
}

impl CcbdApp {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self::with_backend(
            project_root,
            StartFlowService::with_tmux(),
            StopFlowService::with_tmux(),
        )
    }

    /// Build an app with injected start/stop backends. Used by tests.
    pub fn with_backend(
        project_root: impl Into<PathBuf>,
        start_flow: StartFlowService,
        stop_flow: StopFlowService,
    ) -> Self {
        let project_root = project_root.into();
        let layout = ccb_storage::paths::PathLayout::new(
            Utf8Path::from_path(&project_root).unwrap_or(Utf8Path::new("/")),
        );
        let (registry, agent_names) = load_agent_registry(&layout, &project_root);
        let socket_path = layout.ccbd_socket_path().as_str().to_string();

        Self {
            project_root,
            layout: layout.clone(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            registry,
            dispatcher: JobDispatcher::new(agent_names),
            handlers: Arc::new(build_registry()),
            project_namespace: ProjectNamespaceController::new(&layout),
            start_flow,
            stop_flow,
            health_monitor: HealthMonitor::new(Some(socket_path)),
            lifecycle: LifecycleService::new(),
            supervision: SupervisionLoop::new(1000, 5),
            runtime_service: RuntimeService::new(),
            ownership: OwnershipService::new(),
            start_policy_store: StartPolicyStore::new(&layout),
            last_startup_report: None,
            last_shutdown_report: None,
        }
    }

    pub fn project_id(&self) -> &str {
        self.layout.project_id()
    }

    pub fn socket_path(&self) -> String {
        self.layout.ccbd_socket_path().as_str().to_string()
    }

    pub fn tmux_socket_path(&self) -> String {
        self.layout.ccbd_tmux_socket_path().as_str().to_string()
    }

    pub fn tmux_session_name(&self) -> String {
        self.layout.ccbd_tmux_session_name()
    }

    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }

    /// Start the daemon: acquire ownership and write a startup report.
    pub fn start(&mut self) -> crate::Result<()> {
        let socket_path = self.socket_path();
        let generation = self
            .ownership
            .acquire(std::process::id(), &socket_path)
            .generation;

        let report = CcbdStartupReport {
            project_id: self.project_id().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            trigger: "start_command".into(),
            status: "ok".into(),
            actions_taken: vec!["daemon_started".into()],
            agent_results: vec![],
            failure_reason: None,
            api_version: crate::models::api_models::common::API_VERSION,
        };
        let report_path = self.layout.ccbd_startup_report_path();
        self.last_startup_report = Some(report.clone());
        let _ = ccb_storage::json::JsonStore::new().save(&report_path, &report);

        self.lifecycle
            .record(crate::services::lifecycle::LifecycleReport {
                project_id: self.project_id().to_string(),
                event: "started".into(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                details: serde_json::json!({"generation": generation, "socket_path": socket_path}),
            });

        Ok(())
    }

    /// Shut the daemon down: stop all agents and write a shutdown report.
    pub fn shutdown(&mut self) -> crate::Result<()> {
        let result = self.stop_all(true, "shutdown");
        let report = CcbdShutdownReport {
            project_id: self.project_id().to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            trigger: "shutdown".into(),
            status: result.status.clone(),
            forced: true,
            stopped_agents: result.stopped_agents.clone(),
            daemon_generation: self.ownership.current().map(|o| o.generation),
            reason: Some("shutdown".into()),
            actions_taken: result.actions_taken.clone(),
            cleanup_summaries: result
                .cleanup_summaries
                .iter()
                .map(summary_to_model)
                .collect(),
            runtime_snapshots: vec![],
            failure_reason: None,
            api_version: crate::models::api_models::common::API_VERSION,
        };
        let report_path = self.layout.ccbd_shutdown_report_path();
        self.last_shutdown_report = Some(report.clone());
        let _ = ccb_storage::json::JsonStore::new().save(&report_path, &report);
        self.ownership.release();
        self.project_namespace.unmount().ok();
        Ok(())
    }

    /// Stop all agents without writing the final shutdown report.
    pub fn stop_all(&mut self, force: bool, reason: &str) -> StopFlowResult {
        let ns = self.project_namespace.load();
        let agent_names: Vec<String> = ns.map(|n| n.agent_names.clone()).unwrap_or_default();
        let socket_path = ns.map(|n| n.tmux_socket_path.clone());
        let session_name = ns.map(|n| n.tmux_session_name.clone());

        let result = self.stop_flow.execute(
            &mut self.registry,
            socket_path.as_deref(),
            session_name.as_deref(),
            &agent_names,
            force,
        );

        self.lifecycle.record(crate::services::lifecycle::LifecycleReport {
            project_id: self.project_id().to_string(),
            event: "stopped".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            details: serde_json::json!({"force": force, "reason": reason, "stopped_agents": result.stopped_agents}),
        });

        result
    }

    /// Run one heartbeat tick: health, supervision, dispatcher.
    pub fn heartbeat(&mut self) {
        self.health_monitor.bump_generation();
        let agents: Vec<String> = self.agent_names();

        // Supervision: decide which agents need restart.
        let needs_restart = self.supervision.tick(&agents);
        for agent_name in needs_restart {
            self.supervision
                .record_restart(&agent_name, "supervision_tick");
        }

        // Dispatcher tick.
        let _ = self.dispatcher.tick();
    }

    /// Dispatch a single RPC request string.
    pub fn handle_rpc(&mut self, raw: &str) -> String {
        let handlers = self.handlers.clone();
        protocol::handle_request(self, &handlers, raw)
    }

    pub fn agent_names(&self) -> Vec<String> {
        self.registry
            .all_entries()
            .iter()
            .map(|e| e.agent_name.clone())
            .collect()
    }

    pub fn persist_start_policy(&self, auto_permission: bool, source: &str) -> crate::Result<()> {
        let policy = crate::services::start_policy::StartPolicy {
            auto_permission,
            source: source.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.start_policy_store
            .save(&policy)
            .map_err(crate::DaemonError::Config)
    }

    /// Execute the start flow and persist the resulting namespace.
    pub fn run_start_flow(
        &mut self,
        agent_names: &[String],
        restore: bool,
        auto_permission: bool,
    ) -> Result<StartFlowResult, String> {
        let project_root = self.layout.project_root.clone();
        let (result, namespace) = self.start_flow.execute(
            &project_root,
            self.project_id(),
            &self.tmux_socket_path(),
            &self.tmux_session_name(),
            agent_names,
            restore,
            auto_permission,
        )?;

        for agent in &result.agent_results {
            if let Some(pane_id) = &agent.pane_id {
                self.registry.register(AgentRuntimeEntry {
                    agent_name: agent.agent_name.clone(),
                    provider: String::new(),
                    state: "idle".into(),
                    health: "healthy".into(),
                    pane_id: Some(pane_id.clone()),
                    workspace_path: Some(self.project_root.to_string_lossy().to_string()),
                    runtime_pid: None,
                    session_id: None,
                    restart_count: 0,
                });
            }
        }

        self.project_namespace.mount(namespace)?;
        self.supervision.record_success("daemon");

        // Update startup report with agent results.
        if let Some(report) = &mut self.last_startup_report {
            report.agent_results = result
                .agent_results
                .iter()
                .map(|a| CcbdStartupAgentResult {
                    agent_name: a.agent_name.clone(),
                    status: a.status.clone(),
                    reason: a.reason.clone(),
                    pane_id: a.pane_id.clone(),
                })
                .collect();
            let _ = ccb_storage::json::JsonStore::new()
                .save(&self.layout.ccbd_startup_report_path(), report);
        }

        Ok(result)
    }
}

fn summary_to_model(summary: &StopCleanupSummary) -> CcbdTmuxCleanupSummary {
    CcbdTmuxCleanupSummary {
        socket_name: summary.socket_name.clone(),
        killed_panes: summary.killed_panes.clone(),
        errors: summary.errors.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ccbd_app_lifecycle() {
        let dir = TempDir::new().unwrap();
        let app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        assert!(!app.is_shutdown_requested());
        app.request_shutdown();
        assert!(app.is_shutdown_requested());
    }

    #[test]
    fn test_startup_shutdown() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.start().unwrap();
        app.shutdown().unwrap();
    }

    #[test]
    fn test_handle_rpc_ping() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let response = app.handle_rpc(r#"{"op":"ping","request":{"target":"ccbd"}}"#);
        assert!(response.contains("pong"));
    }

    #[test]
    fn test_handle_rpc_ping_cli_shape() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let response = app.handle_rpc(r#"{"method":"ping","params":{"target":"ccbd"}}"#);
        assert!(response.contains("pong"));
        assert!(response.contains("\"result\""));
    }
}
