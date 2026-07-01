use std::collections::HashMap;

use ccb_agents::models::{AgentSpec, ProjectConfig, RuntimeMode};
use ccb_completion::{
    build_completion_profile, CompletionConfidence, CompletionCursor, CompletionDecision,
    CompletionDetector, CompletionFamily, CompletionItem, CompletionItemKind, CompletionManifest,
    CompletionOrchestrator, CompletionRegistry, CompletionRequestContext, CompletionSnapshot,
    CompletionSnapshotStore, CompletionSourceKind, CompletionState, CompletionStatus,
    CompletionTrackerService, FinalMessageSelector, JobRecord, ProtocolTurnDetector,
    ReplyCandidate, ReplyCandidateKind, ReplySelector, SelectorFamily, SessionBoundaryDetector,
    StructuredResultDetector, TerminalTextQuietDetector,
};

fn ts() -> &'static str {
    "2024-01-01T00:00:00Z"
}

fn cursor() -> CompletionCursor {
    CompletionCursor::new(CompletionSourceKind::ProtocolEventStream, ts())
}

fn make_item(kind: CompletionItemKind) -> CompletionItem {
    CompletionItem::new(kind, ts(), cursor(), "claude", "agent1", "job-1").unwrap()
}

fn item_with_text(kind: CompletionItemKind, key: &str, text: &str) -> CompletionItem {
    let mut item = make_item(kind);
    item.payload.insert(key.into(), text.into());
    item
}

fn manifest_for(
    family: CompletionFamily,
    selector: SelectorFamily,
    source: CompletionSourceKind,
) -> CompletionManifest {
    CompletionManifest::new("claude", "pane-backed")
        .unwrap()
        .with_completion_family(family)
        .with_completion_source_kind(source)
        .with_selector_family(selector)
}

fn agent_spec() -> AgentSpec {
    AgentSpec {
        name: "agent1".into(),
        provider: "claude".into(),
        target: "default".into(),
        workspace_mode: ccb_agents::models::WorkspaceMode::Inplace,
        workspace_root: None,
        runtime_mode: RuntimeMode::PaneBacked,
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
    }
}

fn project_config() -> ProjectConfig {
    let mut config = ProjectConfig::default();
    config.agents.insert("agent1".into(), agent_spec());
    config
}

fn resolver_for(manifest: CompletionManifest) -> HashMap<(String, String), CompletionManifest> {
    let mut map = HashMap::new();
    map.insert(("claude".into(), "pane-backed".into()), manifest);
    map
}

// ---------------------------------------------------------------------------
// Model / utility tests
// ---------------------------------------------------------------------------

#[test]
fn reply_candidates_extracted_from_item() {
    let item = item_with_text(CompletionItemKind::Result, "reply", "hello");
    let candidates = ccb_completion::reply_candidates_from_item(&item);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].kind, ReplyCandidateKind::FinalAnswer);
    assert_eq!(candidates[0].text, "hello");
}

#[test]
fn reply_candidate_priority_ordering() {
    let mut selector = FinalMessageSelector::default();
    selector.ingest_candidate(
        ReplyCandidate::new(ReplyCandidateKind::FallbackText, "fallback", ts()).unwrap(),
    );
    selector.ingest_candidate(
        ReplyCandidate::new(ReplyCandidateKind::FinalAnswer, "final", ts()).unwrap(),
    );
    selector.ingest_candidate(
        ReplyCandidate::new(ReplyCandidateKind::AssistantChunkMerged, "chunk", ts()).unwrap(),
    );

    let terminal = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("done".into()),
        confidence: Some(CompletionConfidence::Exact),
        ..CompletionDecision::pending(None)
    };
    let reply = selector.select(&terminal);
    assert_eq!(reply, "final");
    assert_eq!(selector.preview(), "final");
}

#[test]
fn selector_uses_latest_for_same_priority() {
    let mut selector = FinalMessageSelector::default();
    selector.ingest_candidate(
        ReplyCandidate::new(
            ReplyCandidateKind::FinalAnswer,
            "first",
            "2024-01-01T00:00:00Z",
        )
        .unwrap(),
    );
    selector.ingest_candidate(
        ReplyCandidate::new(
            ReplyCandidateKind::FinalAnswer,
            "second",
            "2024-01-01T00:00:01Z",
        )
        .unwrap(),
    );
    let terminal = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("done".into()),
        confidence: Some(CompletionConfidence::Exact),
        ..CompletionDecision::pending(None)
    };
    assert_eq!(selector.select(&terminal), "second");
}

