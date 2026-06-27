use std::collections::HashMap;
use std::path::PathBuf;

use ccbr_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::contracts::ProviderBackend;
use ccbr_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccbr_provider_core::protocol;
use serde_json::Value;

use crate::execution::{
    backend_config_from_session_data, build_item, error_submission, resolve_prompt_target,
    resolve_prompt_target_for_session, store_backend_config, ExecutionAdapter, PromptTarget,
    ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};
use crate::kimi::{
    build_runtime_launcher, build_session_binding, load_project_session, observe_kimi_turn,
    KimiTurnObservation,
};
use crate::native_cli_support::wrap_native_prompt;

pub const PROVIDER_NAME: &str = "kimi";

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;
const MAX_WAIT_SECS: f64 = 300.0;
const ANCHOR_WAIT_SECS: f64 = 120.0;
const READY_WAIT_SECS: f64 = 60.0;
const NEW_BOX_READY_STABLE_SECS: f64 = 3.0;
const PANE_LINES_DEFAULT: usize = 2000;

/// Build the Kimi provider manifest.
pub fn manifest() -> ProviderManifest {
    let provider = PROVIDER_NAME.to_string();
    let mut profiles = HashMap::new();
    profiles.insert(
        RuntimeMode::PaneBacked,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "pane-backed".to_string(),
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            ..Default::default()
        },
    );
    ProviderManifest::new(
        provider, false, // supports_resume
        false, // supports_permission_auto
        false, // supports_stream_watch
        false, // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build the full Kimi provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        // The execution adapter is registered with the ccbr-providers execution
        // registry rather than the ccbr-provider-core backend slot because the
        // two crates currently define distinct ExecutionAdapter traits.
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(build_runtime_launcher()),
    }
}

/// Kimi provider execution adapter.
pub struct KimiExecutionAdapter;

impl ExecutionAdapter for KimiExecutionAdapter {
    fn provider(&self) -> &str {
        PROVIDER_NAME
    }

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        start_native_submission(job, context, now)
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        poll_submission(submission, now)
    }

    fn export_runtime_state(
        &self,
        submission: &ProviderSubmission,
    ) -> Option<HashMap<String, Value>> {
        Some(submission.runtime_state.clone())
    }
}

