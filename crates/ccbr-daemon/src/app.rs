use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::adapters::completion::to_completion_job_record;
use crate::adapters::mailbox::{to_mailbox_completion_decision, to_mailbox_job_record};
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
use ccbr_completion::models::{CompletionDecision, CompletionStatus};
use ccbr_completion::{CompletionRegistry, CompletionTrackerService};
use ccbr_jobs::JobStore;
use ccbr_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccbr_provider_core::catalog::{build_default_provider_catalog, ProviderCatalog};
use ccbr_providers::execution::{ExecutionService, ProviderRuntimeContext};

fn load_agent_registry(
    layout: &ccbr_storage::paths::PathLayout,
    config: Option<&ccbr_agents::models::ProjectConfig>,
) -> (AgentRegistry, Vec<String>) {
    let Some(config) = config else {
        // No project config yet; start with an empty registry so the start
        // flow can create agents on demand.
        return (AgentRegistry::new(), Vec::new());
    };

    let names: Vec<String> = if config.default_agents.is_empty() {
        config.agents.keys().cloned().collect()
    } else {
        config.default_agents.clone()
    };
    let mut registry = AgentRegistry::new();
    for name in &names {
        let spec = config.agents.get(name);
        let workspace_path = spec
            .and_then(|s| s.workspace_path.clone())
            .unwrap_or_else(|| layout.project_root.to_string());
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
use crate::services::start_policy::StartPolicyStore;
use crate::socket_server::protocol;
use crate::start_flow::service::{StartFlowResult, StartFlowService};
use crate::stop_flow::service::{StopCleanupSummary, StopFlowResult, StopFlowService};
use crate::supervision::loop_runner::SupervisionLoop;

/// The main CCBR daemon application.
pub struct CcbdApp {
    pub project_root: PathBuf,
    pub layout: ccbr_storage::paths::PathLayout,
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
    pub execution: ExecutionService,
    pub mailbox: MessageBureauFacade,
    pub mailbox_control: MessageBureauControlService,
    pub ownership: OwnershipService,
    pub start_policy_store: StartPolicyStore,
    pub fault_service: crate::fault_injection::FaultInjectionService,
    pub last_startup_report: Option<CcbdStartupReport>,
    pub last_shutdown_report: Option<CcbdShutdownReport>,
    pub current_config: Option<ccbr_agents::models::ProjectConfig>,
    pub completion_tracker: CompletionTrackerService<ProviderCatalog>,
    pub daemon_instance_id: String,
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
        let layout = ccbr_storage::paths::PathLayout::new(
            Utf8Path::from_path(&project_root).unwrap_or(Utf8Path::new("/")),
        );
        let config_result = if layout.ccbr_dir().join("ccbr.config").exists() {
            ccbr_agents::config::load_project_config(&layout).ok()
        } else {
            None
        };
        if config_result.is_none() && layout.ccbr_dir().join("ccbr.config").exists() {
            tracing::warn!("failed to load project config");
        }
        let config = config_result.map(|r| r.config);
        let current_config = config.clone();
        let (registry, agent_names) = load_agent_registry(&layout, config.as_ref());
        let config_value = config.as_ref().and_then(|c| serde_json::to_value(c).ok());
        let socket_path = layout.ccbrd_socket_path().as_str().to_string();
        let daemon_instance_id = uuid::Uuid::new_v4().simple().to_string();
        let shared_job_store = JobStore::new(&layout);

        let mailbox = MessageBureauFacade::new(
            layout.clone(),
            config_value.clone(),
            Arc::new(|| chrono::Utc::now().to_rfc3339()),
        );
        let mailbox_control = MessageBureauControlService::from_facade(
            &mailbox,
            config_value,
            Some(shared_job_store.clone()),
            None,
        );

        Self {
            project_root,
            layout: layout.clone(),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            registry,
            dispatcher: JobDispatcher::new(agent_names).with_mailbox_job_store(shared_job_store),
            handlers: Arc::new(build_registry()),
            project_namespace: ProjectNamespaceController::new(&layout),
            start_flow,
            stop_flow,
            health_monitor: HealthMonitor::new(Some(socket_path)),
            lifecycle: LifecycleService::new(),
            supervision: SupervisionLoop::new(1000, 5),
            runtime_service: RuntimeService::new(),
            execution: ExecutionService::new(
                ccbr_providers::build_default_execution_registry(),
                || chrono::Utc::now().to_rfc3339(),
                None,
            ),
            completion_tracker: CompletionTrackerService::new(
                config.clone().unwrap_or_default(),
                build_default_provider_catalog(false, false),
                CompletionRegistry,
            ),
            mailbox,
            mailbox_control,
            ownership: OwnershipService::new(),
            start_policy_store: StartPolicyStore::new(&layout),
            fault_service: crate::fault_injection::FaultInjectionService::new(),
            last_startup_report: None,
            last_shutdown_report: None,
            current_config,
            daemon_instance_id,
        }
    }

    pub fn project_id(&self) -> &str {
        self.layout.project_id()
    }

    pub fn socket_path(&self) -> String {
        self.layout.ccbrd_socket_path().as_str().to_string()
    }

    pub fn daemon_instance_id(&self) -> &str {
        &self.daemon_instance_id
    }

    pub fn tmux_socket_path(&self) -> String {
        self.layout.ccbrd_tmux_socket_path().as_str().to_string()
    }

    pub fn tmux_session_name(&self) -> String {
        self.layout.ccbrd_tmux_session_name()
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
        let instance_id = self.daemon_instance_id().to_string();
        let generation = self
            .ownership
            .acquire(std::process::id(), &socket_path, &instance_id)
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
        let report_path = self.layout.ccbrd_startup_report_path();
        self.last_startup_report = Some(report.clone());
        let _ = ccbr_storage::json::JsonStore::new().save(&report_path, &report);

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
        let report_path = self.layout.ccbrd_shutdown_report_path();
        self.last_shutdown_report = Some(report.clone());
        let _ = ccbr_storage::json::JsonStore::new().save(&report_path, &report);
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

        // Dispatcher tick: promote queued jobs to running and start completion
        // trackers for newly-running jobs.
        let started = self.dispatcher.tick();
        let now = chrono::Utc::now().to_rfc3339();
        // Clone job records to avoid borrow conflicts when accessing
        // self.registry + self.execution inside the loop.
        let started_jobs: Vec<_> = started
            .iter()
            .map(|j| j.clone())
            .collect();
        for job in &started_jobs {
            let completion_job = to_completion_job_record(job);
            let _ = self.completion_tracker.start(&completion_job, &now);
            // Register with execution service so heartbeat poll() can track
            // provider output and drive completion detection.
            let entry = self.registry.get(&job.agent_name);
            let ctx = ProviderRuntimeContext {
                agent_name: job.agent_name.clone(),
                workspace_path: entry.and_then(|e| e.workspace_path.clone()),
                backend_type: Some("tmux".to_string()),
                runtime_ref: entry.and_then(|e| e.pane_id.clone()),
                ..Default::default()
            };
            let _ = self.execution.start(&completion_job, Some(&ctx));
            // Feed the prompt text from the job's request body so the adapter's
            // deferred-prompt dispatch can send it to the provider pane.
            let mut prompt_patch = std::collections::HashMap::new();
            prompt_patch.insert(
                "prompt_text".to_string(),
                serde_json::Value::String(job.request.body.clone()),
            );
            self.execution
                .feed_runtime_state(&completion_job.job_id, prompt_patch);
        }

        // Feed live tmux pane text into active execution submissions so adapters
        // that detect completion from pane output can make progress.
        self.feed_active_pane_text_to_execution();

        // Execution service poll: update job statuses from provider adapters.
        // Also ingest provider items into the completion tracker so the
        // orchestrator can settle on a terminal decision.
        let updates = self.execution.poll();
        eprintln!("DEBUG heartbeat: poll_updates={} active_contexts={}", updates.len(), self.execution.active_contexts().len());
        for u in &updates {
            eprintln!("DEBUG heartbeat: job={} items={} decision={:?}", u.job_id, u.items.len(), u.decision.as_ref().map(|d| (&d.status, d.terminal)));
        }
        for update in updates {
            // Ensure every running job has a tracker (handles restore/restart).
            if self.completion_tracker.current(&update.job_id).is_none() {
                if let Some(job) = self.dispatcher.get(&update.job_id) {
                    let completion_job = to_completion_job_record(job);
                    let _ = self.completion_tracker.start(&completion_job, &now);
                }
            }

            for item in &update.items {
                let _ = self.completion_tracker.ingest(&update.job_id, item);
            }

            let tracker_decision = self
                .completion_tracker
                .tick(&update.job_id, &now)
                .ok()
                .filter(|view| view.decision.terminal)
                .map(|view| view.decision);

            let effective_decision = tracker_decision.or(update.decision);
            let status = effective_decision
                .as_ref()
                .map(|d| map_completion_status(d.status))
                .unwrap_or(crate::models::api_models::common::JobStatus::Running);
            let decision_record = effective_decision.as_ref().map(decision_to_record);
            self.dispatcher
                .update_job_status(&update.job_id, status, decision_record);

            // Persist terminal completion decisions to the mailbox layer.
            if let Some(decision) = effective_decision.as_ref() {
                if decision.terminal {
                    self.completion_tracker.finish(&update.job_id);
                    if let Some(job) = self.dispatcher.get(&update.job_id) {
                        let finished_at = decision
                            .finished_at
                            .as_deref()
                            .unwrap_or(&chrono::Utc::now().to_rfc3339())
                            .to_string();
                        let mailbox_job = to_mailbox_job_record(job);
                        let mailbox_decision = to_mailbox_completion_decision(decision);
                        let _ = self.mailbox.record_terminal(
                            &mailbox_job,
                            &mailbox_decision,
                            &finished_at,
                            true,
                            true,
                        );
                    }
                }
            }
        }
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
            recovery_restore: true,
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
        config_windows: Option<Vec<ccbr_agents::models::WindowSpec>>,
    ) -> Result<StartFlowResult, String> {
        let project_root = self.layout.project_root.clone();
        // Load any existing namespace so panes can be reused on restore.
        let _ = self.project_namespace.load_from_disk();
        let namespace_agent_panes = self
            .project_namespace
            .load()
            .map(|ns| ns.agent_panes.clone());
        let (result, namespace) = self.start_flow.execute(
            &project_root,
            self.project_id(),
            &self.tmux_socket_path(),
            &self.tmux_session_name(),
            agent_names,
            &self.registry,
            restore,
            auto_permission,
            namespace_agent_panes.as_ref(),
            config_windows,
        )?;

        for agent in &result.agent_results {
            if let Some(pane_id) = &agent.pane_id {
                self.registry.update_pane_id(&agent.agent_name, pane_id);
            }
        }

        // P0: Launch provider CLIs into their assigned panes.
        // Collect provider info first to avoid borrow conflicts.
        let agent_launches: Vec<(String, String, String, Option<String>)> = result
            .agent_results
            .iter()
            .filter_map(|agent| {
                let pane_id = agent.pane_id.as_ref()?;
                let entry = self.registry.get(&agent.agent_name)?;
                Some((
                    agent.agent_name.clone(),
                    pane_id.clone(),
                    entry.provider.clone(),
                    entry.workspace_path.clone(),
                ))
            })
            .collect();

        let launcher = crate::provider_launcher::ProviderLauncher::new();
        let socket = self.tmux_socket_path();
        let project_id = self.project_id().to_string();
        let project_root_str = project_root.to_string();

        for (agent_name, pane_id, provider, workspace) in &agent_launches {
            if provider.trim().is_empty() {
                continue;
            }
            let ws = workspace.as_deref().unwrap_or(project_root_str.as_str());
            let ctx = crate::provider_launcher::LaunchContext {
                provider: provider.as_str(),
                agent_name: agent_name.as_str(),
                project_id: project_id.as_str(),
                project_root: project_root_str.as_str(),
                workspace_path: ws,
                pane_id: pane_id.as_str(),
                socket_path: socket.as_str(),
                restore,
                command_template: None,
                startup_args: &[],
                auto_permission,
                spec: None,
            };
            if let Err(e) = launcher.launch(&ctx) {
                eprintln!("ccbrd: provider launch failed for {agent_name}: {e}");
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
            let _ = ccbr_storage::json::JsonStore::new()
                .save(&self.layout.ccbrd_startup_report_path(), report);
        }

        Ok(result)
    }
}

impl CcbdApp {
    /// Capture the current text of each active execution pane and push it into
    /// the provider adapter's runtime state as `reply_buffer`.
    fn feed_active_pane_text_to_execution(&mut self) {
        let socket_path = self.tmux_socket_path();
        if !std::path::Path::new(&socket_path).exists() {
            return;
        }
        let backend = ccbr_terminal::TmuxBackend::new(None, Some(socket_path.clone()));
        let contexts = self.execution.active_contexts();
        for (job_id, context) in contexts {
            let pane_id = context.runtime_ref.unwrap_or_default();
            if pane_id.is_empty() {
                continue;
            }
            let text = backend
                .tmux_run_capture(&["capture-pane", "-p", "-t", &pane_id])
                .unwrap_or_default();
            let text = ccbr_terminal::TmuxBackend::strip_ansi(&text);
            let mut patch = std::collections::HashMap::new();
            patch.insert("reply_buffer".to_string(), serde_json::Value::String(text));
            patch.insert("socket_path".to_string(), serde_json::Value::String(socket_path.clone()));
            patch.insert("pane_id".to_string(), serde_json::Value::String(pane_id.clone()));
            self.execution.feed_runtime_state(&job_id, patch);
        }
    }
}

pub(crate) fn map_completion_status(
    status: CompletionStatus,
) -> crate::models::api_models::common::JobStatus {
    use crate::models::api_models::common::JobStatus;
    match status {
        CompletionStatus::Completed => JobStatus::Completed,
        CompletionStatus::Cancelled => JobStatus::Cancelled,
        CompletionStatus::Failed => JobStatus::Failed,
        CompletionStatus::Incomplete => JobStatus::Incomplete,
    }
}

pub(crate) fn decision_to_record(decision: &CompletionDecision) -> serde_json::Value {
    serde_json::json!({
        "terminal": decision.terminal,
        "status": decision.status,
        "reason": decision.reason,
        "reply": decision.reply,
        "finished_at": decision.finished_at,
    })
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
    fn test_ccbrd_app_lifecycle() {
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
        let response = app.handle_rpc(r#"{"op":"ping","request":{"target":"ccbrd"}}"#);
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
        let response = app.handle_rpc(r#"{"method":"ping","params":{"target":"ccbrd"}}"#);
        assert!(response.contains("pong"));
        assert!(response.contains("\"result\""));
    }

    #[test]
    fn test_loads_project_config_into_registry() {
        let dir = TempDir::new().unwrap();
        let ccbr_dir = dir.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr_dir).unwrap();
        std::fs::write(
            ccbr_dir.join("ccbr.config"),
            r#"version = 2
default_agents = ["agent1"]

[agents.agent1]
provider = "codex"
target = "agent1"

[windows]
main = "agent1:codex"
"#,
        )
        .unwrap();

        let app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        let entry = app
            .registry
            .get("agent1")
            .expect("agent1 should be registered");
        assert_eq!(entry.provider, "codex");
        assert_eq!(app.dispatcher.agent_names, vec!["agent1"]);
    }
}