#[test]
fn selector_reset_clears_candidates() {
    let mut selector = FinalMessageSelector::default();
    selector.ingest_candidate(
        ReplyCandidate::new(ReplyCandidateKind::FinalAnswer, "final", ts()).unwrap(),
    );
    selector.reset();
    let terminal = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("done".into()),
        confidence: Some(CompletionConfidence::Exact),
        ..CompletionDecision::pending(None)
    };
    assert_eq!(selector.select(&terminal), "");
}

// ---------------------------------------------------------------------------
// Detector tests
// ---------------------------------------------------------------------------

#[test]
fn protocol_turn_detector_completes_on_boundary() {
    let mut detector = ProtocolTurnDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );

    detector.ingest(&item_with_text(
        CompletionItemKind::AssistantChunk,
        "text",
        "partial",
    ));
    assert!(!detector.decision().terminal);

    let boundary = item_with_text(CompletionItemKind::TurnBoundary, "reply", "final answer");
    detector.ingest(&boundary);
    assert!(detector.decision().terminal);
    assert_eq!(detector.decision().status, CompletionStatus::Completed);
    assert_eq!(detector.decision().reply, "final answer");
}

#[test]
fn protocol_turn_detector_empty_boundary_without_anchor_is_delivery_late_empty() {
    let mut detector = ProtocolTurnDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    detector.ingest(&make_item(CompletionItemKind::TurnBoundary));
    assert!(detector.decision().terminal);
    assert_eq!(detector.decision().status, CompletionStatus::Incomplete);
    assert_eq!(
        detector.decision().reason.as_deref(),
        Some("delivery_late_empty")
    );
    let diagnostics = &detector.decision().diagnostics;
    assert_eq!(
        diagnostics
            .get("empty_reply_reason")
            .and_then(|v| v.as_str()),
        Some("delivery_late_empty")
    );
    assert!(diagnostics
        .get("diagnosis")
        .and_then(|v| v.as_str())
        .unwrap()
        .contains("before the request anchor was observed"));
}

#[test]
fn protocol_turn_detector_empty_boundary_with_anchor_is_model_empty_output() {
    let mut detector = ProtocolTurnDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    detector.ingest(&make_item(CompletionItemKind::AnchorSeen));
    detector.ingest(&make_item(CompletionItemKind::TurnBoundary));
    assert!(detector.decision().terminal);
    assert_eq!(detector.decision().status, CompletionStatus::Incomplete);
    assert_eq!(
        detector.decision().reason.as_deref(),
        Some("model_empty_output")
    );
    assert_eq!(
        detector
            .decision()
            .diagnostics
            .get("empty_reply_reason")
            .and_then(|v| v.as_str()),
        Some("model_empty_output")
    );
}

#[test]
fn protocol_turn_detector_empty_boundary_after_api_error_is_api_empty_after_error() {
    let mut detector = ProtocolTurnDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    detector.ingest(&make_item(CompletionItemKind::AnchorSeen));
    let mut boundary = make_item(CompletionItemKind::TurnBoundary);
    boundary
        .payload
        .insert("api_error_seen".into(), true.into());
    detector.ingest(&boundary);
    assert!(detector.decision().terminal);
    assert_eq!(detector.decision().status, CompletionStatus::Incomplete);
    assert_eq!(
        detector.decision().reason.as_deref(),
        Some("api_empty_after_error")
    );
}

#[test]
fn structured_result_detector_completes_on_result() {
    let mut detector = StructuredResultDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    detector.ingest(&item_with_text(
        CompletionItemKind::AssistantChunk,
        "text",
        "partial",
    ));
    assert!(!detector.decision().terminal);

    detector.ingest(&item_with_text(
        CompletionItemKind::Result,
        "reply",
        "structured result",
    ));
    assert!(detector.decision().terminal);
    assert_eq!(detector.decision().status, CompletionStatus::Completed);
    assert_eq!(detector.decision().reply, "structured result");
}

