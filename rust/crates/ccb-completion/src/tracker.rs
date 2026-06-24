use std::collections::HashMap;

use ccb_agents::models::{ProjectConfig, RuntimeMode};

use crate::detectors::CompletionDetector;
use crate::error::{CompletionError, Result};
use crate::models::{
    CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, JobRecord, TargetKind,
};
use crate::profiles::CompletionManifestResolver;
use crate::registry::CompletionRegistry;
use crate::selectors::ReplySelector;
use crate::utils::seconds_between;

const DEFAULT_REQUEST_TIMEOUT_S: f64 = 3600.0;
const DISABLED_REQUEST_BINDING_TIMEOUT_S: f64 = 31_536_000.0;

struct ActiveTracker {
    agent_name: String,
    detector: Box<dyn CompletionDetector + Send>,
    selector: Box<dyn ReplySelector + Send>,
    started_at: String,
    timeout_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionTrackerView {
    pub job_id: String,
    pub agent_name: String,
    pub state: CompletionState,
    pub decision: CompletionDecision,
}

pub struct CompletionTrackerService<R: CompletionManifestResolver> {
    config: ProjectConfig,
    resolver: R,
    registry: CompletionRegistry,
    request_timeout_s: f64,
    trackers: HashMap<String, ActiveTracker>,
}

impl<R: CompletionManifestResolver> CompletionTrackerService<R> {
    pub fn new(config: ProjectConfig, resolver: R, registry: CompletionRegistry) -> Self {
        Self {
            config,
            resolver,
            registry,
            request_timeout_s: DEFAULT_REQUEST_TIMEOUT_S,
            trackers: HashMap::new(),
        }
    }

    pub fn with_request_timeout_s(mut self, timeout_s: f64) -> Self {
        self.request_timeout_s = timeout_s;
        self
    }

    pub fn start(&mut self, job: &JobRecord, started_at: &str) -> Result<CompletionTrackerView> {
        let (agent_name, manifest) = if job.target_kind == TargetKind::Agent {
            let spec = self
                .config
                .agents
                .get(&job.agent_name.to_lowercase())
                .ok_or_else(|| {
                    CompletionError::Validation(format!("unknown agent {:?}", job.agent_name))
                })?;
            let manifest = self
                .resolver
                .resolve_completion_manifest(&spec.provider, &spec.runtime_mode)?;
            (job.agent_name.clone(), manifest)
        } else {
            let manifest = self
                .resolver
                .resolve_completion_manifest(&job.provider, &RuntimeMode::PaneBacked)?;
            let name = if job.agent_name.trim().is_empty() {
                job.provider.clone()
            } else {
                job.agent_name.clone()
            };
            (name, manifest)
        };

        let agent_name_norm = ccb_storage::path_helpers::normalize_agent_name(&agent_name)
            .map_err(|e| CompletionError::Validation(format!("invalid agent name: {e}")))?;

        let agent_spec = ccb_agents::models::AgentSpec {
            name: agent_name_norm.clone(),
            provider: manifest.provider.clone(),
            target: "default".into(),
            workspace_mode: ccb_agents::models::WorkspaceMode::Inplace,
            workspace_root: None,
            runtime_mode: match manifest.runtime_mode.as_str() {
                "pty-backed" => RuntimeMode::PtyBacked,
                "headless" => RuntimeMode::Headless,
                _ => RuntimeMode::PaneBacked,
            },
            restore_default: ccb_agents::models::RestoreMode::Fresh,
            permission_default: ccb_agents::models::PermissionMode::Manual,
            queue_policy: ccb_agents::models::QueuePolicy::SerialPerAgent,
            workspace_path: None,
            workspace_group: None,
            provider_command_template: None,
            model: None,
            startup_args: Vec::new(),
            env: HashMap::new(),
            api: ccb_agents::models::AgentApiSpec::default(),
            provider_profile: ccb_agents::models::ProviderProfileSpec::default(),
            branch_template: None,
            labels: Vec::new(),
            description: None,
            role: None,
            watch_paths: Vec::new(),
        };

        let profile = self.registry.build_profile(&agent_spec, None, &manifest)?;
        let mut detector = self.registry.build_detector(&profile);
        let selector = self.registry.build_selector(&profile);

        let binding_timeout_s = if self.request_timeout_s > 0.0 {
            self.request_timeout_s
        } else {
            DISABLED_REQUEST_BINDING_TIMEOUT_S
        };

        detector.bind(
            CompletionRequestContext::new(
                job.job_id.clone(),
                &agent_name_norm,
                &manifest.provider,
                binding_timeout_s,
            )?,
            CompletionCursor::new(profile.completion_source_kind, started_at),
        );

        self.trackers.insert(
            job.job_id.clone(),
            ActiveTracker {
                agent_name: agent_name_norm,
                detector,
                selector,
                started_at: started_at.to_string(),
                timeout_s: self.request_timeout_s,
            },
        );

        self.current(&job.job_id)
            .ok_or_else(|| CompletionError::Validation("tracker disappeared after start".into()))
    }