fn start_native_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let work_dir = match resolve_work_dir(job, context) {
        Some(p) => p,
        None => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::SessionEventLog,
                "runtime_unavailable",
                "work_dir_missing",
            );
        }
    };

    if work_dir.as_os_str().is_empty() || !work_dir.exists() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::SessionEventLog,
            "runtime_unavailable",
            "work_dir_missing",
        );
    }

    let instance = job.agent_name.trim().to_lowercase();
    let instance = if instance.is_empty() {
        None
    } else {
        Some(instance.as_str())
    };
    let session = match load_project_session(&work_dir, instance) {
        Some(s) => s,
        None => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::SessionEventLog,
                "runtime_unavailable",
                "kimi_session_file_missing",
            );
        }
    };

    let pane_id = session.pane_id().unwrap_or("").to_string();
    if pane_id.is_empty() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::SessionEventLog,
            "pane_unavailable",
            "pane_id_missing_in_session",
        );
    }

    let backend_config = backend_config_from_session_data(&session.data);
    let target = match resolve_prompt_target_for_session(&session.data) {
        Some(t) => t,
        None => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::SessionEventLog,
                "backend_unavailable",
                "terminal_backend_unavailable",
            );
        }
    };

    let req_id = protocol::request_anchor_for_job(&job.job_id);
    let prompt = wrap_native_prompt(&job.request.body, &req_id);

    let mut initial_content = target
        .get_pane_content(&pane_id, PANE_LINES_DEFAULT)
        .unwrap_or_default();
    let mut new_box_prompt = pane_has_new_box_prompt(&initial_content);
    let mut new_box_stabilized = false;
    if new_box_prompt && !pane_has_legacy_prompt(&initial_content) {
        let stable_secs = new_box_ready_stable_secs();
        if stable_secs > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(stable_secs));
            initial_content = target
                .get_pane_content(&pane_id, PANE_LINES_DEFAULT)
                .unwrap_or_default();
            new_box_prompt = pane_has_new_box_prompt(&initial_content);
            new_box_stabilized = new_box_prompt;
        }
    }
    let prompt_deferred_until_ready = !pane_ready_for_input(&initial_content)
        || new_box_prompt && !pane_has_legacy_prompt(&initial_content) && !new_box_stabilized;

    let mut send_error: Option<String> = None;
    let mut prompt_sent = false;
    if !prompt_deferred_until_ready {
        send_error = send_prompt(&*target, &pane_id, &prompt);
        prompt_sent = send_error.is_none();
    }

    let mut diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "native_turn_log",
        "pane_id": pane_id,
        "req_id": req_id,
        "workspace_path": work_dir.to_string_lossy().to_string(),
    });
    if let Some(err) = &send_error {
        diagnostics["send_error"] = Value::String(err.clone());
    }
    if prompt_deferred_until_ready {
        diagnostics["prompt_deferred_until_ready"] = Value::Bool(true);
    }

    let mut runtime_state = HashMap::new();
    runtime_state.insert(
        "mode".to_string(),
        Value::String("native_turn_log".to_string()),
    );
    runtime_state.insert(
        "provider".to_string(),
        Value::String(PROVIDER_NAME.to_string()),
    );
    store_backend_config(&mut runtime_state, &backend_config);
    runtime_state.insert("pane_id".to_string(), Value::String(pane_id));
    runtime_state.insert("request_anchor".to_string(), Value::String(req_id.clone()));
    runtime_state.insert("req_id".to_string(), Value::String(req_id.clone()));
    runtime_state.insert(
        "work_dir".to_string(),
        Value::String(work_dir.to_string_lossy().to_string()),
    );
    runtime_state.insert("started_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(prompt_sent));
    runtime_state.insert("pending_prompt".to_string(), Value::String(prompt));
    runtime_state.insert(
        "prompt_deferred_until_ready".to_string(),
        Value::Bool(prompt_deferred_until_ready),
    );
    if new_box_prompt {
        runtime_state.insert(
            "ready_candidate_seen_at".to_string(),
            Value::String(now.to_string()),
        );
    }
    if let Some(err) = send_error {
        runtime_state.insert("send_error".to_string(), Value::String(err));
    }
    runtime_state.insert("snapshot_errors".to_string(), Value::Number(0.into()));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_emitted".to_string(), Value::Bool(false));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert(
        "last_reply_signature".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "turn_boundary_ref".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert("session_path".to_string(), Value::String(String::new()));

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: PROVIDER_NAME.to_string(),
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

fn poll_submission(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    if submission.is_terminal() {
        return None;
    }

    let mut state = submission.runtime_state.clone();

    let send_error = runtime_str(&state, "send_error");
    if !send_error.is_empty() {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            &format!("send_failed:{send_error}"),
            "",
            CompletionConfidence::Degraded,
            now,
        ));
    }

    let pane_id = runtime_str(&state, "pane_id");
    let req_id = {
        let anchor = runtime_str(&state, "request_anchor");
        if !anchor.is_empty() {
            anchor
        } else {
            let fallback = runtime_str(&state, "req_id");
            if !fallback.is_empty() {
                fallback
            } else {
                submission.job_id.clone()
            }
        }
    };
    let work_dir = runtime_str(&state, "work_dir");
    if pane_id.is_empty() || req_id.is_empty() || work_dir.is_empty() {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            "runtime_state_invalid",
            "",
            CompletionConfidence::Degraded,
            now,
        ));
    }

    let target = resolve_prompt_target(&state)?;

    if !runtime_bool(&state, "prompt_sent") {
        return poll_deferred_prompt(submission, &mut state, now, &*target, &pane_id);
    }

    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    let started_at = runtime_str(&state, "started_at");
    let started_at = if started_at.is_empty() {
        submission.accepted_at.clone()
    } else {
        started_at
    };
    let total_secs = seconds_between(&started_at, now);
    state.insert(
        "total_secs".to_string(),
        Value::Number((total_secs as u64).into()),
    );

    let work_dir_path = PathBuf::from(work_dir);
    let mut observation = observe_kimi_turn(&work_dir_path, &req_id, None);
    let pane_observation = observe_kimi_pane_turn(
        &*target,
        &pane_id,
        &req_id,
        &runtime_str(&state, "pending_prompt"),
    );
    if let Some(pane_observation) = pane_observation {
        let use_pane = observation
            .as_ref()
            .map(|native| pane_observation.completed && !native.completed)
            .unwrap_or(true);
        if use_pane {
            observation = Some(pane_observation);
            state.insert("pane_fallback_observed".to_string(), Value::Bool(true));
        }
    }

    if observation.is_none() {
        if total_secs >= ANCHOR_WAIT_SECS {
            return Some(terminal_result(
                submission,
                &mut state,
                CompletionStatus::Incomplete,
                "kimi_native_anchor_missing",
                "",
                CompletionConfidence::Degraded,
                now,
            ));
        }
        return None;
    }

    let observation = observation.unwrap();
    let mut items = Vec::new();

    let session_path = observation
        .session_path
        .as_deref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let session_path_opt = if session_path.is_empty() {
        None
    } else {
        Some(session_path.clone())
    };

    if session_path != runtime_str(&state, "session_path") {
        let mut payload = HashMap::new();
        payload.insert(
            "session_path".to_string(),
            Value::String(session_path.clone()),
        );
        payload.insert(
            "provider_session_id".to_string(),
            observation
                .session_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        let mut item = build_item(
            submission,
            CompletionItemKind::SessionRotate,
            now,
            next_seq(&mut state),
            payload,
        );
        item.cursor.session_path = session_path_opt.clone();
        items.push(item);
        state.insert(
            "session_path".to_string(),
            Value::String(session_path.clone()),
        );
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
    }

    if observation.request_seen && !runtime_bool(&state, "anchor_emitted") {
        let mut payload = HashMap::new();
        payload.insert("turn_id".to_string(), Value::String(req_id.clone()));
        payload.insert(
            "session_path".to_string(),
            session_path_opt
                .as_ref()
                .map(|s| Value::String(s.clone()))
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_session_id".to_string(),
            observation
                .session_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "native_started_at".to_string(),
            observation
                .native_started_at
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        let mut item = build_item(
            submission,
            CompletionItemKind::AnchorSeen,
            now,
            next_seq(&mut state),
            payload,
        );
        item.cursor.session_path = session_path_opt.clone();
        items.push(item);
        state.insert("anchor_emitted".to_string(), Value::Bool(true));
    }

    let reply = observation.reply.clone();
    if observation.completed && reply.is_empty() {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Incomplete,
            "kimi_native_empty_reply",
            "",
            CompletionConfidence::Observed,
            now,
        ));
    }

    let reply_signature = hash_text(&reply);
    if !reply.is_empty() && reply_signature != runtime_str(&state, "last_reply_signature") {
        state.insert("reply_buffer".to_string(), Value::String(reply.clone()));
        state.insert(
            "last_reply_signature".to_string(),
            Value::String(reply_signature),
        );
        let mut payload = HashMap::new();
        payload.insert("text".to_string(), Value::String(reply.clone()));
        payload.insert("reply".to_string(), Value::String(reply.clone()));
        payload.insert("final_answer".to_string(), Value::String(reply.clone()));
        payload.insert("turn_id".to_string(), Value::String(req_id.clone()));
        payload.insert(
            "session_path".to_string(),
            session_path_opt
                .as_ref()
                .map(|s| Value::String(s.clone()))
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_session_id".to_string(),
            observation
                .session_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_turn_ref".to_string(),
            observation
                .provider_turn_ref
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "native_completed".to_string(),
            Value::Bool(observation.completed),
        );
        let mut item = build_item(
            submission,
            CompletionItemKind::AssistantFinal,
            now,
            next_seq(&mut state),
            payload,
        );
        item.cursor.session_path = session_path_opt.clone();
        items.push(item);
    }

    let boundary_ref = observation
        .provider_turn_ref
        .clone()
        .or(observation.session_id.clone())
        .or(session_path_opt.clone())
        .unwrap_or_else(|| req_id.clone());
    if observation.completed && boundary_ref != runtime_str(&state, "turn_boundary_ref") {
        let mut payload = HashMap::new();
        payload.insert(
            "reason".to_string(),
            Value::String("kimi_turn_end".to_string()),
        );
        payload.insert(
            "last_agent_message".to_string(),
            Value::String(reply.clone()),
        );
        payload.insert("turn_id".to_string(), Value::String(req_id.clone()));
        payload.insert(
            "session_path".to_string(),
            session_path_opt
                .as_ref()
                .map(|s| Value::String(s.clone()))
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_session_id".to_string(),
            observation
                .session_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_turn_ref".to_string(),
            observation
                .provider_turn_ref
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "native_completed_at".to_string(),
            observation
                .native_completed_at
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        let mut boundary_item = build_item(
            submission,
            CompletionItemKind::TurnBoundary,
            now,
            next_seq(&mut state),
            payload,
        );
        boundary_item.cursor.session_path = session_path_opt.clone();
        items.push(boundary_item.clone());
        state.insert("turn_boundary_ref".to_string(), Value::String(boundary_ref));

        let mut updated = submission.clone();
        updated.reply = reply.clone();
        updated.runtime_state = state;

        let cursor = CompletionCursor {
            source_kind: submission.source_kind,
            event_seq: Some(boundary_item.cursor.event_seq.unwrap_or(1)),
            updated_at: Some(now.to_string()),
            session_path: session_path_opt,
            ..Default::default()
        };
        let decision = CompletionDecision {
            terminal: true,
            status: CompletionStatus::Completed,
            reason: Some("kimi_turn_end".to_string()),
            confidence: Some(CompletionConfidence::Observed),
            reply: reply.clone(),
            anchor_seen: true,
            reply_started: !reply.is_empty(),
            reply_stable: !reply.is_empty(),
            provider_turn_ref: Some(req_id),
            source_cursor: Some(cursor),
            finished_at: Some(now.to_string()),
            diagnostics: Default::default(),
        };
        return Some(ProviderPollResult::new(updated, items, Some(decision)));
    }

    if total_secs >= MAX_WAIT_SECS && !observation.completed {
        let reply_buffer = runtime_str(&state, "reply_buffer");
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            "kimi_native_turn_timeout",
            &reply_buffer,
            CompletionConfidence::Degraded,
            now,
        ));
    }

    if items.is_empty() {
        return None;
    }

    let mut updated = submission.clone();
    updated.reply = runtime_str(&state, "reply_buffer");
    updated.runtime_state = state;
    Some(ProviderPollResult::new(updated, items, None))
}