#[test]
fn session_boundary_detector_observed_completion() {
    let mut detector = SessionBoundaryDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    detector.ingest(&item_with_text(
        CompletionItemKind::AssistantChunk,
        "text",
        "partial",
    ));
    assert!(!detector.decision().terminal);

    detector.ingest(&item_with_text(
        CompletionItemKind::TurnBoundary,
        "reply",
        "done",
    ));
    assert!(detector.decision().terminal);
    assert_eq!(
        detector.decision().confidence,
        Some(CompletionConfidence::Observed)
    );
}

#[test]
fn anchored_session_stability_settles_after_window() {
    use ccb_completion::AnchoredSessionStabilityDetector;
    let mut detector = AnchoredSessionStabilityDetector::new(2.0);
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );

    let mut snapshot = item_with_text(CompletionItemKind::SessionSnapshot, "reply", "stable reply");
    snapshot.payload.insert("message_id".into(), "msg-1".into());
    detector.ingest(&snapshot);
    assert!(!detector.decision().terminal);

    detector.tick("2024-01-01T00:00:01Z", None);
    assert!(!detector.decision().terminal);

    detector.tick("2024-01-01T00:00:03Z", None);
    assert!(detector.decision().terminal);
    assert_eq!(
        detector.decision().reason,
        Some("session_reply_stable".into())
    );
}

#[test]
fn terminal_text_quiet_done_marker() {
    let mut detector = TerminalTextQuietDetector::default();
    detector.bind(
        CompletionRequestContext::new("job-1", "agent1", "claude", 60.0).unwrap(),
        cursor(),
    );
    let mut item = item_with_text(CompletionItemKind::AssistantChunk, "text", "final text");
    item.payload.insert("done_marker".into(), true.into());
    detector.ingest(&item);
    assert!(detector.decision().terminal);
    assert_eq!(
        detector.decision().reason,
        Some("terminal_done_marker".into())
    );
}

// ---------------------------------------------------------------------------
// Registry / profile tests
// ---------------------------------------------------------------------------

#[test]
fn registry_builds_detector_and_selector() {
    let registry = CompletionRegistry;
    let manifest = manifest_for(
        CompletionFamily::ProtocolTurn,
        SelectorFamily::FinalMessage,
        CompletionSourceKind::ProtocolEventStream,
    );
    let profile = registry
        .build_profile(&agent_spec(), None, &manifest)
        .unwrap();
    let _detector = registry.build_detector(&profile);
    let _selector = registry.build_selector(&profile);
}

#[test]
fn profile_builder_validates_provider() {
    let mut spec = agent_spec();
    spec.provider = "codex".into();
    let manifest = manifest_for(
        CompletionFamily::ProtocolTurn,
        SelectorFamily::FinalMessage,
        CompletionSourceKind::ProtocolEventStream,
    );
    let err = build_completion_profile(&spec, &manifest).unwrap_err();
    assert!(err.to_string().contains("agent provider"));
}

// ---------------------------------------------------------------------------
// Tracker service tests
// ---------------------------------------------------------------------------

#[test]
fn tracker_service_start_ingest_and_finish() {
    let manifest = manifest_for(
        CompletionFamily::ProtocolTurn,
        SelectorFamily::FinalMessage,
        CompletionSourceKind::ProtocolEventStream,
    );
    let resolver = resolver_for(manifest);
    let mut service = CompletionTrackerService::new(project_config(), resolver, CompletionRegistry);

    let job = JobRecord::new("job-1", "agent1", "claude");
    let view = service.start(&job, ts()).unwrap();
    assert_eq!(view.job_id, "job-1");
    assert!(!view.decision.terminal);

    service
        .ingest(
            &job.job_id,
            &item_with_text(CompletionItemKind::TurnBoundary, "reply", "all done"),
        )
        .unwrap();
    let view = service.current(&job.job_id).unwrap();
    assert!(view.decision.terminal);
    assert_eq!(view.decision.reply, "all done");

    service.finish(&job.job_id);
    assert!(service.current(&job.job_id).is_none());
}