    pub fn current(&self, job_id: &str) -> Option<CompletionTrackerView> {
        let tracker = self.trackers.get(job_id)?;
        let mut decision = tracker.detector.decision();
        let reply = if decision.terminal {
            tracker.selector.select(&decision)
        } else {
            tracker.selector.preview()
        };
        if !reply.is_empty() && decision.reply.is_empty() {
            decision = decision.with_reply(reply);
        }
        Some(CompletionTrackerView {
            job_id: job_id.to_string(),
            agent_name: tracker.agent_name.clone(),
            state: tracker.detector.state(),
            decision,
        })
    }

    pub fn ingest(&mut self, job_id: &str, item: &CompletionItem) -> Result<CompletionTrackerView> {
        let tracker = self.require(job_id)?;
        if item.kind == CompletionItemKind::SessionRotate {
            tracker.selector.reset();
        }
        for candidate in crate::models::reply_candidates_from_item(item) {
            tracker.selector.ingest_candidate(candidate);
        }
        tracker.detector.ingest(item);
        self.current(job_id)
            .ok_or_else(|| CompletionError::UnknownTracker(job_id.into()))
    }

    pub fn tick(&mut self, job_id: &str, now: &str) -> Result<CompletionTrackerView> {
        let tracker = self.require(job_id)?;
        let cursor = tracker.detector.state().latest_cursor;
        tracker.detector.tick(now, cursor.as_ref());
        maybe_finalize_timeout(tracker, now, tracker.timeout_s);
        self.current(job_id)
            .ok_or_else(|| CompletionError::UnknownTracker(job_id.into()))
    }

    pub fn tick_all(&mut self, now: &str) -> Vec<CompletionTrackerView> {
        let job_ids: Vec<String> = self.trackers.keys().cloned().collect();
        job_ids
            .into_iter()
            .filter_map(|job_id| self.tick(&job_id, now).ok())
            .collect()
    }

    pub fn finish(&mut self, job_id: &str) {
        self.trackers.remove(job_id);
    }

    fn require(&mut self, job_id: &str) -> Result<&mut ActiveTracker> {
        self.trackers
            .get_mut(job_id)
            .ok_or_else(|| CompletionError::UnknownTracker(job_id.into()))
    }
}

fn maybe_finalize_timeout(tracker: &mut ActiveTracker, now: &str, timeout_s: f64) {
    if timeout_s <= 0.0 {
        return;
    }
    if tracker.detector.decision().terminal {
        return;
    }
    let elapsed = match seconds_between(&tracker.started_at, now) {
        Ok(v) => v,
        Err(_) => return,
    };
    if elapsed < timeout_s {
        return;
    }
    let cursor = tracker.detector.state().latest_cursor;
    tracker.detector.finalize_timeout(now, cursor.as_ref());
}
