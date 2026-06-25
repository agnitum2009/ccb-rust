use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::adapters::completion::to_completion_job_record;
use crate::adapters::mailbox::{to_mailbox_completion_decision, to_mailbox_job_record};
use crate::handlers::{build_registry, HandlerRegistry};
use crate::models::lifecycle::{
    build_lifecycle, CcbdLifecycleUpdates, LIFECYCLE_DESIRED_STATE_RUNNING,
    LIFECYCLE_DESIRED_STATE_STOPPED, LIFECYCLE_PHASE_MOUNTED, LIFECYCLE_PHASE_STARTING,
    LIFECYCLE_PHASE_UNMOUNTED,
};
use crate::models::lifecycle::{
    CcbdShutdownReport, CcbdStartupAgentResult, CcbdStartupReport, CcbdTmuxCleanupSummary,
};
use crate::services::dispatcher::JobDispatcher;
use crate::services::health::HealthMonitor;
use crate::services::lifecycle::LifecycleStore;
use crate::services::ownership::OwnershipService;
use crate::services::project_namespace::ProjectNamespaceController;
use crate::services::registry::{AgentRegistry, AgentRuntimeEntry};
use crate::services::runtime::RuntimeService;
use ccbr_completion::models::{CompletionDecision, CompletionStatus};
use ccbr_completion::{CompletionRegistry, CompletionTrackerService};
use ccbr_jobs::JobStore;
use ccbr_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade};
use ccbr_provider_core::catalog::{build_default_provider_catalog, ProviderCatalog};
use ccbr_providers::execution::{ExecutionService, ExecutionStateStore, ProviderRuntimeContext};

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
    pub lifecycle: LifecycleStore,
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
    pub job_heartbeat: crate::services::job_heartbeat_runtime::JobHeartbeatRuntimeService,
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
        let execution_state_store = ExecutionStateStore::new(layout.clone());
        let lifecycle_store = LifecycleStore::new(layout.clone());

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
            health_monitor: HealthMonitor::new(Some(socket_path.clone())),
            lifecycle: lifecycle_store,
            supervision: SupervisionLoop::new(1000, 5),
            runtime_service: RuntimeService::new(),
            execution: ExecutionService::new(
                ccbr_providers::build_default_execution_registry(),
                || chrono::Utc::now().to_rfc3339(),
                Some(execution_state_store),
            ),
            completion_tracker: CompletionTrackerService::new(
                config.clone().unwrap_or_default(),
                build_default_provider_catalog(
                    false,
                    std::env::var("CCBR_INCLUDE_TEST_DOUBLES")
                        .map(|v| v == "1")
                        .unwrap_or(false),
                ),
                CompletionRegistry,
            ),
            job_heartbeat:
                crate::services::job_heartbeat_runtime::JobHeartbeatRuntimeService::with_defaults(),
            mailbox,
            mailbox_control,
            ownership: OwnershipService::with_state_path(
                layout.ccbrd_dir().join("ownership-state.json"),
            ),
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

    /// Build the runtime context used to restore/resume a job's provider
    /// execution from the current registry and namespace.
    fn build_runtime_context_for_job(
        &self,
        job: &crate::models::api_models::records::JobRecord,
    ) -> ProviderRuntimeContext {
        let entry = self.registry.get(&job.agent_name);
        let namespace = self.project_namespace.load();
        let runtime_ref = entry
            .and_then(|e| e.pane_id.clone())
            .or_else(|| namespace.and_then(|ns| ns.agent_panes.get(&job.agent_name).cloned()))
            .filter(|s| !s.is_empty());
        ProviderRuntimeContext {
            agent_name: job.agent_name.clone(),
            workspace_path: entry
                .and_then(|e| e.workspace_path.clone())
                .or_else(|| Some(self.project_root.to_string_lossy().to_string())),
            backend_type: Some("tmux".to_string()),
            runtime_ref,
            session_ref: None,
            runtime_pid: None,
            runtime_health: None,
            runtime_binding_source: None,
        }
    }

    /// Start the daemon: acquire ownership, restore running jobs, and publish
    /// the mounted lifecycle record.
    pub fn start(&mut self) -> crate::Result<()> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();
        let socket_path = self.socket_path();
        let instance_id = self.daemon_instance_id().to_string();
        let pid = std::process::id();

        // Load the previous lifecycle record (if any) so we can inherit the
        // shared startup deadline and startup_id across daemon restarts.
        let previous = self.lifecycle.load();
        let startup_id = previous
            .as_ref()
            .and_then(|l| l.startup_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
        let startup_deadline = previous
            .as_ref()
            .and_then(|l| l.startup_deadline_at.clone())
            .filter(|d| {
                chrono::DateTime::parse_from_rfc3339(d)
                    .map(|dt| chrono::Utc::now() < dt.with_timezone(&chrono::Utc))
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| {
                let budget_s: i64 = std::env::var("CCB_STARTUP_TRANSACTION_TIMEOUT_S")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30);
                (chrono::Utc::now() + chrono::Duration::seconds(budget_s)).to_rfc3339()
            });

        let starting = build_lifecycle(
            self.project_id(),
            &now_str,
            LIFECYCLE_DESIRED_STATE_RUNNING,
            LIFECYCLE_PHASE_STARTING,
            previous.as_ref().map(|l| l.generation).unwrap_or(0),
            CcbdLifecycleUpdates {
                startup_id: Some(Some(startup_id)),
                startup_stage: Some(Some("spawn_requested".into())),
                last_progress_at: Some(Some(now_str.clone())),
                startup_deadline_at: Some(Some(startup_deadline)),
                keeper_pid: Some(Some(pid)),
                owner_pid: Some(Some(pid)),
                owner_daemon_instance_id: Some(Some(instance_id.clone())),
                socket_path: Some(Some(socket_path.clone())),
                ..Default::default()
            },
        );
        let _ = self.lifecycle.save(&starting);

        // Re-establish ownership from durable state before acquiring a new guard.
        // This preserves the generation sequence and avoids duplicate guards when
        // the same daemon instance restarts.
        let generation = self
            .ownership
            .restore_or_acquire(pid, &socket_path, &instance_id)?
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

        // Restore running jobs from the previous daemon instance so trace shows
        // them and heartbeat/comms_recover can continue driving them.
        let running_jobs_path = self.layout.ccbrd_dir().join("running-jobs.json");
        self.dispatcher.restore_running_jobs(&running_jobs_path);

        // Re-register restored running jobs with the execution service and the
        // completion tracker so heartbeat poll() can drive them to completion
        // without a new submission.
        let running_jobs: Vec<_> = self
            .dispatcher
            .job_store
            .iter()
            .filter(|j| j.status == crate::models::api_models::common::JobStatus::Running)
            .cloned()
            .collect();
        let mut restore_results = Vec::new();
        for job in &running_jobs {
            let completion_job = to_completion_job_record(job);
            let ctx = self.build_runtime_context_for_job(job);
            let result = self.execution.restore(&completion_job, Some(&ctx));
            restore_results.push((job.clone(), completion_job, result));
        }

        let after_restore = chrono::Utc::now().to_rfc3339();
        for (job, completion_job, result) in &restore_results {
            if result.restored() {
                let _ = self
                    .completion_tracker
                    .start(completion_job, &after_restore);
                let mut prompt_patch = std::collections::HashMap::new();
                prompt_patch.insert(
                    "prompt_text".to_string(),
                    serde_json::Value::String(job.request.body.clone()),
                );
                self.execution.feed_runtime_state(&job.job_id, prompt_patch);
            } else if let Some(decision) = &result.decision {
                // terminal_pending: finish the job immediately without heartbeat.
                let status = map_completion_status(decision.status);
                let decision_record = decision_to_record(decision);
                self.dispatcher
                    .update_job_status(&job.job_id, status, Some(decision_record));
                self.execution.finish(&job.job_id);
                if status.is_terminal() {
                    let finished_at = decision
                        .finished_at
                        .as_deref()
                        .unwrap_or(&after_restore)
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

        let mounted = starting.with_phase(
            LIFECYCLE_PHASE_MOUNTED,
            chrono::Utc::now().to_rfc3339(),
            CcbdLifecycleUpdates {
                generation: Some(generation),
                startup_stage: Some(Some("mounted".into())),
                last_progress_at: Some(Some(chrono::Utc::now().to_rfc3339())),
                startup_deadline_at: Some(None),
                owner_pid: Some(Some(pid)),
                owner_daemon_instance_id: Some(Some(instance_id.clone())),
                socket_path: Some(Some(socket_path.clone())),
                ..Default::default()
            },
        );
        let _ = self.lifecycle.save(&mounted);

        Ok(())
    }

    /// Shut the daemon down: stop all agents and write a shutdown report.
    pub fn shutdown(&mut self) -> crate::Result<()> {
        // Persist running/non-terminal jobs *before* trying to stop agents so
        // that a slow or stuck stop flow never loses in-flight job memory.
        // Graceful shutdown leaves provider panes running so a restarted daemon
        // can adopt them and continue driving in-flight jobs (S3.3 continuity).
        let running_jobs_path = self.layout.ccbrd_dir().join("running-jobs.json");
        let _ = self.dispatcher.persist_running_jobs(&running_jobs_path);

        let result = self.stop_all(false, "shutdown");
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

        // Persist final lifecycle state so CLI waiters and diagnostics see the
        // daemon as stopped/unmounted after shutdown.
        if let Some(current) = self.lifecycle.load() {
            let unmounted = current.with_phase(
                LIFECYCLE_PHASE_UNMOUNTED,
                chrono::Utc::now().to_rfc3339(),
                CcbdLifecycleUpdates {
                    desired_state: Some(LIFECYCLE_DESIRED_STATE_STOPPED.into()),
                    owner_pid: Some(None),
                    owner_daemon_instance_id: Some(None),
                    socket_path: Some(Some(self.socket_path())),
                    namespace_epoch: Some(None),
                    startup_stage: Some(None),
                    last_progress_at: Some(Some(chrono::Utc::now().to_rfc3339())),
                    startup_deadline_at: Some(None),
                    last_failure_reason: Some(None),
                    ..Default::default()
                },
            );
            let _ = self.lifecycle.save(&unmounted);
        }

        // Persist ownership state so a restarted daemon can re-establish the
        // same guard and continue the generation sequence.
        let _ = self.ownership.save();

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

        // Mark the daemon as stopping while agents are torn down.
        if let Some(current) = self.lifecycle.load() {
            let stopping = current.with_phase(
                "stopping",
                chrono::Utc::now().to_rfc3339(),
                CcbdLifecycleUpdates {
                    desired_state: Some(LIFECYCLE_DESIRED_STATE_STOPPED.into()),
                    startup_stage: Some(None),
                    last_progress_at: Some(Some(chrono::Utc::now().to_rfc3339())),
                    startup_deadline_at: Some(None),
                    shutdown_intent: Some(Some(reason.to_string())),
                    last_failure_reason: Some(None),
                    ..Default::default()
                },
            );
            let _ = self.lifecycle.save(&stopping);
        }

        result
    }

    /// Run one heartbeat tick: health, supervision, dispatcher.
    pub fn heartbeat(&mut self) {
        self.health_monitor.bump_generation();

        // Keep the lifecycle progress timestamp fresh while the daemon is alive
        // so CLI waiters do not time out on a stalled startup progress clock.
        if let Some(current) = self.lifecycle.load() {
            if current.phase == LIFECYCLE_PHASE_STARTING || current.phase == LIFECYCLE_PHASE_MOUNTED
            {
                let now = chrono::Utc::now().to_rfc3339();
                if current.last_progress_at.as_deref() != Some(&now) {
                    let updated = current.with_updates(CcbdLifecycleUpdates {
                        last_progress_at: Some(Some(now)),
                        ..Default::default()
                    });
                    let _ = self.lifecycle.save(&updated);
                }
            }
        }
        let agents: Vec<String> = self.agent_names();

        // Supervision: inspect agent pane/session health and decide whether to
        // restart, escalate, or clear backoff state.
        let namespace = self.project_namespace.load().cloned();
        let tmux_socket_path = self.tmux_socket_path();
        let checker = crate::supervision::loop_runner::TmuxHealthChecker::new(
            &self.registry,
            namespace.as_ref(),
            &tmux_socket_path,
        );
        let decisions = self.supervision.tick(&agents, &checker);
        let mut restart_agents: Vec<String> = Vec::new();
        for decision in decisions {
            match decision {
                crate::supervision::loop_runner::SupervisionDecision::Restart {
                    agent_name,
                    reason,
                } => {
                    self.supervision.record_restart(&agent_name, &reason);
                    restart_agents.push(agent_name);
                }
                crate::supervision::loop_runner::SupervisionDecision::Escalate {
                    agent_name,
                    reason,
                } => {
                    eprintln!("ccbrd: supervision escalated {agent_name}: {reason}");
                    if let Some(entry) = self.registry.get_mut(&agent_name) {
                        entry.health = "degraded".into();
                        entry.state = "failed".into();
                    }
                }
            }
        }

        // Respawn the whole provider topology when any agent needs recovery.
        // This keeps the namespace consistent and avoids reusing phantom pane IDs.
        if !restart_agents.is_empty() {
            let all_agents: Vec<String> = self.agent_names();
            let config_windows = self.current_config.as_ref().and_then(|c| c.windows.clone());
            eprintln!(
                "ccbrd: respawning agents after supervision decisions: {:?}",
                restart_agents
            );
            if let Err(e) =
                self.run_start_flow(&all_agents, false, true, config_windows, None, None, &[])
            {
                eprintln!("ccbrd: respawn after supervision failed: {e}");
            }
        }

        // Fallback: detect externally-killed tmux panes that the supervision
        // loop may not have seen yet and respawn the whole project topology.
        if let Err(e) = self.respawn_dead_agents() {
            eprintln!("ccbrd: respawn_dead_agents failed: {e}");
        }

        // Dispatcher tick: promote queued jobs to running and start completion
        // trackers for newly-running jobs.
        let started = self.dispatcher.tick();
        let now = chrono::Utc::now().to_rfc3339();
        // Clone job records to avoid borrow conflicts when accessing
        // self.registry + self.execution inside the loop.
        let started_jobs: Vec<_> = started.to_vec();
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

        // Job heartbeat timeout: terminalize running jobs that have made no
        // progress for too many heartbeat notices, mirroring Python
        // `lib/ccbd/services/job_heartbeat_runtime/tick.py`.
        let running_jobs: Vec<_> = self
            .dispatcher
            .job_store
            .iter()
            .filter(|j| j.status == crate::models::api_models::common::JobStatus::Running)
            .cloned()
            .collect();
        for job in running_jobs {
            let tick_job = crate::tick::Job {
                job_id: job.job_id.clone(),
                agent_name: job.agent_name.clone(),
                updated_at: Some(job.updated_at.clone()),
                request: crate::tick::JobRequest {
                    from_actor: job.request.from_actor.clone(),
                },
            };
            let mut adapter =
                crate::services::job_heartbeat_runtime::JobHeartbeatDispatcherAdapter {
                    dispatcher: &mut self.dispatcher,
                };
            let timeout_finished_at = chrono::Utc::now().to_rfc3339();
            match crate::tick::tick_job_heartbeat(&self.job_heartbeat, &mut adapter, &tick_job) {
                Ok(false) => {
                    // Timeout terminalized the job; record mailbox terminal state.
                    crate::services::job_heartbeat_runtime::record_terminal_timeout(
                        &self.dispatcher,
                        &self.mailbox,
                        &job.job_id,
                        &timeout_finished_at,
                    );
                }
                Ok(true) => {}
                Err(e) => {
                    eprintln!("ccbrd: heartbeat tick failed for {}: {e}", job.job_id);
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
    #[allow(clippy::too_many_arguments)]
    pub fn run_start_flow(
        &mut self,
        agent_names: &[String],
        restore: bool,
        auto_permission: bool,
        config_windows: Option<Vec<ccbr_agents::models::WindowSpec>>,
        terminal_size: Option<(u32, u32)>,
        startup_timeout_s: Option<f64>,
        startup_args: &[String],
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
            terminal_size,
            startup_timeout_s,
            startup_args,
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
                terminal_size: None,
                startup_timeout_s: None,
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

    /// Check whether any agent's tmux pane has died (e.g. external kill-pane)
    /// and, if so, respawn the whole project topology.  This keeps the
    /// namespace consistent and avoids reusing phantom pane IDs.
    fn respawn_dead_agents(&mut self) -> Result<(), String> {
        let socket_path = self.tmux_socket_path();
        if !std::path::Path::new(&socket_path).exists() {
            return Ok(());
        }
        let backend = ccbr_terminal::TmuxBackend::new(None, Some(socket_path));
        self.respawn_dead_agents_with_checker(|pane_id| {
            backend
                .tmux_run_capture(&["display-message", "-p", "-t", pane_id, "#{pane_id}"])
                .map(|s| s.trim().starts_with('%'))
                .unwrap_or(false)
        })
    }

    /// Testable core of `respawn_dead_agents`: given a predicate that reports
    /// whether a pane id is alive, collect dead agents and respawn.
    fn respawn_dead_agents_with_checker<F>(&mut self, mut is_alive: F) -> Result<(), String>
    where
        F: FnMut(&str) -> bool,
    {
        let mut dead_agents: Vec<String> = Vec::new();
        for entry in self.registry.all_entries() {
            if let Some(pane_id) = entry.pane_id.as_deref() {
                if pane_id.is_empty() {
                    continue;
                }
                if !is_alive(pane_id) {
                    eprintln!(
                        "ccbrd: detected dead pane {} for agent {}, will respawn",
                        pane_id, entry.agent_name
                    );
                    dead_agents.push(entry.agent_name.clone());
                }
            }
        }
        if dead_agents.is_empty() {
            return Ok(());
        }
        // Respawn the whole topology so layouts stay consistent.
        let all_agents: Vec<String> = self.agent_names();
        let config_windows = self.current_config.as_ref().and_then(|c| c.windows.clone());
        eprintln!(
            "ccbrd: respawning agents after pane death: {:?}",
            dead_agents
        );
        let _result =
            self.run_start_flow(&all_agents, false, true, config_windows, None, None, &[])?;
        Ok(())
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
            patch.insert(
                "socket_path".to_string(),
                serde_json::Value::String(socket_path.clone()),
            );
            patch.insert(
                "pane_id".to_string(),
                serde_json::Value::String(pane_id.clone()),
            );
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

    #[test]
    fn test_respawn_dead_agents_with_checker_triggers_start_flow() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent1".into(),
            provider: "codex".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%42".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        // All panes considered dead -> should trigger a start flow respawn.
        app.respawn_dead_agents_with_checker(|_pane_id| false)
            .expect("respawn should succeed with stub backend");

        let entry = app.registry.get("agent1").expect("agent1 still registered");
        assert!(
            entry.pane_id.as_deref() != Some("%42"),
            "pane id should be updated after respawn, got {:?}",
            entry.pane_id
        );
    }

    #[test]
    fn test_respawn_dead_agents_with_checker_no_op_when_alive() {
        let dir = TempDir::new().unwrap();
        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        app.registry.register(AgentRuntimeEntry {
            agent_name: "agent1".into(),
            provider: "codex".into(),
            state: "idle".into(),
            health: "healthy".into(),
            pane_id: Some("%42".into()),
            workspace_path: None,
            runtime_pid: None,
            session_id: None,
            restart_count: 0,
        });

        // All panes considered alive -> no start flow, pane id unchanged.
        app.respawn_dead_agents_with_checker(|_pane_id| true)
            .expect("respawn check should succeed");

        let entry = app.registry.get("agent1").expect("agent1 still registered");
        assert_eq!(entry.pane_id.as_deref(), Some("%42"));
    }

    #[test]
    fn test_shutdown_persists_and_start_restores_running_jobs() {
        use crate::models::api_models::common::JobStatus;
        use crate::models::api_models::messages::MessageEnvelope;
        use crate::models::api_models::records::JobRecord;

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
"#,
        )
        .unwrap();

        let mut app = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );

        // Inject a running job directly into the dispatcher.
        let job = JobRecord {
            job_id: "job-1".into(),
            submission_id: None,
            agent_name: "agent1".into(),
            provider: "codex".into(),
            request: MessageEnvelope {
                project_id: "proj-1".into(),
                to_agent: "agent1".into(),
                from_actor: "user".into(),
                body: "keep running".into(),
                task_id: None,
                reply_to: None,
                message_type: "ask".into(),
                delivery_scope: crate::models::api_models::common::DeliveryScope::Single,
                silence_on_success: false,
                route_options: serde_json::json!({}),
                body_artifact: None,
            },
            status: JobStatus::Running,
            terminal_decision: None,
            cancel_requested_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            workspace_path: None,
            target_kind: crate::models::api_models::common::TargetKind::Agent,
            target_name: "agent1".into(),
        };
        app.dispatcher.job_store.push(job);
        app.dispatcher
            .state
            .rebuild(&app.dispatcher.job_store.clone());

        app.shutdown().expect("shutdown should succeed");

        // Simulate a fresh daemon instance in the same project.
        let mut restarted = CcbdApp::with_backend(
            dir.path(),
            StartFlowService::with_stub(),
            StopFlowService::with_stub(),
        );
        restarted.start().expect("start should succeed");

        let restored = restarted
            .dispatcher
            .get("job-1")
            .expect("running job should be restored");
        assert_eq!(restored.status, JobStatus::Running);
        assert_eq!(restored.agent_name, "agent1");
    }
}
