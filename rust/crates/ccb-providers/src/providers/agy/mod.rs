use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::protocol;
use regex::Regex;
use serde_json::Value;

use crate::execution::target::{
    backend_config_from_session_data, resolve_prompt_target_for_session, store_backend_config,
};
use crate::execution::{
    build_item, error_submission, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext,
    ProviderSubmission,
};

pub mod launcher;
pub mod native_log;

pub use launcher::{build_start_cmd as build_agy_start_cmd, AgyStartCommand};

pub const PROVIDER_NAME: &str = "agy";

const AGY_SESSION_FILENAME: &str = ".agy-session";
const AGY_SESSION_ID_ATTR: &str = "agy_session_id";
const AGY_SESSION_PATH_ATTR: &str = "agy_session_path";

const AGY_REQ_ID_PREFIX: &str = "CCB_REQ_ID:";
const AGY_DONE_PREFIX: &str = "CCB_DONE:";

const PANE_LINES_DEFAULT: i64 = 2000;
const QUIET_SECS: f64 = 4.0;
const MAX_WAIT_SECS: f64 = 300.0;
const MIN_OBSERVED_SECS: f64 = 2.0;
const ANCHOR_WAIT_SECS: f64 = 120.0;

const BANNER_KEYWORDS: &[&str] = &["CCB_REQ_ID:", "CCB_DONE:"];
const BANNER_INSTRUCTIONS: &[&str] = &[
    "IMPORTANT: when you finish",
    "IMPORTANT:",
    "on its own line as the final line",
    "no quoting, no code fence",
];

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the AGY provider manifest.
///
/// Mirrors Python `provider_backends.agy.manifest.build_manifest`.
pub fn manifest() -> ProviderManifest {
    let provider = PROVIDER_NAME.to_string();
    let mut profiles = HashMap::new();
    profiles.insert(
        RuntimeMode::PaneBacked,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "pane-backed".to_string(),
            poll_interval_ms: 500,
            timeout_ms: 300_000,
            ..Default::default()
        },
    );
    ProviderManifest::new(
        provider, true,  // supports_resume
        true,  // supports_permission_auto
        false, // supports_stream_watch
        true,  // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build the AGY provider backend registration.
///
/// Mirrors Python `provider_backends.agy.build_backend`.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(build_runtime_launcher()),
    }
}

/// Build the AGY session binding.
pub fn build_session_binding() -> ProviderSessionBinding {
    let mut binding = ProviderSessionBinding::new(PROVIDER_NAME);
    binding.session_id_attr = AGY_SESSION_ID_ATTR.to_string();
    binding.session_path_attr = AGY_SESSION_PATH_ATTR.to_string();
    binding
}

/// Build the AGY runtime launcher descriptor.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::SimpleTmux)
}

// ---------------------------------------------------------------------------
// Session model
// ---------------------------------------------------------------------------