fn poll_deferred_prompt(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    now: &str,
    target: &dyn PromptTarget,
    pane_id: &str,
) -> Option<ProviderPollResult> {
    let started_at = runtime_str(state, "started_at");
    let started_at = if started_at.is_empty() {
        submission.accepted_at.clone()
    } else {
        started_at
    };
    let ready_wait_secs = seconds_between(&started_at, now);
    state.insert(
        "ready_wait_secs".to_string(),
        Value::Number((ready_wait_secs as u64).into()),
    );

    let content = target
        .get_pane_content(pane_id, PANE_LINES_DEFAULT)
        .unwrap_or_default();
    if pane_ready_for_input(&content) {
        if pane_has_new_box_prompt(&content) && !pane_has_legacy_prompt(&content) {
            let seen_at = runtime_str(state, "ready_candidate_seen_at");
            if seen_at.is_empty() {
                state.insert(
                    "ready_candidate_seen_at".to_string(),
                    Value::String(now.to_string()),
                );
                state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
                next_seq(state);
                let mut updated = submission.clone();
                updated.runtime_state = state.clone();
                return Some(ProviderPollResult::new(updated, vec![], None));
            }
            if seconds_between(&seen_at, now) < new_box_ready_stable_secs() {
                state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
                next_seq(state);
                let mut updated = submission.clone();
                updated.runtime_state = state.clone();
                return Some(ProviderPollResult::new(updated, vec![], None));
            }
        }
        let pending_prompt = runtime_str(state, "pending_prompt");
        if pending_prompt.is_empty() {
            return Some(terminal_result(
                submission,
                state,
                CompletionStatus::Failed,
                "runtime_state_invalid",
                "",
                CompletionConfidence::Degraded,
                now,
            ));
        }
        let send_error = send_prompt(target, pane_id, &pending_prompt);
        if let Some(err) = send_error {
            state.insert("send_error".to_string(), Value::String(err));
            return Some(terminal_result(
                submission,
                state,
                CompletionStatus::Failed,
                &format!("send_failed:{}", runtime_str(state, "send_error")),
                "",
                CompletionConfidence::Degraded,
                now,
            ));
        }
        state.insert("prompt_sent".to_string(), Value::Bool(true));
        state.insert("prompt_sent_at".to_string(), Value::String(now.to_string()));
        state.insert(
            "prompt_deferred_until_ready".to_string(),
            Value::Bool(false),
        );
        state.insert("started_at".to_string(), Value::String(now.to_string()));
        state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
        next_seq(state);
        let mut updated = submission.clone();
        updated.runtime_state = state.clone();
        return Some(ProviderPollResult::new(updated, vec![], None));
    }

    if ready_wait_secs >= READY_WAIT_SECS {
        return Some(terminal_result(
            submission,
            state,
            CompletionStatus::Incomplete,
            "kimi_input_not_ready",
            "",
            CompletionConfidence::Degraded,
            now,
        ));
    }

    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    next_seq(state);
    let mut updated = submission.clone();
    updated.runtime_state = state.clone();
    Some(ProviderPollResult::new(updated, vec![], None))
}