#[test]
fn tracker_service_timeout_finalizes() {
    let manifest = manifest_for(
        CompletionFamily::TerminalTextQuiet,
        SelectorFamily::FinalMessage,
        CompletionSourceKind::TerminalText,
    );
    let resolver = resolver_for(manifest);
    let mut service = CompletionTrackerService::new(project_config(), resolver, CompletionRegistry)
        .with_request_timeout_s(1.0);

    let job = JobRecord::new("job-1", "agent1", "claude");
    service.start(&job, "2024-01-01T00:00:00Z").unwrap();
    service
        .ingest(
            &job.job_id,
            &item_with_text(CompletionItemKind::AssistantChunk, "text", "partial"),
        )
        .unwrap();

    let view = service.tick(&job.job_id, "2024-01-01T00:00:02Z").unwrap();
    assert!(view.decision.terminal);
    assert_eq!(view.decision.reason, Some("terminal_quiet".into()));
}

#[test]
fn tracker_resets_selector_on_session_rotate() {
    let manifest = manifest_for(
        CompletionFamily::ProtocolTurn,
        SelectorFamily::FinalMessage,
        CompletionSourceKind::ProtocolEventStream,
    );
    let resolver = resolver_for(manifest);
    let mut service = CompletionTrackerService::new(project_config(), resolver, CompletionRegistry);

    let job = JobRecord::new("job-1", "agent1", "claude");
    service.start(&job, ts()).unwrap();

    service
        .ingest(
            &job.job_id,
            &item_with_text(CompletionItemKind::TurnBoundary, "reply", "first"),
        )
        .unwrap();
    let view = service.current(&job.job_id).unwrap();
    assert_eq!(view.decision.reply, "first");

    service
        .ingest(
            &job.job_id,
            &CompletionItem {
                kind: CompletionItemKind::SessionRotate,
                timestamp: ts().to_string(),
                cursor: cursor(),
                provider: "claude".into(),
                agent_name: "agent1".into(),
                req_id: "job-1".into(),
                payload: Default::default(),
            },
        )
        .unwrap();

    let view_after_rotate = service.current(&job.job_id).unwrap();
    assert!(!view_after_rotate.decision.terminal);
    assert!(view_after_rotate.decision.reply.is_empty());
}

// ---------------------------------------------------------------------------
// Snapshot store tests
// ---------------------------------------------------------------------------

#[test]
fn snapshot_store_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let layout = ccb_storage::paths::PathLayout::new(tmp.path().to_str().unwrap());
    let store = CompletionSnapshotStore::new(layout, None);

    let snapshot = CompletionSnapshot::new(
        "job-1",
        "agent1",
        CompletionFamily::ProtocolTurn,
        CompletionState::default(),
        CompletionDecision::pending(None),
        ts(),
    )
    .unwrap();

    store.save(&snapshot).unwrap();
    let loaded = store.load("job-1").unwrap().unwrap();
    assert_eq!(loaded.job_id, snapshot.job_id);
    assert_eq!(loaded.agent_name, snapshot.agent_name);
}

// ---------------------------------------------------------------------------
// Orchestrator tests
// ---------------------------------------------------------------------------

struct VecSource {
    baseline: CompletionCursor,
    items: Vec<CompletionItem>,
    index: usize,
}

impl VecSource {
    fn new(items: Vec<CompletionItem>) -> Self {
        Self {
            baseline: cursor(),
            items,
            index: 0,
        }
    }
}

impl ccb_completion::CompletionSource for VecSource {
    fn capture_baseline(&self) -> CompletionCursor {
        self.baseline.clone()
    }

    fn poll(&mut self, _cursor: &CompletionCursor, _timeout_s: f64) -> Option<CompletionItem> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

#[test]
fn orchestrator_runs_to_completion() {
    let orchestrator = CompletionOrchestrator::default();
    let request_ctx = CompletionRequestContext::new("job-1", "agent1", "claude", 5.0).unwrap();
    let mut source = VecSource::new(vec![
        item_with_text(CompletionItemKind::AssistantChunk, "text", "partial"),
        item_with_text(CompletionItemKind::TurnBoundary, "reply", "final"),
    ]);
    let mut detector = ProtocolTurnDetector::default();
    let mut selector = FinalMessageSelector::default();

    let decision = orchestrator.run(&request_ctx, &mut source, &mut detector, &mut selector);
    assert!(decision.terminal);
    assert_eq!(decision.reply, "final");
}