/// A loaded AGY project session.
///
/// Mirrors Python `provider_backends.agy.session.AgyProjectSession`.
#[derive(Debug, Clone, Default)]
pub struct AgyProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl AgyProjectSession {
    pub fn agy_session_id(&self) -> String {
        self.data
            .get(AGY_SESSION_ID_ATTR)
            .or_else(|| self.data.get("agy_session_id"))
            .or_else(|| self.data.get("ccb_session_id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn agy_session_path(&self) -> String {
        self.data
            .get(AGY_SESSION_PATH_ATTR)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn pane_id(&self) -> String {
        self.data
            .get("pane_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }
}

/// Find an AGY project session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(AGY_SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load an AGY project session from disk.
pub fn load_project_session(work_dir: &Path, instance: Option<&str>) -> Option<AgyProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(AgyProjectSession { session_file, data })
}

// ---------------------------------------------------------------------------
// Execution adapter
// ---------------------------------------------------------------------------

/// AGY execution adapter.
///
/// Mirrors Python `provider_backends.agy.execution.AgyProviderAdapter` and the
/// surrounding `execution_runtime` modules.
pub struct AgyExecutionAdapter;

impl ExecutionAdapter for AgyExecutionAdapter {
    fn provider(&self) -> &str {
        PROVIDER_NAME
    }

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        start_active_submission(job, context, now)
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        poll_submission(submission, now)
    }

    fn resume(
        &self,
        _job: &JobRecord,
        _submission: &ProviderSubmission,
        _context: Option<&ProviderRuntimeContext>,
        _persisted_state: &crate::execution::PersistedExecutionState,
        _now: &str,
    ) -> Option<ProviderSubmission> {
        None
    }
}

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let provider = PROVIDER_NAME;
    let work_dir = resolve_work_dir(context);
    if work_dir.is_none() {
        return error_submission(
            job,
            provider,
            now,
            CompletionSourceKind::TerminalText,
            "runtime_unavailable",
            "work_dir_missing",
        );
    }
    let work_dir = work_dir.unwrap();

    let instance = job.agent_name.trim().to_lowercase();
    let instance_opt = if instance.is_empty() {
        None
    } else {
        Some(instance.as_str())
    };

    let session = load_project_session(&work_dir, instance_opt)
        .or_else(|| load_project_session(&work_dir, None));

    let session = match session {
        Some(s) => s,
        None => {
            return error_submission(
                job,
                provider,
                now,
                CompletionSourceKind::TerminalText,
                "runtime_unavailable",
                "agy_session_file_missing",
            );
        }
    };

    let pane_id = session.pane_id();
    if pane_id.is_empty() {
        return error_submission(
            job,
            provider,
            now,
            CompletionSourceKind::TerminalText,
            "pane_unavailable",
            "pane_id_missing_in_session",
        );
    }

    let req_id = request_anchor(&job.job_id);
    let prompt = wrap_agy_prompt(&job.request.body, &req_id);

    let backend_config = backend_config_from_session_data(&session.data);
    let target = resolve_prompt_target_for_session(&session.data);
    let mut send_error: Option<String> = None;
    if let Some(target) = target {
        if let Err(err) = target.send_text(&pane_id, &prompt) {
            send_error = Some(err);
        }
    }

    let agy_home = session
        .data
        .get("start_cmd")
        .and_then(Value::as_str)
        .and_then(native_log::agy_home_from_start_cmd)
        .or_else(|| {
            session
                .data
                .get("agy_home")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            session
                .data
                .get("runtime_dir")
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("home")
        });

    let mut diagnostics = serde_json::json!({
        "provider": provider,
        "mode": "native_transcript_log",
        "pane_id": pane_id,
        "req_id": req_id,
        "workspace_path": work_dir.to_string_lossy().to_string(),
    });
    if let Some(err) = &send_error {
        diagnostics
            .as_object_mut()
            .unwrap()
            .insert("send_error".to_string(), Value::String(err.clone()));
    }

    let mut runtime_state = HashMap::new();
    runtime_state.insert(
        "mode".to_string(),
        Value::String("native_transcript_log".to_string()),
    );
    runtime_state.insert("pane_id".to_string(), Value::String(pane_id));
    runtime_state.insert("req_id".to_string(), Value::String(req_id.clone()));
    runtime_state.insert("request_anchor".to_string(), Value::String(req_id));
    runtime_state.insert(
        "pane_lines".to_string(),
        Value::Number((PANE_LINES_DEFAULT as u64).into()),
    );
    runtime_state.insert("work_dir".to_string(), work_dir.to_value());
    runtime_state.insert(
        "runtime_dir".to_string(),
        session
            .data
            .get("runtime_dir")
            .cloned()
            .unwrap_or_else(|| work_dir.to_value()),
    );
    runtime_state.insert("agy_home".to_string(), agy_home.to_value());
    runtime_state.insert("started_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("last_hash".to_string(), Value::Null);
    runtime_state.insert("last_change_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(send_error.is_none()));
    runtime_state.insert(
        "send_error".to_string(),
        send_error.map(Value::String).unwrap_or(Value::Null),
    );
    runtime_state.insert("snapshot_errors".to_string(), Value::Number(0.into()));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("prompt_text".to_string(), Value::String(prompt));
    runtime_state.insert(
        "session_path".to_string(),
        Value::String(session.agy_session_path()),
    );
    runtime_state.insert("anchor_emitted".to_string(), Value::Bool(false));
    runtime_state.insert(
        "last_reply_signature".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "turn_boundary_ref".to_string(),
        Value::String(String::new()),
    );
    store_backend_config(&mut runtime_state, &backend_config);

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: provider.to_string(),
        accepted_at: now.to_string(),
        ready_at: now.to_string(),
        source_kind: CompletionSourceKind::SessionEventLog,
        reply: String::new(),
        status: CompletionStatus::Incomplete,
        reason: "in_progress".to_string(),
        confidence: CompletionConfidence::Observed,
        diagnostics: Some(diagnostics),
        runtime_state,
    }
}

trait PathToValue {
    fn to_value(&self) -> Value;
}

impl PathToValue for PathBuf {
    fn to_value(&self) -> Value {
        Value::String(self.to_string_lossy().to_string())
    }
}

impl PathToValue for Path {
    fn to_value(&self) -> Value {
        Value::String(self.to_string_lossy().to_string())
    }
}

fn poll_submission(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    if submission.is_terminal() {
        return None;
    }

    let mut state = submission.runtime_state.clone();

    let send_error = state
        .get("send_error")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    if let Some(err) = send_error {
        return Some(terminal_result(
            submission,
            &mut state,
            now,
            CompletionStatus::Failed,
            &format!("send_failed:{err}"),
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    let pane_id = state_str(&state, "pane_id");
    let req_id = state_str(&state, "req_id");
    if pane_id.is_empty() || req_id.is_empty() {
        return Some(terminal_result(
            submission,
            &mut state,
            now,
            CompletionStatus::Failed,
            "runtime_state_invalid",
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    // Test seam and legacy fallback: if the runtime state carries an explicit
    // pane snapshot, use the old pane-quiet extraction algorithm.
    if state.contains_key("pane_content") {
        return poll_pane_quiet(submission, now, &mut state, &pane_id, &req_id);
    }

    poll_native_transcript(submission, now, &mut state, &pane_id, &req_id)
}

fn poll_pane_quiet(
    submission: &ProviderSubmission,
    now: &str,
    state: &mut HashMap<String, Value>,
    pane_id: &str,
    req_id: &str,
) -> Option<ProviderPollResult> {
    let content = snapshot_text(state, pane_id);
    if content.is_empty() {
        let errors = state_int(state, "snapshot_errors", 0) + 1;
        state.insert("snapshot_errors".to_string(), Value::Number(errors.into()));
    }

    let current_hash = if content.is_empty() {
        state_str(state, "last_hash")
    } else {
        hash_text(&content)
    };
    let last_hash = state
        .get("last_hash")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let started_at = state_str(state, "started_at");
    let started_at = if started_at.is_empty() {
        submission.accepted_at.clone()
    } else {
        started_at
    };
    let mut last_change_at = state_str(state, "last_change_at");

    if !content.is_empty() && current_hash != last_hash {
        state.insert("last_hash".to_string(), Value::String(current_hash));
        state.insert("last_change_at".to_string(), Value::String(now.to_string()));
        last_change_at = now.to_string();
    }

    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    let next_seq = state_int(state, "next_seq", 1) + 1;
    state.insert("next_seq".to_string(), Value::Number(next_seq.into()));

    let quiet_secs = seconds_between(&last_change_at, now);
    let total_secs = seconds_between(&started_at, now);
    state.insert("quiet_secs".to_string(), f64_value(quiet_secs));
    state.insert("total_secs".to_string(), f64_value(total_secs));

    let (reply, done_seen) = extract_reply_for_req(&content, req_id);
    state.insert("done_seen".to_string(), Value::Bool(done_seen));
    state.insert("reply_chars".to_string(), Value::Number(reply.len().into()));

    let anchor_present = !content.is_empty() && pane_contains_req_anchor(&content, req_id);
    state.insert("anchor_present".to_string(), Value::Bool(anchor_present));

    if done_seen && !reply.is_empty() {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Completed,
            "pane_done_marker",
            &reply,
            CompletionConfidence::Observed,
            None,
        ));
    }

    if done_seen && reply.is_empty() {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Incomplete,
            "pane_done_empty_reply",
            "",
            CompletionConfidence::Observed,
            Some(empty_reply_diagnostics()),
        ));
    }

    if total_secs >= MAX_WAIT_SECS {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Failed,
            "pane_quiet_timeout",
            &reply,
            CompletionConfidence::Degraded,
            None,
        ));
    }

    if !reply.is_empty() && total_secs >= MIN_OBSERVED_SECS && quiet_secs >= QUIET_SECS {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Completed,
            "pane_text_quiet",
            &reply,
            CompletionConfidence::Degraded,
            None,
        ));
    }

    if !anchor_present && total_secs >= ANCHOR_WAIT_SECS {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Incomplete,
            "agy_input_unresponsive",
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    None
}

fn poll_native_transcript(
    submission: &ProviderSubmission,
    now: &str,
    state: &mut HashMap<String, Value>,
    pane_id: &str,
    req_id: &str,
) -> Option<ProviderPollResult> {
    // Ensure the prompt has been dispatched to the pane.
    if !state_bool(state, "prompt_sent") {
        if let Some(err) = dispatch_prompt(state, pane_id) {
            state.insert("send_error".to_string(), Value::String(err.clone()));
            return Some(terminal_result(
                submission,
                state,
                now,
                CompletionStatus::Failed,
                &format!("send_failed:{err}"),
                "",
                CompletionConfidence::Degraded,
                None,
            ));
        }
        state.insert("prompt_sent".to_string(), Value::Bool(true));
        state.insert("prompt_sent_at".to_string(), Value::String(now.to_string()));
        let updated = ProviderSubmission {
            runtime_state: state.clone(),
            ..submission.clone()
        };
        return Some(ProviderPollResult::new(updated, Vec::new(), None));
    }

    let work_dir = state_str(state, "work_dir");
    if work_dir.is_empty() {
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Failed,
            "runtime_state_invalid",
            "",
            CompletionConfidence::Degraded,
            None,
        ));
    }