fn terminal_result(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    status: CompletionStatus,
    reason: &str,
    reply: &str,
    confidence: CompletionConfidence,
    now: &str,
) -> ProviderPollResult {
    let reply = reply.to_string();
    let mut updated = submission.clone();
    updated.status = status;
    updated.reason = reason.to_string();
    updated.reply = reply.clone();
    updated.confidence = confidence;
    updated.runtime_state = state.clone();

    let seq = runtime_u64(state, "next_seq").max(1);
    let total_secs = runtime_str(state, "total_secs")
        .parse::<f64>()
        .unwrap_or_else(|_| {
            runtime_str(state, "ready_wait_secs")
                .parse::<f64>()
                .unwrap_or(0.0)
        });
    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(seq),
        updated_at: Some(now.to_string()),
        ..Default::default()
    };
    let mut diagnostics: serde_json::Map<String, Value> = serde_json::json!({
        "mode": "native_turn_log",
        "total_secs": total_secs,
        "anchor_seen": runtime_bool(state, "anchor_emitted"),
        "reply_chars": reply.len(),
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    if reason == "kimi_input_not_ready" {
        diagnostics.insert("input_not_ready".to_string(), Value::Bool(true));
        diagnostics.insert(
            "ready_wait_secs".to_string(),
            state
                .get("ready_wait_secs")
                .cloned()
                .unwrap_or(Value::Number(0.into())),
        );
    }
    let request_anchor = runtime_str(state, "request_anchor");
    let provider_turn_ref = if request_anchor.is_empty() {
        submission.job_id.clone()
    } else {
        request_anchor
    };
    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: reply.clone(),
        anchor_seen: runtime_bool(state, "anchor_emitted"),
        reply_started: !reply.is_empty(),
        reply_stable: !reply.is_empty() && status == CompletionStatus::Completed,
        provider_turn_ref: Some(provider_turn_ref),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

fn send_prompt(target: &dyn PromptTarget, pane_id: &str, prompt: &str) -> Option<String> {
    target.send_text(pane_id, prompt).err()
}

fn pane_ready_for_input(content: &str) -> bool {
    pane_has_legacy_prompt(content) || pane_has_new_box_prompt(content)
}

fn pane_has_legacy_prompt(content: &str) -> bool {
    content.contains("── input") && content.contains("agent (")
}

fn pane_has_new_box_prompt(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('│') && trimmed.contains('>') && trimmed.ends_with('│')
    })
}

fn observe_kimi_pane_turn(
    target: &dyn PromptTarget,
    pane_id: &str,
    req_id: &str,
    pending_prompt: &str,
) -> Option<KimiTurnObservation> {
    let content = target
        .get_pane_content(pane_id, PANE_LINES_DEFAULT)
        .unwrap_or_default();
    if content.trim().is_empty() {
        return None;
    }
    let anchor = if !req_id.is_empty() && content.contains(req_id) {
        req_id.to_string()
    } else {
        pane_request_anchor_from_prompt(pending_prompt)
            .filter(|anchor| content.contains(anchor))
            .unwrap_or_default()
    };
    if anchor.is_empty() {
        return None;
    }
    let completed = pane_ready_for_input(&content);
    let reply = if completed {
        extract_kimi_pane_reply(&content, &anchor)
    } else {
        String::new()
    };
    Some(KimiTurnObservation {
        request_seen: true,
        completed: completed && !reply.is_empty(),
        reply,
        session_id: Some(pane_id.to_string()),
        session_path: Some(PathBuf::from(format!("pane:{pane_id}"))),
        provider_turn_ref: Some(format!("pane:{pane_id}:{req_id}")),
        line_count: content.lines().count(),
        native_started_at: None,
        native_completed_at: None,
    })
}