    let started_at = state_str(state, "started_at");
    let started_at = if started_at.is_empty() {
        submission.accepted_at.clone()
    } else {
        started_at
    };

    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    let total_secs = seconds_between(&started_at, now);
    state.insert("total_secs".to_string(), f64_value(total_secs));

    let observation = native_log::observe_agy_transcript(
        Path::new(&work_dir),
        req_id,
        Some(&agy_home_candidates(state)),
    );

    if observation.is_none() && total_secs >= ANCHOR_WAIT_SECS {
        let mut extra = HashMap::new();
        extra.insert("anchor_seen".to_string(), Value::Bool(false));
        extra.insert(
            "diagnosis".to_string(),
            Value::String("AGY transcript did not record the submitted CCB_REQ_ID.".to_string()),
        );
        return Some(terminal_result(
            submission,
            state,
            now,
            CompletionStatus::Incomplete,
            "agy_native_anchor_missing",
            "",
            CompletionConfidence::Degraded,
            Some(extra),
        ));
    }

    let mut items = Vec::new();
    let mut dirty = false;

    if let Some(observation) = observation {
        let transcript_path = observation
            .transcript_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let prev_session_path = state_str(state, "session_path");
        if !transcript_path.is_empty() && transcript_path != prev_session_path {
            let mut rotate_payload = HashMap::new();
            rotate_payload.insert(
                "session_path".to_string(),
                Value::String(transcript_path.clone()),
            );
            if let Some(id) = &observation.conversation_id {
                rotate_payload.insert("provider_session_id".to_string(), Value::String(id.clone()));
            }
            items.push(build_item(
                submission,
                CompletionItemKind::SessionRotate,
                now,
                next_seq(state),
                rotate_payload,
            ));
            state.insert("session_path".to_string(), Value::String(transcript_path));
            state.insert("anchor_emitted".to_string(), Value::Bool(false));
            state.insert("reply_buffer".to_string(), Value::String(String::new()));
            state.insert(
                "last_reply_signature".to_string(),
                Value::String(String::new()),
            );
            state.insert(
                "turn_boundary_ref".to_string(),
                Value::String(String::new()),
            );
            dirty = true;
        }

        if observation.request_seen && !state_bool(state, "anchor_emitted") {
            let mut anchor_payload = HashMap::new();
            anchor_payload.insert("turn_id".to_string(), Value::String(req_id.to_string()));
            if let Some(path) = &observation.transcript_path {
                anchor_payload.insert(
                    "session_path".to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
            if let Some(id) = &observation.conversation_id {
                anchor_payload.insert("provider_session_id".to_string(), Value::String(id.clone()));
            }
            if let Some(ts) = &observation.native_started_at {
                anchor_payload.insert("native_started_at".to_string(), Value::String(ts.clone()));
            }
            items.push(build_item(
                submission,
                CompletionItemKind::AnchorSeen,
                now,
                next_seq(state),
                anchor_payload,
            ));
            state.insert("anchor_emitted".to_string(), Value::Bool(true));
            dirty = true;
        }

        let reply = observation.reply.clone();
        let reply_signature = hash_text(&reply);
        let prev_signature = state_str(state, "last_reply_signature");
        if !reply.is_empty() && reply_signature != prev_signature {
            state.insert("reply_buffer".to_string(), Value::String(reply.clone()));
            state.insert(
                "last_reply_signature".to_string(),
                Value::String(reply_signature),
            );
            let mut reply_payload = HashMap::new();
            reply_payload.insert("text".to_string(), Value::String(reply.clone()));
            reply_payload.insert("reply".to_string(), Value::String(reply.clone()));
            reply_payload.insert("final_answer".to_string(), Value::String(reply.clone()));
            reply_payload.insert("turn_id".to_string(), Value::String(req_id.to_string()));
            if let Some(path) = &observation.transcript_path {
                reply_payload.insert(
                    "session_path".to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
            if let Some(id) = &observation.conversation_id {
                reply_payload.insert("provider_session_id".to_string(), Value::String(id.clone()));
            }
            if let Some(r) = &observation.provider_turn_ref {
                reply_payload.insert("provider_turn_ref".to_string(), Value::String(r.clone()));
            }
            reply_payload.insert(
                "native_completed".to_string(),
                Value::Bool(observation.completed),
            );
            items.push(build_item(
                submission,
                CompletionItemKind::AssistantFinal,
                now,
                next_seq(state),
                reply_payload,
            ));
            dirty = true;
        }

        let boundary_ref = observation
            .provider_turn_ref
            .clone()
            .or(observation.conversation_id.clone())
            .or(observation
                .transcript_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|| req_id.to_string());
        let prev_boundary_ref = state_str(state, "turn_boundary_ref");
        if observation.completed && boundary_ref != prev_boundary_ref {
            let mut boundary_payload = HashMap::new();
            boundary_payload.insert(
                "reason".to_string(),
                Value::String("agy_transcript_response_done".to_string()),
            );
            boundary_payload.insert("last_agent_message".to_string(), Value::String(reply));
            boundary_payload.insert("turn_id".to_string(), Value::String(req_id.to_string()));
            if let Some(path) = &observation.transcript_path {
                boundary_payload.insert(
                    "session_path".to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                );
            }
            if let Some(id) = &observation.conversation_id {
                boundary_payload
                    .insert("provider_session_id".to_string(), Value::String(id.clone()));
            }
            if let Some(r) = &observation.provider_turn_ref {
                boundary_payload.insert("provider_turn_ref".to_string(), Value::String(r.clone()));
            }
            if let Some(ts) = &observation.native_completed_at {
                boundary_payload
                    .insert("native_completed_at".to_string(), Value::String(ts.clone()));
            }
            if let Some(status) = &observation.latest_status {
                boundary_payload.insert("latest_status".to_string(), Value::String(status.clone()));
            }
            items.push(build_item(
                submission,
                CompletionItemKind::TurnBoundary,
                now,
                next_seq(state),
                boundary_payload,
            ));
            state.insert("turn_boundary_ref".to_string(), Value::String(boundary_ref));
            dirty = true;
        }

        if total_secs >= MAX_WAIT_SECS && !observation.completed {
            return Some(terminal_result(
                submission,
                state,
                now,
                CompletionStatus::Failed,
                "agy_native_turn_timeout",
                &state_str(state, "reply_buffer"),
                CompletionConfidence::Degraded,
                None,
            ));
        }

        if observation.completed {
            let reply = state_str(state, "reply_buffer");
            return Some(terminal_result(
                submission,
                state,
                now,
                CompletionStatus::Completed,
                "agy_transcript_response_done",
                &reply,
                CompletionConfidence::Exact,
                None,
            ));
        }
    }

    let updated = ProviderSubmission {
        reply: state_str(state, "reply_buffer"),
        runtime_state: state.clone(),
        ..submission.clone()
    };
    if dirty {
        return Some(ProviderPollResult::new(updated, items, None));
    }
    None
}

fn dispatch_prompt(state: &HashMap<String, Value>, pane_id: &str) -> Option<String> {
    let prompt = state_str(state, "prompt_text");
    if prompt.is_empty() {
        return Some("prompt_text_missing".to_string());
    }
    let target = crate::execution::target::resolve_prompt_target(state)?;
    target.send_text(pane_id, &prompt).err()
}

fn agy_home_candidates(state: &HashMap<String, Value>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["agy_home", "home", "runtime_home"] {
        let value = state_str(state, key).trim().to_string();
        if !value.is_empty() {
            candidates.push(PathBuf::from(value));
        }
    }
    let runtime_dir = state_str(state, "runtime_dir").trim().to_string();
    if !runtime_dir.is_empty() {
        candidates.push(PathBuf::from(runtime_dir).join("home"));
    }
    candidates
}

fn next_seq(state: &mut HashMap<String, Value>) -> u64 {
    let seq = state_int(state, "next_seq", 1) as u64;
    state.insert("next_seq".to_string(), Value::Number((seq + 1).into()));
    seq
}

#[allow(clippy::too_many_arguments)]
fn terminal_result(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    now: &str,
    status: CompletionStatus,
    reason: &str,
    reply: &str,
    confidence: CompletionConfidence,
    diagnostics_extra: Option<HashMap<String, Value>>,
) -> ProviderPollResult {
    let reply = reply.to_string();
    let next_seq = state_int(state, "next_seq", 1) as u64;
    let req_id = state_str(state, "req_id");

    let updated = ProviderSubmission {
        runtime_state: state.clone(),
        status,
        reason: reason.to_string(),
        reply: reply.clone(),
        confidence,
        ..submission.clone()
    };

    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(next_seq),
        updated_at: Some(now.to_string()),
        ..Default::default()
    };

    let mut diagnostics = HashMap::new();
    diagnostics.insert("mode".to_string(), Value::String("pane_quiet".to_string()));
    diagnostics.insert(
        "quiet_secs".to_string(),
        f64_value(state_f64(state, "quiet_secs")),
    );
    diagnostics.insert(
        "total_secs".to_string(),
        f64_value(state_f64(state, "total_secs")),
    );
    diagnostics.insert(
        "done_seen".to_string(),
        Value::Bool(
            state
                .get("done_seen")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    diagnostics.insert(
        "anchor_present".to_string(),
        Value::Bool(
            state
                .get("anchor_present")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    diagnostics.insert(
        "snapshot_errors".to_string(),
        Value::Number(state_int(state, "snapshot_errors", 0).into()),
    );
    diagnostics.insert("reply_chars".to_string(), Value::Number(reply.len().into()));
    if let Some(extra) = diagnostics_extra {
        diagnostics.extend(extra);
    }

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: reply.clone(),
        anchor_seen: state_bool(state, "done_seen") || !reply.is_empty(),
        reply_started: !reply.is_empty(),
        reply_stable: !reply.is_empty() && status == CompletionStatus::Completed,
        provider_turn_ref: Some(if req_id.is_empty() {
            submission.job_id.clone()
        } else {
            req_id
        }),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics: diagnostics.into_iter().collect(),
    };

    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

fn empty_reply_diagnostics() -> HashMap<String, Value> {
    let diagnosis = "Provider pane showed the requested done marker without assistant reply \
                     text; inspect the pane transcript, prompt echo boundaries, and \
                     authentication/API output.";
    let mut map = HashMap::new();
    map.insert("empty_reply".to_string(), Value::Bool(true));
    map.insert(
        "error_type".to_string(),
        Value::String("empty_provider_reply".to_string()),
    );
    map.insert("message".to_string(), Value::String(diagnosis.to_string()));
    map.insert(
        "diagnosis".to_string(),
        Value::String(diagnosis.to_string()),
    );
    map
}

// ---------------------------------------------------------------------------
// Protocol helpers
// ---------------------------------------------------------------------------

/// Generate a short request ID from a job ID.
pub fn make_req_id(job_id: &str) -> String {
    protocol::make_req_id(job_id)
}

/// Build the request anchor marker for a job.
///
/// Mirrors Python `provider_core.protocol.request_anchor_for_job` and is used by
/// AGY as the identifier embedded in `CCB_REQ_ID:` / `CCB_DONE:` markers.
pub fn request_anchor(job_id: &str) -> String {
    protocol::request_anchor_for_job(job_id)
}

/// Wrap an AGY prompt with explicit request/done markers.
///
/// Mirrors Python `provider_backends.agy.protocol.wrap_agy_prompt`.
pub fn wrap_agy_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!(
        "{AGY_REQ_ID_PREFIX} {req_id}\n\n{rendered}\n\n\
         IMPORTANT: when you finish answering, write this exact line on its \
         own line as the final line of your reply (no quoting, no code fence):\n\
         {AGY_DONE_PREFIX} {req_id}\n"
    )
}

/// Check whether a pane snapshot contains the request anchor for `req_id`.
pub fn pane_contains_req_anchor(text: &str, req_id: &str) -> bool {
    if text.is_empty() || req_id.is_empty() {
        return false;
    }
    req_anchor_re(req_id).is_match(text)
}

/// Extract the reply window for `req_id` from a pane snapshot.
///
/// Returns `(reply, done_seen)`. The algorithm mirrors Python
/// `provider_backends.agy.protocol.extract_reply_for_req`:
/// the last `CCB_REQ_ID:` anchor is located, and the last two
/// `CCB_DONE:` occurrences after it are used to separate the echoed
/// prompt tail from the assistant reply.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> (String, bool) {
    if text.is_empty() || req_id.is_empty() {
        return (String::new(), false);
    }

    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    let anchor_re = req_anchor_re(req_id);
    let anchors: Vec<_> = anchor_re.find_iter(&text).collect();
    if anchors.is_empty() {
        return (String::new(), false);
    }

    let after_anchor = &text[anchors.last().unwrap().end()..];
    let done_re = done_anywhere_re(req_id);
    let dones: Vec<_> = done_re.find_iter(after_anchor).collect();
    if dones.len() < 2 {
        return (String::new(), false);
    }

    let echo_done_start = dones[dones.len() - 2].start();
    let model_done_start = dones[dones.len() - 1].start();
    let echo_line_end = line_end(after_anchor, echo_done_start);
    let model_line_start = line_start(after_anchor, model_done_start);

    let reply_start = if echo_line_end < after_anchor.len() {
        echo_line_end + 1
    } else {
        echo_line_end
    };
    let body = &after_anchor[reply_start..model_line_start];

    let cleaned = clean_body(body, req_id);
    if contains_banner_fragment(&cleaned) {
        return (String::new(), false);
    }

    (cleaned, true)
}

fn req_anchor_re(req_id: &str) -> Regex {
    let prefix = regex::escape(AGY_REQ_ID_PREFIX);
    let id = regex::escape(req_id);
    Regex::new(&format!(r"{prefix}\s*{id}")).unwrap()
}

fn done_anywhere_re(req_id: &str) -> Regex {
    let prefix = regex::escape(AGY_DONE_PREFIX);
    let id = regex::escape(req_id);
    Regex::new(&format!(r"{prefix}\s*{id}")).unwrap()
}

fn line_start(text: &str, pos: usize) -> usize {
    text[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0)
}

fn line_end(text: &str, pos: usize) -> usize {
    text[pos..]
        .find('\n')
        .map(|p| pos + p)
        .unwrap_or(text.len())
}

fn clean_body(body: &str, req_id: &str) -> String {
    let text = body.replace("\r\n", "\n").replace('\r', "\n");
    let text = strip_done_text_for_req(&text, req_id);
    let text = strip_any_done_lines(&text);

    let line_prefix_re = Regex::new(r"^[\s>$#❯]+").unwrap();
    let mut cleaned_lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let stripped = line_prefix_re.replace(raw, "").trim_end().to_string();
        if is_banner_line(&stripped) {
            continue;
        }
        cleaned_lines.push(stripped);
    }

    while cleaned_lines
        .first()
        .map(|s| s.trim().is_empty())
        .unwrap_or(false)
    {
        cleaned_lines.remove(0);
    }
    while cleaned_lines
        .last()
        .map(|s| s.trim().is_empty())
        .unwrap_or(false)
    {
        cleaned_lines.pop();
    }

    cleaned_lines.join("\n").trim().to_string()
}

fn strip_done_text_for_req(text: &str, req_id: &str) -> String {
    let re = Regex::new(&format!(
        r"(?m)^\s*{}\s*{}\s*$",
        regex::escape(AGY_DONE_PREFIX),
        regex::escape(req_id)
    ))
    .unwrap();
    re.replace_all(text, "").to_string()
}

fn strip_any_done_lines(text: &str) -> String {
    let re = Regex::new(r"(?m)^\s*CCB_DONE:\s*\S+\s*$").unwrap();
    re.replace_all(text, "").to_string()
}

fn contains_banner_fragment(text: &str) -> bool {
    for marker in BANNER_KEYWORDS.iter().chain(BANNER_INSTRUCTIONS.iter()) {
        if text.contains(marker) {
            return true;
        }
    }
    false
}

fn is_banner_line(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() {
        return false;
    }
    for marker in BANNER_KEYWORDS.iter().chain(BANNER_INSTRUCTIONS.iter()) {
        if text.contains(marker) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Pane snapshot helpers
// ---------------------------------------------------------------------------

/// Capture the current text of a tmux pane, stripping ANSI escape sequences.
///
/// If `pane_id` does not look like a tmux target, returns `None` rather than
/// invoking tmux.
fn capture_pane_content(pane_id: &str, lines: usize) -> Option<String> {
    if !ccb_terminal::tmux::looks_like_tmux_target(pane_id) {
        return None;
    }
    let output = std::process::Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            pane_id,
            "-p",
            "-S",
            &format!("-{lines}"),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    Some(strip_ansi(&text))
}

fn snapshot_text(state: &HashMap<String, Value>, pane_id: &str) -> String {
    if let Some(override_content) = state.get("pane_content").and_then(Value::as_str) {
        return override_content.to_string();
    }
    let lines = state_int(state, "pane_lines", PANE_LINES_DEFAULT);
    let lines = usize::try_from(lines).unwrap_or(PANE_LINES_DEFAULT as usize);
    capture_pane_content(pane_id, lines).unwrap_or_default()
}

fn strip_ansi(text: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").unwrap();
    re.replace_all(text, "").to_string()
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

fn resolve_work_dir(context: Option<&ProviderRuntimeContext>) -> Option<PathBuf> {
    let path = context?.workspace_path.as_deref()?;
    if path.trim().is_empty() {
        return None;
    }
    let expanded = if let Some(rest) = path.strip_prefix("~") {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(rest.trim_start_matches('/')))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    };
    Some(expanded)
}

fn hash_text(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_now(now: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    if now.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(now).ok()
}

fn seconds_between(start: &str, end: &str) -> f64 {
    match (parse_now(start), parse_now(end)) {
        (Some(start_dt), Some(end_dt)) => {
            ((end_dt - start_dt).num_milliseconds() as f64 / 1000.0).max(0.0)
        }
        _ => 0.0,
    }
}

fn state_int(state: &HashMap<String, Value>, key: &str, default: i64) -> i64 {
    state.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn state_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn state_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn state_f64(state: &HashMap<String, Value>, key: &str) -> f64 {
    state
        .get(key)
        .and_then(|v| v.as_f64())
        .or_else(|| state.get(key).and_then(Value::as_i64).map(|i| i as f64))
        .unwrap_or(0.0)
}

fn f64_value(value: f64) -> Value {
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or_else(|| Value::Number(0.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_agy_prompt_format() {
        let wrapped = wrap_agy_prompt("hello", "<<BEGIN:req-12345678>>");
        assert!(wrapped.contains("CCB_REQ_ID: <<BEGIN:req-12345678>>"));
        assert!(wrapped.contains("hello"));
        assert!(wrapped.contains("CCB_DONE: <<BEGIN:req-12345678>>"));
        assert!(wrapped.ends_with('\n'));
    }

    #[test]
    fn test_extract_reply_with_echo_and_model_done() {
        let req_id = "<<BEGIN:req-12345678>>";
        let text =
            format!("CCB_REQ_ID: {req_id}\nhello\nCCB_DONE: {req_id}\nworld\nCCB_DONE: {req_id}");
        let (reply, done) = extract_reply_for_req(&text, req_id);
        assert!(done);
        assert_eq!(reply, "world");
    }

    #[test]
    fn test_extract_reply_not_done_with_one_done() {
        let req_id = "<<BEGIN:req-12345678>>";
        let text = format!("CCB_REQ_ID: {req_id}\nhello\nCCB_DONE: {req_id}");
        let (reply, done) = extract_reply_for_req(&text, req_id);
        assert!(!done);
        assert!(reply.is_empty());
    }

    #[test]
    fn test_request_anchor_deterministic() {
        let a = request_anchor("job-123");
        let b = request_anchor("job-123");
        assert_eq!(a, b);
        assert!(a.starts_with("<<BEGIN:"));
        assert!(a.ends_with(">>"));
    }
}