fn pane_request_anchor_from_prompt(prompt: &str) -> Option<String> {
    let mut lines = prompt
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    while let Some(line) = lines.next() {
        if line.starts_with(protocol::REQ_ID_PREFIX) {
            return lines.next().map(str::to_string).filter(|s| !s.is_empty());
        }
    }
    None
}

fn extract_kimi_pane_reply(content: &str, anchor: &str) -> String {
    let tail = content.split(anchor).last().unwrap_or("");
    let lines: Vec<&str> = tail.lines().collect();
    let mut answer_start: Option<(usize, String)> = None;
    for (index, line) in lines.iter().enumerate() {
        let stripped = line.trim();
        if !stripped.starts_with(['●', '•']) {
            continue;
        }
        let candidate = stripped.trim_start_matches(['●', '•']).trim().to_string();
        if candidate.is_empty() || looks_like_kimi_non_answer(&candidate) {
            continue;
        }
        answer_start = Some((index, candidate));
        break;
    }
    let Some((index, first)) = answer_start else {
        return String::new();
    };
    let mut reply_lines = vec![first];
    for line in lines.iter().skip(index + 1) {
        let stripped = line.trim();
        if looks_like_kimi_input_box_line(stripped) {
            break;
        }
        if stripped.starts_with(['●', '•'])
            && looks_like_kimi_non_answer(stripped.trim_start_matches(['●', '•']).trim())
        {
            break;
        }
        reply_lines.push(line.trim_end().to_string());
    }
    clean_kimi_pane_reply(&reply_lines.join("\n"))
}

fn clean_kimi_pane_reply(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    while lines
        .first()
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        lines.remove(0);
    }
    while lines
        .last()
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        lines.pop();
    }
    if let Some(first) = lines.first_mut() {
        if first.trim().starts_with(['●', '•']) {
            *first = first
                .trim()
                .trim_start_matches(['●', '•'])
                .trim()
                .to_string();
        }
    }
    lines.join("\n").trim().to_string()
}

fn looks_like_kimi_input_box_line(stripped: &str) -> bool {
    !stripped.is_empty()
        && (stripped.starts_with('╭')
            || stripped.starts_with('╰')
            || stripped.starts_with("│ >")
            || stripped.contains("K2.7 Code") && stripped.contains("context:"))
}

fn looks_like_kimi_non_answer(text: &str) -> bool {
    let stripped = text.trim();
    let lowered = stripped.to_lowercase();
    if stripped.is_empty() {
        return true;
    }
    lowered.starts_with("the user ")
        || lowered.starts_with("user ")
        || lowered.starts_with("i should ")
        || lowered.starts_with("i need ")
        || lowered.starts_with("i'll ")
        || lowered.starts_with("i will ")
        || lowered.starts_with("let me ")
        || lowered.starts_with("thinking")
        || lowered.starts_with("using ")
        || lowered.starts_with("used ")
        || lowered.starts_with("running ")
        || lowered.starts_with("reading ")
}

fn new_box_ready_stable_secs() -> f64 {
    std::env::var("CCBR_KIMI_NEW_BOX_READY_STABLE_SECS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(NEW_BOX_READY_STABLE_SECS)
        .max(0.0)
}

fn resolve_work_dir(job: &JobRecord, context: Option<&ProviderRuntimeContext>) -> Option<PathBuf> {
    let candidate = context
        .and_then(|c| c.workspace_path.as_deref())
        .or(job.workspace_path.as_deref())?;
    if candidate.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_tilde(candidate)))
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

fn hash_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_now(now: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(now)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn seconds_between(start: &str, end: &str) -> f64 {
    match (parse_now(start), parse_now(end)) {
        (Some(s), Some(e)) => (e - s).num_milliseconds() as f64 / 1000.0,
        _ => 0.0,
    }
}

fn next_seq(state: &mut HashMap<String, Value>) -> u64 {
    let seq = runtime_u64(state, "next_seq").max(1);
    state.insert("next_seq".to_string(), Value::Number((seq + 1).into()));
    seq
}

fn runtime_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn runtime_u64(state: &HashMap<String, Value>, key: &str) -> u64 {
    state.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn runtime_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Deprecated alias retained for API compatibility.
///
/// New code should use [`crate::native_cli_support::wrap_native_prompt`].
pub fn wrap_kimi_prompt(message: &str, req_id: &str) -> String {
    wrap_native_prompt(message, req_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::with_prompt_target_override;
    use std::io::Write;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    fn write_lines(path: &Path, lines: &[&str]) {
        let mut file = std::fs::File::create(path).unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
    }

    #[derive(Default, Clone)]
    struct RecordingTarget {
        sent: Arc<Mutex<Vec<(String, String)>>>,
        content: Arc<Mutex<String>>,
        ready: Arc<Mutex<bool>>,
    }

    impl PromptTarget for RecordingTarget {
        fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
            self.sent
                .lock()
                .unwrap()
                .push((pane_id.to_string(), text.to_string()));
            Ok(())
        }

        fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
            if *self.ready.lock().unwrap() {
                Ok("── input\nagent (\n".to_string())
            } else {
                Ok(self.content.lock().unwrap().clone())
            }
        }
    }

    impl RecordingTarget {
        fn ready(self) -> Self {
            *self.ready.lock().unwrap() = true;
            self
        }

        fn sent_count(&self) -> usize {
            self.sent.lock().unwrap().len()
        }

        fn first_sent(&self) -> Option<(String, String)> {
            self.sent.lock().unwrap().first().cloned()
        }
    }

    #[test]
    fn test_manifest_is_pane_backed() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
    }

    #[test]
    fn test_backend_has_session_binding_and_launcher() {
        let b = backend();
        assert!(b.execution_adapter.is_none());
        assert_eq!(b.session_binding.as_ref().unwrap().provider, PROVIDER_NAME);
        assert!(b.runtime_launcher.is_some());
    }

    #[test]
    fn test_start_submission_missing_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let adapter = KimiExecutionAdapter;
        let ctx = ProviderRuntimeContext {
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        let sub = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
        assert_eq!(sub.provider, PROVIDER_NAME);
        assert!(sub
            .runtime_state
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("kimi_session_file_missing"));
    }

    #[test]
    fn test_start_submission_sends_prompt_when_ready() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".kimi-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let target = Arc::new(RecordingTarget::default().ready());
        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let adapter = KimiExecutionAdapter;
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });

        assert_eq!(target.sent_count(), 1);
        let (pane, prompt) = target.first_sent().unwrap();
        assert_eq!(pane, "%1");
        assert!(prompt.contains(protocol::REQ_ID_PREFIX));
        assert!(result
            .runtime_state
            .get("prompt_sent")
            .and_then(|v| v.as_bool())
            .unwrap());
        assert_eq!(result.runtime_state.get("backend_type").unwrap(), "tmux");
    }

    #[test]
    fn test_start_submission_defers_prompt_until_ready() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".kimi-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let target = Arc::new(RecordingTarget::default());
        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let adapter = KimiExecutionAdapter;
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });

        assert_eq!(target.sent_count(), 0);
        assert!(!result
            .runtime_state
            .get("prompt_sent")
            .and_then(|v| v.as_bool())
            .unwrap());
        assert!(result
            .runtime_state
            .get("prompt_deferred_until_ready")
            .and_then(|v| v.as_bool())
            .unwrap());
    }

    #[test]
    fn test_poll_submission_detects_reply() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let req_id = protocol::request_anchor_for_job("j1");
        write_json(
            &work_dir,
            ".kimi-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let home = tmp.path().join(".kimi");
        let sessions_root = home
            .join("sessions")
            .join(crate::kimi::native_log::kimi_project_hash(&work_dir));
        std::fs::create_dir_all(&sessions_root).unwrap();
        let wire_path = sessions_root.join("sess1").join("wire.jsonl");
        std::fs::create_dir(wire_path.parent().unwrap()).unwrap();
        write_lines(
            &wire_path,
            &[
                &format!(
                    r#"{{"type":"turn.prompt","payload":{{"user_input":[{{"text":"{} {}"}}],"turnId":"turn-1"}}}}"#,
                    protocol::REQ_ID_PREFIX,
                    req_id
                ),
                r#"{"type":"ContentPart","payload":{"text":"hello"}}"#,
                r#"{"type":"TurnEnd"}"#,
            ],
        );

        let target = Arc::new(RecordingTarget::default().ready());
        let adapter = KimiExecutionAdapter;
        std::env::set_var("KIMI_HOME", &home);
        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
            adapter.poll(&submission, "2025-01-01T00:00:01Z")
        });
        std::env::remove_var("KIMI_HOME");

        let result = result.expect("expected poll result");
        assert!(result.decision.is_some());
        assert_eq!(result.submission.reply, "hello");
        assert!(result.items.iter().any(|i| i.cursor.session_path.is_some()));
    }

    #[test]
    fn test_poll_deferred_prompt_sends_when_ready() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".kimi-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let target = Arc::new(RecordingTarget::default());
        let adapter = KimiExecutionAdapter;
        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let ctx = ProviderRuntimeContext {
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        let submission = with_prompt_target_override(target.clone(), || {
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });

        assert_eq!(target.sent_count(), 0);

        // Make the pane ready on the next poll.
        *target.ready.lock().unwrap() = true;
        let result = with_prompt_target_override(target.clone(), || {
            adapter.poll(&submission, "2025-01-01T00:00:01Z")
        });

        assert!(result.is_some());
        assert_eq!(target.sent_count(), 1);
        assert!(with_prompt_target_override(target.clone(), || {
            result
                .unwrap()
                .submission
                .runtime_state
                .get("prompt_sent")
                .and_then(|v| v.as_bool())
                .unwrap()
        }));
    }

    #[test]
    fn test_pane_ready_for_new_kimi_box_prompt() {
        let content = "\
╭────────────────────────────────────────────────────────────────────────────╮
│ >                                                                          │
╰────────────────────────────────────────────────────────────────────────────╯
yolo  K2.7 Code  /mnt/d/dapro-ass";

        assert!(pane_ready_for_input(content));
    }

    #[test]
    fn test_pane_ready_when_new_kimi_status_mentions_thinking() {
        let content = "\
╭────────────────────────────────────────────────────────────────────────────╮
│ >                                                                          │
╰────────────────────────────────────────────────────────────────────────────╯
yolo  K2.7 Code thinking  /mnt/d/dapro-ass /init: generate AGENTS.md";

        assert!(pane_ready_for_input(content));
    }

    #[test]
    fn test_start_submission_defers_new_box_prompt_until_stable() {
        std::env::set_var("CCBR_KIMI_NEW_BOX_READY_STABLE_SECS", "0");
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".kimi-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let target = RecordingTarget::default();
        *target.content.lock().unwrap() = "\
╭────────────────────────────────────────────────────────────────────────────╮
│ >                                                                          │
╰────────────────────────────────────────────────────────────────────────────╯
yolo  K2.7 Code thinking  /tmp/workspace"
            .to_string();
        let target = Arc::new(target);

        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let adapter = KimiExecutionAdapter;
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });
        std::env::remove_var("CCBR_KIMI_NEW_BOX_READY_STABLE_SECS");

        assert_eq!(target.sent_count(), 0);
        assert!(!result
            .runtime_state
            .get("prompt_sent")
            .and_then(|v| v.as_bool())
            .unwrap());
        assert_eq!(
            result
                .runtime_state
                .get("ready_candidate_seen_at")
                .and_then(|v| v.as_str()),
            Some("2025-01-01T00:00:00Z")
        );
    }

    #[test]
    fn test_extract_kimi_pane_reply_skips_reasoning_bullet() {
        let content = "\
✨ Reply exactly: CCBR_KIMI_SOCKET_LIVE_1782564301

● The user wants an exact reply with the literal string. I should output
  exactly that.

● CCBR_KIMI_SOCKET_LIVE_1782564301
╭────────────────────────────────────────────────────────────────────────────╮
│ >                                                                          │
╰────────────────────────────────────────────────────────────────────────────╯";

        assert_eq!(
            extract_kimi_pane_reply(content, "Reply exactly: CCBR_KIMI_SOCKET_LIVE_1782564301"),
            "CCBR_KIMI_SOCKET_LIVE_1782564301"
        );
    }

    #[test]
    fn test_pane_request_anchor_from_prompt_uses_user_request_line() {
        let prompt = "\
req- req-123

Reply exactly: TOKEN

CCBR reply guidance:
- Answer directly";

        assert_eq!(
            pane_request_anchor_from_prompt(prompt).as_deref(),
            Some("Reply exactly: TOKEN")
        );
    }
}
