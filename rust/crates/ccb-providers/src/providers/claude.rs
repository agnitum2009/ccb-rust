use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccb_provider_core::protocol::{
    extract_reply_for_req as protocol_extract_reply_for_req, is_done_text, make_req_id,
    request_anchor_for_job,
};
use serde_json::Value;

use crate::claude::{load_project_session, ClaudeLogReader};
use crate::execution::resolve_prompt_target_for_session;
use crate::execution::{
    backend_config_from_session_data, build_item, error_submission, no_wrap_requested,
    request_anchor_from_runtime_state, store_backend_config, CompletionReliabilityPolicy,
    ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};

pub const PROVIDER_NAME: &str = "claude";

const CLAUDE_REQ_ID_PREFIX: &str = "CCB_REQ_ID:";
const CLAUDE_BEGIN_PREFIX: &str = "<<BEGIN:";
const CLAUDE_DONE_PREFIX: &str = "<<DONE:";

const DEFAULT_READY_TIMEOUT_S: f64 = 8.0;
const PANE_CONTENT_LINES: usize = 120;

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// Build the Claude provider manifest.
/// Mirrors Python `provider_backends.claude.manifest.build_manifest`.
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
    profiles.insert(
        RuntimeMode::Headless,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "headless".to_string(),
            poll_interval_ms: 500,
            timeout_ms: 300_000,
            ..Default::default()
        },
    );
    ProviderManifest::new(
        provider, true, // supports_resume
        true, // supports_permission_auto
        true, // supports_stream_watch
        true, // supports_subagents
        true, // supports_workspace_attach
        profiles,
    )
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// Build a complete Claude provider backend.
/// Mirrors Python `provider_backends.claude.build_backend`.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        // The execution adapter is registered with the ccb-providers execution
        // registry rather than the ccb-provider-core backend slot because the
        // two crates currently define distinct ExecutionAdapter traits.
        execution_adapter: None,
        session_binding: Some(ProviderSessionBinding {
            provider: PROVIDER_NAME.to_string(),
            session_id_attr: "claude_session_id".to_string(),
            session_path_attr: "claude_session_path".to_string(),
        }),
        runtime_launcher: Some(ProviderRuntimeLauncher {
            provider: PROVIDER_NAME.to_string(),
            launch_mode: LaunchMode::SimpleTmux,
        }),
    }
}

// ---------------------------------------------------------------------------
// Reliability policy
// ---------------------------------------------------------------------------

fn claude_reliability_policy() -> &'static CompletionReliabilityPolicy {
    static POLICY: OnceLock<CompletionReliabilityPolicy> = OnceLock::new();
    POLICY.get_or_init(|| {
        CompletionReliabilityPolicy::new(PROVIDER_NAME, 900.0, "hook_artifact_or_session_event_log")
    })
}

// ---------------------------------------------------------------------------
// Execution adapter
// ---------------------------------------------------------------------------

/// Claude execution adapter.
/// Mirrors Python `provider_backends.claude.execution.ClaudeProviderAdapter`.
pub struct ClaudeExecutionAdapter;

impl ExecutionAdapter for ClaudeExecutionAdapter {
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

    fn export_runtime_state(
        &self,
        submission: &ProviderSubmission,
    ) -> Option<HashMap<String, Value>> {
        Some(export_claude_runtime_state(submission))
    }

    fn resume(
        &self,
        _job: &JobRecord,
        submission: &ProviderSubmission,
        context: Option<&ProviderRuntimeContext>,
        _persisted_state: &crate::execution::PersistedExecutionState,
        _now: &str,
    ) -> Option<ProviderSubmission> {
        if context.is_none() || context.and_then(|c| c.workspace_path.as_ref()).is_none() {
            return None;
        }
        let mut resumed = submission.clone();
        resumed
            .runtime_state
            .insert("mode".to_string(), Value::String("active".to_string()));
        Some(resumed)
    }

    fn reliability_policy(&self) -> Option<&CompletionReliabilityPolicy> {
        Some(claude_reliability_policy())
    }
}

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let workspace_path = context
        .and_then(|c| c.workspace_path.as_deref())
        .map(PathBuf::from);

    if let Some(ws) = &workspace_path {
        if ws.as_os_str().is_empty() || !ws.exists() {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::SessionEventLog,
                "missing_workspace",
                "workspace path missing or does not exist",
            );
        }
    }

    let instance = agent_instance(&job.agent_name);
    let session = workspace_path.as_ref().and_then(|ws| {
        if let Some(inst) = &instance {
            if let Some(s) = load_project_session(ws, Some(inst)) {
                return Some(s);
            }
        }
        load_project_session(ws, None)
    });

    if workspace_path.is_some() && session.is_none() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::SessionEventLog,
            "missing_claude_session",
            "claude session file not found",
        );
    }

    let request_anchor = request_anchor_for_job(&job.job_id);
    let no_wrap = no_wrap_requested(job.provider_options.get("no_wrap").or_else(|| {
        job.provider_options
            .get("options")
            .and_then(|v| v.as_object())
            .and_then(|m| m.get("no_wrap"))
    }));
    let reply_delivery = job
        .request
        .message_type
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase()
        == "reply_delivery";

    let session_path = session
        .as_ref()
        .and_then(|s| s.claude_session_path())
        .map(PathBuf::from);
    let claude_projects_root = session.as_ref().and_then(|s| s.claude_projects_root());
    let completion_dir = session.as_ref().and_then(|s| s.completion_dir());
    let pane_id = session
        .as_ref()
        .and_then(|s| s.pane_id())
        .map(|s| s.to_string());
    let backend_config = session
        .as_ref()
        .map(|s| backend_config_from_session_data(&s.data));

    let reader_state = session
        .as_ref()
        .map(|s| ClaudeLogReader::from_session(s).capture_state())
        .unwrap_or_default();

    let prompt = if no_wrap {
        job.request.body.clone()
    } else if completion_dir.is_some() {
        wrap_claude_turn_prompt(&job.request.body, &make_req_id(&job.job_id))
    } else {
        wrap_claude_prompt(&job.request.body, &make_req_id(&job.job_id))
    };

    let mut prompt_sent = false;
    let mut prompt_deferred_for_ready = false;
    let mut send_error: Option<String> = None;

    if let (Some(pane_id), Some(_backend_config)) = (&pane_id, &backend_config) {
        if let Some(target) = resolve_prompt_target_for_session(&session.as_ref().unwrap().data) {
            match target.get_pane_content(pane_id, PANE_CONTENT_LINES) {
                Ok(content) if looks_ready(&content) => {
                    if let Err(err) = target.send_text(pane_id, &prompt) {
                        send_error = Some(err);
                    } else {
                        prompt_sent = true;
                    }
                }
                _ => {
                    prompt_deferred_for_ready = true;
                }
            }
        } else {
            prompt_deferred_for_ready = true;
        }
    }

    let mut diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "active",
        "workspace_path": workspace_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
    });
    if let Some(err) = &send_error {
        diagnostics["send_error"] = Value::String(err.clone());
    }
    if prompt_deferred_for_ready {
        diagnostics["prompt_deferred_for_ready"] = Value::Bool(true);
    }

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_seen".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("raw_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert(
        "session_path".to_string(),
        session_path
            .map(|p| Value::String(p.to_string_lossy().to_string()))
            .unwrap_or(Value::Null),
    );
    runtime_state.insert(
        "completion_dir".to_string(),
        completion_dir
            .map(|p| Value::String(p.to_string_lossy().to_string()))
            .unwrap_or(Value::String(String::new())),
    );
    if let Some(root) = &claude_projects_root {
        runtime_state.insert(
            "claude_projects_root".to_string(),
            Value::String(root.to_string_lossy().to_string()),
        );
    }
    if let Some(ws) = &workspace_path {
        runtime_state.insert(
            "workspace_path".to_string(),
            Value::String(ws.to_string_lossy().to_string()),
        );
    }
    runtime_state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("prompt_text".to_string(), Value::String(prompt));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(prompt_sent));
    runtime_state.insert(
        "prompt_deferred_for_ready".to_string(),
        Value::Bool(prompt_deferred_for_ready),
    );
    runtime_state.insert(
        "reply_delivery_complete_on_dispatch".to_string(),
        Value::Bool(reply_delivery),
    );
    runtime_state.insert(
        "reply_delivery_require_ready".to_string(),
        Value::Bool(reply_delivery),
    );
    runtime_state.insert(
        "ready_wait_started_at".to_string(),
        Value::String(now.to_string()),
    );
    runtime_state.insert(
        "ready_timeout_s".to_string(),
        Value::Number(
            serde_json::Number::from_f64(resolve_ready_timeout_s()).unwrap_or_else(|| 0.into()),
        ),
    );
    runtime_state.insert(
        "reader_state".to_string(),
        Value::Object(reader_state.into_iter().collect()),
    );
    if let Some(pane_id) = &pane_id {
        runtime_state.insert("pane_id".to_string(), Value::String(pane_id.clone()));
    }
    if let Some(backend_config) = &backend_config {
        store_backend_config(&mut runtime_state, backend_config);
    }
    if let Some(err) = send_error {
        runtime_state.insert("send_error".to_string(), Value::String(err));
    }

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

fn agent_instance(agent_name: &str) -> Option<String> {
    let name = agent_name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_lowercase())
    }
}

fn poll_submission(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    if submission.is_terminal() {
        return None;
    }

    if !runtime_bool(&submission.runtime_state, "prompt_sent") {
        if runtime_str(&submission.runtime_state, "pane_id").is_empty()
            || runtime_str(&submission.runtime_state, "backend_type").is_empty()
        {
            return Some(dispatch_deferred_prompt(submission, now));
        }
        return dispatch_deferred_prompt_when_ready(submission, now);
    }

    if runtime_bool(
        &submission.runtime_state,
        "reply_delivery_complete_on_dispatch",
    ) {
        return Some(reply_delivery_terminal_result(submission, now));
    }

    if let Some(result) = poll_exact_hook(submission, now) {
        return Some(result);
    }

    if let Some(result) = poll_event_batches(submission, now) {
        return Some(result);
    }

    // Live-mode fallback: detect turn completion from pane text when no
    // structured hook/event data is available (no completion_dir events,
    // no session log batches). If the pane shows a ready prompt (❯) after
    // the prompt was sent, the turn is complete.
    if let Some(result) = poll_pane_text_completion(submission, now) {
        return Some(result);
    }

    None
}

fn dispatch_deferred_prompt_when_ready(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    let pane_id = runtime_str(&submission.runtime_state, "pane_id");
    let prompt = runtime_str(&submission.runtime_state, "prompt_text");
    if pane_id.is_empty() || prompt.is_empty() {
        return None;
    }

    // Dismiss provider startup screens (e.g., Claude's "Press Enter to
    // continue…" first-run security prompt) before checking readiness.
    // Send Enter and defer — the next poll will find the pane interactive.
    if let Some(target) = crate::execution::resolve_prompt_target(&submission.runtime_state) {
        if target
            .get_pane_content(&pane_id, PANE_CONTENT_LINES)
            .map(|c| c.contains("Press Enter to continue"))
            .unwrap_or(false)
        {
            let _ = target.send_text(&pane_id, "");
            return None;
        }
    }

    let ready =
        if let Some(target) = crate::execution::resolve_prompt_target(&submission.runtime_state) {
            target
                .get_pane_content(&pane_id, PANE_CONTENT_LINES)
                .map(|content| looks_ready(&content))
                .unwrap_or(true)
        } else {
            true
        };

    if !ready && !ready_wait_timed_out(submission, now) {
        let mut updated = submission.clone();
        updated
            .runtime_state
            .insert("prompt_deferred_for_ready".to_string(), Value::Bool(true));
        return None;
    }

    if let Some(target) = crate::execution::resolve_prompt_target(&submission.runtime_state) {
        if let Err(err) = target.send_text(&pane_id, &prompt) {
            let mut updated = submission.clone();
            updated
                .runtime_state
                .insert("send_error".to_string(), Value::String(err));
            updated
                .runtime_state
                .insert("prompt_sent".to_string(), Value::Bool(false));
            updated
                .runtime_state
                .insert("prompt_deferred_for_ready".to_string(), Value::Bool(true));
            return Some(ProviderPollResult::new(updated, Vec::new(), None));
        }
    } else {
        let mut updated = submission.clone();
        updated.runtime_state.insert(
            "send_error".to_string(),
            Value::String("missing_prompt_target".to_string()),
        );
        updated
            .runtime_state
            .insert("prompt_sent".to_string(), Value::Bool(false));
        updated
            .runtime_state
            .insert("prompt_deferred_for_ready".to_string(), Value::Bool(true));
        return Some(ProviderPollResult::new(updated, Vec::new(), None));
    }
    Some(dispatch_deferred_prompt(submission, now))
}

fn dispatch_deferred_prompt(submission: &ProviderSubmission, now: &str) -> ProviderPollResult {
    let mut updated = submission.clone();
    let next_seq = runtime_u64(&updated.runtime_state, "next_seq").max(1);
    let anchor_seen = runtime_bool(&updated.runtime_state, "anchor_seen");
    let deferred_for_ready = runtime_bool(&updated.runtime_state, "prompt_deferred_for_ready");
    let anchor_emitted = deferred_for_ready && !anchor_seen;

    updated
        .runtime_state
        .insert("prompt_sent".to_string(), Value::Bool(true));
    updated
        .runtime_state
        .insert("prompt_sent_at".to_string(), Value::String(now.to_string()));
    updated.runtime_state.insert(
        "anchor_seen".to_string(),
        Value::Bool(anchor_seen || anchor_emitted),
    );
    updated.runtime_state.insert(
        "next_seq".to_string(),
        Value::Number((next_seq + if anchor_emitted { 1 } else { 0 }).into()),
    );
    updated
        .runtime_state
        .insert("prompt_deferred_for_ready".to_string(), Value::Bool(false));
    if anchor_emitted {
        updated.runtime_state.insert(
            "prompt_anchor_emitted_at".to_string(),
            Value::String(now.to_string()),
        );
    }

    let mut items = Vec::new();
    if anchor_emitted {
        let request_anchor =
            request_anchor_from_runtime_state(&updated.runtime_state, &updated.job_id);
        let session_path = runtime_str(&updated.runtime_state, "session_path");
        let session_path_opt = if session_path.is_empty() {
            None
        } else {
            Some(session_path.clone())
        };
        let mut payload = HashMap::new();
        payload.insert("turn_id".to_string(), Value::String(request_anchor));
        payload.insert(
            "session_path".to_string(),
            session_path_opt.map(Value::String).unwrap_or(Value::Null),
        );
        items.push(build_item(
            &updated,
            CompletionItemKind::AnchorSeen,
            now,
            next_seq,
            payload,
        ));
    }

    ProviderPollResult::new(updated, items, None)
}

fn ready_wait_timed_out(submission: &ProviderSubmission, now: &str) -> bool {
    let started_at = runtime_str(&submission.runtime_state, "ready_wait_started_at");
    if started_at.is_empty() {
        return true;
    }
    let timeout_s = runtime_f64(
        &submission.runtime_state,
        "ready_timeout_s",
        DEFAULT_READY_TIMEOUT_S,
    );
    if timeout_s <= 0.0 {
        return true;
    }
    let Ok(start) = ccb_completion::utils::parse_timestamp(&started_at) else {
        return true;
    };
    let Ok(end) = ccb_completion::utils::parse_timestamp(now) else {
        return true;
    };
    (end - start).num_milliseconds() as f64 / 1000.0 >= timeout_s
}

fn reply_delivery_terminal_result(
    submission: &ProviderSubmission,
    now: &str,
) -> ProviderPollResult {
    let provider_turn_ref = runtime_str(&submission.runtime_state, "request_anchor");
    let provider_turn_ref = if provider_turn_ref.is_empty() {
        submission.job_id.clone()
    } else {
        provider_turn_ref
    };

    let diagnostics = serde_json::Map::from_iter([
        ("reply_delivery".to_string(), Value::Bool(true)),
        (
            "delivery_status".to_string(),
            Value::String("sent".to_string()),
        ),
        (
            "provider".to_string(),
            Value::String(PROVIDER_NAME.to_string()),
        ),
        (
            "submission_mode".to_string(),
            Value::String("active".to_string()),
        ),
    ]);

    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("reply_delivery_sent".to_string()),
        confidence: Some(CompletionConfidence::Observed),
        reply: String::new(),
        anchor_seen: true,
        reply_started: false,
        reply_stable: true,
        provider_turn_ref: Some(provider_turn_ref),
        source_cursor: None,
        finished_at: Some(now.to_string()),
        diagnostics,
    };

    let mut updated = submission.clone();
    updated.reply =
        request_anchor_from_runtime_state(&submission.runtime_state, &submission.job_id);
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

/// Detect turn completion from pane text when no structured hook/event data
/// exists. If the pane shows a ready prompt (❯) after the prompt was sent,
/// Claude has finished responding → terminal completion.
fn poll_pane_text_completion(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    if expects_structured_claude_log(submission) {
        return None;
    }
    if !runtime_bool(&submission.runtime_state, "anchor_seen") {
        return None;
    }
    let buffer = runtime_str(&submission.runtime_state, "pane_text_buffer");
    let buffer = if buffer.is_empty() {
        runtime_str(&submission.runtime_state, "reply_buffer")
    } else {
        buffer
    };
    let reply = buffer.trim();
    if reply.is_empty() || looks_like_claude_tui_chrome(reply) {
        return None;
    }

    let pane_id = runtime_str(&submission.runtime_state, "pane_id");
    if pane_id.is_empty() {
        return None;
    }
    let target = crate::execution::resolve_prompt_target(&submission.runtime_state)?;
    let pane_content = target.get_pane_content(&pane_id, PANE_CONTENT_LINES).ok()?;
    if !looks_ready(&pane_content) {
        return None;
    }

    let provider_turn_ref = runtime_str(&submission.runtime_state, "request_anchor");
    let provider_turn_ref = if provider_turn_ref.is_empty() {
        submission.job_id.clone()
    } else {
        provider_turn_ref
    };

    let mut diagnostics = serde_json::Map::new();
    diagnostics.insert("pane_text_completion".to_string(), Value::Bool(true));
    diagnostics.insert(
        "provider".to_string(),
        Value::String(PROVIDER_NAME.to_string()),
    );

    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("pane_text_turn_boundary".to_string()),
        confidence: Some(CompletionConfidence::Observed),
        reply: reply.to_string(),
        anchor_seen: true,
        reply_started: true,
        reply_stable: true,
        provider_turn_ref: Some(provider_turn_ref),
        source_cursor: None,
        finished_at: Some(now.to_string()),
        diagnostics,
    };

    let mut updated = submission.clone();
    updated
        .runtime_state
        .insert("turn_boundary_detected".to_string(), Value::Bool(true));
    Some(ProviderPollResult::new(updated, Vec::new(), Some(decision)))
}

fn expects_structured_claude_log(submission: &ProviderSubmission) -> bool {
    !runtime_str(&submission.runtime_state, "session_path").is_empty()
        || !runtime_str(&submission.runtime_state, "claude_projects_root").is_empty()
}

fn poll_exact_hook(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    let completion_dir = runtime_str(&submission.runtime_state, "completion_dir");
    let request_anchor =
        request_anchor_from_runtime_state(&submission.runtime_state, &submission.job_id);
    if completion_dir.is_empty() || request_anchor.is_empty() {
        return None;
    }

    let path = PathBuf::from(&completion_dir)
        .join("events")
        .join(format!("{}.json", request_anchor));
    let raw = std::fs::read_to_string(&path).ok()?;
    let event: Value = serde_json::from_str(&raw).ok()?;
    let obj = event.as_object()?;

    let reply = obj
        .get("reply")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let status_str = obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");
    let mut status = match status_str.to_lowercase().as_str() {
        "failed" => CompletionStatus::Failed,
        "cancelled" => CompletionStatus::Cancelled,
        "incomplete" => CompletionStatus::Incomplete,
        _ => CompletionStatus::Completed,
    };
    let diagnostics = obj
        .get("diagnostics")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let mut diagnostics_map = diagnostics.as_object().cloned().unwrap_or_default();

    if reply.is_empty()
        && (status == CompletionStatus::Completed || status == CompletionStatus::Incomplete)
    {
        status = CompletionStatus::Incomplete;
        diagnostics_map
            .entry("reason".to_string())
            .or_insert_with(|| Value::String("hook_stop_empty_reply".to_string()));
        diagnostics_map
            .entry("empty_reply".to_string())
            .or_insert(Value::Bool(true));
        diagnostics_map
            .entry("error_type".to_string())
            .or_insert_with(|| Value::String("empty_provider_reply".to_string()));
        diagnostics_map.entry("message".to_string()).or_insert_with(|| {
            Value::String(
                "Provider completion hook fired without assistant reply text; inspect the provider transcript, pane state, and authentication/API output.".to_string(),
            )
        });
        let message = diagnostics_map
            .get("message")
            .cloned()
            .unwrap_or(Value::Null);
        diagnostics_map
            .entry("diagnosis".to_string())
            .or_insert(message);
    }

    let reason = diagnostics_map
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("hook_stop")
        .to_string();
    let provider_turn_ref = obj
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&request_anchor)
        .to_string();
    let timestamp = obj.get("timestamp").and_then(|v| v.as_str()).unwrap_or(now);
    let next_seq = runtime_u64(&submission.runtime_state, "next_seq").max(1);

    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(next_seq),
        updated_at: Some(timestamp.to_string()),
        opaque_cursor: Some(path.to_string_lossy().to_string()),
        ..Default::default()
    };

    let mut item_payload = serde_json::Map::new();
    item_payload.insert("reply".to_string(), Value::String(reply.clone()));
    item_payload.insert("text".to_string(), Value::String(reply.clone()));
    item_payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
    item_payload.insert(
        "provider_turn_ref".to_string(),
        Value::String(provider_turn_ref.clone()),
    );
    item_payload.insert(
        "completion_source".to_string(),
        Value::String("hook_artifact".to_string()),
    );
    item_payload.insert(
        "hook_event_name".to_string(),
        obj.get("hook_event_name").cloned().unwrap_or(Value::Null),
    );
    item_payload.insert("status".to_string(), Value::String(status_str.to_string()));
    for (k, v) in &diagnostics_map {
        if !item_payload.contains_key(k) {
            item_payload.insert(k.clone(), v.clone());
        }
    }

    let item = CompletionItem {
        kind: CompletionItemKind::AssistantFinal,
        timestamp: timestamp.to_string(),
        cursor,
        provider: submission.provider.clone(),
        agent_name: submission.agent_name.clone(),
        req_id: submission.job_id.clone(),
        payload: item_payload,
    };

    let mut updated = submission.clone();
    updated.reply = reply.clone();
    updated
        .runtime_state
        .insert("next_seq".to_string(), Value::Number((next_seq + 1).into()));

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason),
        confidence: Some(CompletionConfidence::Exact),
        reply,
        anchor_seen: runtime_bool(&submission.runtime_state, "anchor_seen"),
        reply_started: !updated.reply.is_empty(),
        reply_stable: !updated.reply.is_empty(),
        provider_turn_ref: Some(provider_turn_ref),
        source_cursor: Some(item.cursor.clone()),
        finished_at: Some(timestamp.to_string()),
        diagnostics: diagnostics_map,
    };

    Some(ProviderPollResult::new(updated, vec![item], Some(decision)))
}

fn poll_event_batches(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    // Legacy test seam: if pending_events is present, process them directly.
    if submission.runtime_state.contains_key("pending_events") {
        return process_pending_events(submission, now);
    }

    let session_path = runtime_str(&submission.runtime_state, "session_path");
    let work_dir = runtime_str(&submission.runtime_state, "workspace_path");
    if session_path.is_empty() && work_dir.is_empty() {
        return None;
    }

    let reader = build_reader(submission);
    let prev_state: HashMap<String, Value> = submission
        .runtime_state
        .get("reader_state")
        .and_then(|v| v.as_object().cloned().map(|m| m.into_iter().collect()))
        .unwrap_or_else(|| reader.capture_state());

    let (entries, new_state) = reader.try_get_entries(&prev_state);

    let mut poll = PollState::from_submission(submission);
    let mut items = Vec::new();

    let new_session_path = new_state
        .get("session_path")
        .and_then(|v| v.as_str())
        .unwrap_or(&session_path)
        .to_string();
    if !new_session_path.is_empty() && new_session_path != poll.session_path {
        apply_session_rotation(submission, &mut poll, &new_session_path, now, &mut items);
    }

    for entry in entries {
        let obj = entry.to_value().as_object().cloned()?;
        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" => handle_user_event(submission, &mut poll, &obj, now, &mut items),
            "system" => {
                if let Some(result) =
                    handle_system_event(submission, &mut poll, &obj, now, &mut items)
                {
                    let mut updated = submission.clone();
                    updated.reply = poll.reply_buffer.clone();
                    updated.runtime_state = apply_poll_state(&updated.runtime_state, &poll);
                    updated.runtime_state.insert(
                        "reader_state".to_string(),
                        Value::Object(new_state.into_iter().collect()),
                    );
                    return Some(merge_items(result, items));
                }
            }
            "assistant" if poll.anchor_seen => {
                handle_assistant_event(submission, &mut poll, &obj, now, &mut items);
            }
            _ => {}
        }
        if poll.reached_turn_boundary {
            break;
        }
    }

    let mut updated = submission.clone();
    updated.reply = poll.reply_buffer.clone();
    updated.runtime_state = apply_poll_state(&updated.runtime_state, &poll);
    updated.runtime_state.insert(
        "reader_state".to_string(),
        Value::Object(new_state.into_iter().collect()),
    );

    if items.is_empty() {
        return None;
    }
    Some(ProviderPollResult::new(updated, items, None))
}

fn build_reader(submission: &ProviderSubmission) -> ClaudeLogReader {
    let session_path = runtime_str(&submission.runtime_state, "session_path");
    let work_dir = runtime_str(&submission.runtime_state, "workspace_path");
    let projects_root = runtime_str(&submission.runtime_state, "claude_projects_root");

    let mut reader = if !projects_root.is_empty() {
        ClaudeLogReader::new(Some(Path::new(&projects_root)), Path::new(&work_dir))
    } else if !work_dir.is_empty() {
        ClaudeLogReader::new(None, Path::new(&work_dir))
    } else {
        ClaudeLogReader::new(None, Path::new("."))
    };
    if !session_path.is_empty() {
        reader.set_preferred_session(Some(PathBuf::from(&session_path)));
    }
    reader
}

fn merge_items(
    result: ProviderPollResult,
    mut prefix_items: Vec<CompletionItem>,
) -> ProviderPollResult {
    prefix_items.extend(result.items);
    ProviderPollResult::new(result.submission, prefix_items, result.decision)
}

fn apply_session_rotation(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    new_session_path: &str,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    let mut payload = HashMap::new();
    payload.insert(
        "session_path".to_string(),
        Value::String(new_session_path.to_string()),
    );
    payload.insert(
        "provider_session_id".to_string(),
        Value::String(
            PathBuf::from(new_session_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        ),
    );
    items.push(build_item(
        submission,
        CompletionItemKind::SessionRotate,
        now,
        poll.next_seq,
        payload,
    ));
    poll.next_seq += 1;
    poll.session_path = new_session_path.to_string();
    poll.anchor_seen = runtime_bool(&submission.runtime_state, "no_wrap");
    poll.reply_buffer.clear();
    poll.raw_buffer.clear();
    poll.last_assistant_uuid.clear();
}

struct PollState {
    request_anchor: String,
    next_seq: u64,
    anchor_seen: bool,
    reply_buffer: String,
    raw_buffer: String,
    session_path: String,
    last_assistant_uuid: String,
    reached_turn_boundary: bool,
}

impl PollState {
    fn from_submission(submission: &ProviderSubmission) -> Self {
        Self {
            request_anchor: request_anchor_from_runtime_state(
                &submission.runtime_state,
                &submission.job_id,
            ),
            next_seq: runtime_u64(&submission.runtime_state, "next_seq").max(1),
            anchor_seen: runtime_bool(&submission.runtime_state, "anchor_seen"),
            reply_buffer: runtime_str(&submission.runtime_state, "reply_buffer"),
            raw_buffer: runtime_str(&submission.runtime_state, "raw_buffer"),
            session_path: runtime_str(&submission.runtime_state, "session_path"),
            last_assistant_uuid: runtime_str(&submission.runtime_state, "last_assistant_uuid"),
            reached_turn_boundary: false,
        }
    }
}

fn apply_poll_state(state: &HashMap<String, Value>, poll: &PollState) -> HashMap<String, Value> {
    let mut out = state.clone();
    out.insert("next_seq".to_string(), Value::Number(poll.next_seq.into()));
    out.insert("anchor_seen".to_string(), Value::Bool(poll.anchor_seen));
    out.insert(
        "reply_buffer".to_string(),
        Value::String(poll.reply_buffer.clone()),
    );
    out.insert(
        "raw_buffer".to_string(),
        Value::String(poll.raw_buffer.clone()),
    );
    out.insert(
        "session_path".to_string(),
        Value::String(poll.session_path.clone()),
    );
    out.insert(
        "last_assistant_uuid".to_string(),
        Value::String(poll.last_assistant_uuid.clone()),
    );
    out
}

fn handle_user_event(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    event: &serde_json::Map<String, Value>,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    let text = event.get("text").and_then(|v| v.as_str()).unwrap_or("");
    if request_anchor_seen_in_text(text, &poll.request_anchor) && !poll.anchor_seen {
        let mut payload = HashMap::new();
        payload.insert(
            "turn_id".to_string(),
            Value::String(poll.request_anchor.clone()),
        );
        payload.insert(
            "session_path".to_string(),
            if poll.session_path.is_empty() {
                Value::Null
            } else {
                Value::String(poll.session_path.clone())
            },
        );
        items.push(build_item(
            submission,
            CompletionItemKind::AnchorSeen,
            now,
            poll.next_seq,
            payload,
        ));
        poll.next_seq += 1;
        poll.anchor_seen = true;
    }
}

fn request_anchor_seen_in_text(text: &str, request_anchor: &str) -> bool {
    if request_anchor.is_empty() {
        return false;
    }
    if text.contains(request_anchor) {
        return true;
    }
    let Some(req_id) = req_id_from_request_anchor(request_anchor) else {
        return false;
    };
    text.contains(&format!("{} {}", CLAUDE_REQ_ID_PREFIX, req_id))
        || text.contains(&format!("{}{}", CLAUDE_REQ_ID_PREFIX, req_id))
}

fn req_id_from_request_anchor(request_anchor: &str) -> Option<String> {
    let text = request_anchor.trim();
    if let Some(inner) = text
        .strip_prefix(CLAUDE_BEGIN_PREFIX)
        .and_then(|s| s.strip_suffix(">>"))
    {
        return Some(inner.trim().to_string()).filter(|s| !s.is_empty());
    }
    if let Some(inner) = text.strip_prefix(CLAUDE_REQ_ID_PREFIX) {
        return Some(inner.trim().to_string()).filter(|s| !s.is_empty());
    }
    text.strip_prefix("req-")
        .map(|_| text.to_string())
        .filter(|s| !s.is_empty())
}

fn handle_system_event(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    event: &serde_json::Map<String, Value>,
    now: &str,
    items: &mut Vec<CompletionItem>,
) -> Option<ProviderPollResult> {
    if let Some(api_error) = terminal_api_error_payload(event) {
        let timestamp = api_error
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or(now)
            .to_string();
        let mut payload = serde_json::Map::new();
        payload.insert("reason".to_string(), Value::String("api_error".to_string()));
        payload.insert(
            "turn_id".to_string(),
            Value::String(poll.request_anchor.clone()),
        );
        payload.insert(
            "session_path".to_string(),
            if poll.session_path.is_empty() {
                Value::Null
            } else {
                Value::String(poll.session_path.clone())
            },
        );
        for (k, v) in &api_error {
            payload.insert(k.clone(), v.clone());
        }

        items.push(build_item(
            submission,
            CompletionItemKind::Error,
            &timestamp,
            poll.next_seq,
            payload.into_iter().collect(),
        ));
        poll.next_seq += 1;

        let cursor = CompletionCursor {
            source_kind: submission.source_kind,
            event_seq: Some(poll.next_seq),
            updated_at: Some(timestamp.clone()),
            session_path: if poll.session_path.is_empty() {
                None
            } else {
                Some(poll.session_path.clone())
            },
            ..Default::default()
        };

        let mut diagnostics = serde_json::Map::new();
        diagnostics.insert(
            "error_type".to_string(),
            Value::String("provider_api_error".to_string()),
        );
        diagnostics.insert(
            "error_code".to_string(),
            api_error.get("error_code").cloned().unwrap_or(Value::Null),
        );
        diagnostics.insert(
            "error_path".to_string(),
            api_error.get("error_path").cloned().unwrap_or(Value::Null),
        );
        diagnostics.insert(
            "retry_attempt".to_string(),
            api_error
                .get("retry_attempt")
                .cloned()
                .unwrap_or(Value::Null),
        );
        diagnostics.insert(
            "max_retries".to_string(),
            api_error.get("max_retries").cloned().unwrap_or(Value::Null),
        );

        let decision = CompletionDecision {
            terminal: true,
            status: CompletionStatus::Failed,
            reason: Some("api_error".to_string()),
            confidence: Some(CompletionConfidence::Observed),
            reply: poll.reply_buffer.clone(),
            anchor_seen: poll.anchor_seen,
            reply_started: !poll.reply_buffer.is_empty(),
            reply_stable: !poll.reply_buffer.is_empty(),
            provider_turn_ref: Some(if poll.request_anchor.is_empty() {
                poll.session_path.clone()
            } else {
                poll.request_anchor.clone()
            }),
            source_cursor: Some(cursor),
            finished_at: Some(timestamp),
            diagnostics,
        };

        let mut updated = submission.clone();
        updated.reply = poll.reply_buffer.clone();
        updated.runtime_state = apply_poll_state(&updated.runtime_state, poll);
        updated
            .runtime_state
            .insert("mode".to_string(), Value::String("passive".to_string()));
        return Some(ProviderPollResult::new(
            updated,
            items.clone(),
            Some(decision),
        ));
    }

    if is_turn_boundary_event(event, &poll.last_assistant_uuid) {
        let mut payload = HashMap::new();
        payload.insert(
            "reason".to_string(),
            Value::String("turn_duration".to_string()),
        );
        payload.insert(
            "last_agent_message".to_string(),
            Value::String(poll.reply_buffer.clone()),
        );
        payload.insert(
            "turn_id".to_string(),
            Value::String(poll.request_anchor.clone()),
        );
        payload.insert(
            "session_path".to_string(),
            if poll.session_path.is_empty() {
                Value::Null
            } else {
                Value::String(poll.session_path.clone())
            },
        );
        payload.insert(
            "assistant_uuid".to_string(),
            if poll.last_assistant_uuid.is_empty() {
                Value::Null
            } else {
                Value::String(poll.last_assistant_uuid.clone())
            },
        );
        items.push(build_item(
            submission,
            CompletionItemKind::TurnBoundary,
            now,
            poll.next_seq,
            payload,
        ));
        poll.next_seq += 1;
        poll.reached_turn_boundary = true;
    }

    None
}

fn handle_assistant_event(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    event: &serde_json::Map<String, Value>,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    let text = event
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let subagent_id = event
        .get("subagent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let subagent_name = event
        .get("subagent_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_subagent = !subagent_id.is_empty() || !subagent_name.is_empty();
    let event_assistant_uuid = event
        .get("uuid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    poll.raw_buffer = append_buffer(&poll.raw_buffer, &text);
    let cleaned = strip_done_text_for_req(&text, &poll.request_anchor);
    if cleaned.trim().is_empty() {
        maybe_append_turn_boundary(submission, poll, now, items);
        maybe_append_assistant_end_turn_boundary(
            submission,
            poll,
            event,
            &cleaned,
            is_subagent,
            now,
            items,
        );
        return;
    }

    poll.reply_buffer = append_buffer(&poll.reply_buffer, &cleaned);
    if !is_subagent {
        poll.last_assistant_uuid.clone_from(&event_assistant_uuid);
    }
    let current_uuid = if event_assistant_uuid.is_empty() {
        poll.last_assistant_uuid.clone()
    } else {
        event_assistant_uuid.clone()
    };

    let mut payload = HashMap::new();
    payload.insert("text".to_string(), Value::String(cleaned));
    payload.insert(
        "merged_text".to_string(),
        Value::String(poll.reply_buffer.clone()),
    );
    payload.insert(
        "turn_id".to_string(),
        Value::String(poll.request_anchor.clone()),
    );
    payload.insert(
        "session_path".to_string(),
        if poll.session_path.is_empty() {
            Value::Null
        } else {
            Value::String(poll.session_path.clone())
        },
    );
    payload.insert(
        "assistant_uuid".to_string(),
        if current_uuid.is_empty() {
            Value::Null
        } else {
            Value::String(current_uuid)
        },
    );
    payload.insert(
        "subagent_id".to_string(),
        if subagent_id.is_empty() {
            Value::Null
        } else {
            Value::String(subagent_id)
        },
    );
    payload.insert(
        "subagent_name".to_string(),
        if subagent_name.is_empty() {
            Value::Null
        } else {
            Value::String(subagent_name)
        },
    );
    payload.insert(
        "stop_reason".to_string(),
        event.get("stop_reason").cloned().unwrap_or(Value::Null),
    );

    items.push(build_item(
        submission,
        CompletionItemKind::AssistantChunk,
        now,
        poll.next_seq,
        payload,
    ));
    poll.next_seq += 1;

    maybe_append_turn_boundary(submission, poll, now, items);
    let reply_buffer_snapshot = poll.reply_buffer.clone();
    maybe_append_assistant_end_turn_boundary(
        submission,
        poll,
        event,
        &reply_buffer_snapshot,
        is_subagent,
        now,
        items,
    );
}

fn maybe_append_turn_boundary(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    if poll.reached_turn_boundary
        || poll.request_anchor.is_empty()
        || !is_done_text(&poll.raw_buffer)
    {
        return;
    }
    let reply = protocol_extract_reply_for_req(&poll.raw_buffer, &poll.request_anchor);
    let reply = if reply.is_empty() {
        poll.reply_buffer.clone()
    } else {
        reply
    };
    append_turn_boundary_item(submission, poll, now, items, reply, "task_complete", None);
}

fn maybe_append_assistant_end_turn_boundary(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    event: &serde_json::Map<String, Value>,
    cleaned: &str,
    is_subagent: bool,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    if poll.reached_turn_boundary
        || is_subagent
        || !poll.anchor_seen
        || poll.request_anchor.is_empty()
    {
        return;
    }
    let stop_reason = event
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if stop_reason != "end_turn" {
        return;
    }
    let reply = if poll.reply_buffer.trim().is_empty() {
        cleaned.trim().to_string()
    } else {
        poll.reply_buffer.clone()
    };
    if reply.is_empty() {
        return;
    }
    if let Some(uuid) = event.get("uuid").and_then(|v| v.as_str()) {
        poll.last_assistant_uuid = uuid.to_string();
    }
    append_turn_boundary_item(
        submission,
        poll,
        now,
        items,
        reply,
        "assistant_end_turn",
        Some("end_turn"),
    );
}

fn append_turn_boundary_item(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    now: &str,
    items: &mut Vec<CompletionItem>,
    reply: String,
    reason: &str,
    stop_reason: Option<&str>,
) {
    let mut payload = HashMap::new();
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    payload.insert("last_agent_message".to_string(), Value::String(reply));
    payload.insert(
        "turn_id".to_string(),
        Value::String(poll.request_anchor.clone()),
    );
    payload.insert(
        "session_path".to_string(),
        if poll.session_path.is_empty() {
            Value::Null
        } else {
            Value::String(poll.session_path.clone())
        },
    );
    payload.insert(
        "assistant_uuid".to_string(),
        if poll.last_assistant_uuid.is_empty() {
            Value::Null
        } else {
            Value::String(poll.last_assistant_uuid.clone())
        },
    );
    if let Some(sr) = stop_reason {
        payload.insert("stop_reason".to_string(), Value::String(sr.to_string()));
    }

    items.push(build_item(
        submission,
        CompletionItemKind::TurnBoundary,
        now,
        poll.next_seq,
        payload,
    ));
    poll.next_seq += 1;
    poll.reached_turn_boundary = true;
}

fn terminal_api_error_payload(
    event: &serde_json::Map<String, Value>,
) -> Option<HashMap<String, Value>> {
    let entry_type = event
        .get("entry_type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let subtype = event
        .get("subtype")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if entry_type != "system" || subtype != "api_error" {
        return None;
    }

    let raw_entry = event
        .get("entry")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| event.clone());

    let retry_attempt = raw_entry.get("retryAttempt").and_then(|v| v.as_u64())? as i64;
    let max_retries = raw_entry.get("maxRetries").and_then(|v| v.as_u64())? as i64;
    if max_retries <= 0 || retry_attempt < max_retries {
        return None;
    }

    let cause = raw_entry.get("cause").and_then(Value::as_object).cloned();
    let error_code = cause
        .as_ref()
        .and_then(|c| c.get("code"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let error_path = cause
        .as_ref()
        .and_then(|c| c.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let mut out = HashMap::new();
    out.insert(
        "message".to_string(),
        Value::String(build_api_error_message(error_code, error_path)),
    );
    out.insert(
        "error_code".to_string(),
        error_code
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    out.insert(
        "error_path".to_string(),
        error_path
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    );
    out.insert(
        "retry_attempt".to_string(),
        Value::Number((retry_attempt.max(0) as u64).into()),
    );
    out.insert(
        "max_retries".to_string(),
        Value::Number((max_retries.max(0) as u64).into()),
    );
    out.insert(
        "timestamp".to_string(),
        raw_entry.get("timestamp").cloned().unwrap_or(Value::Null),
    );
    Some(out)
}

fn build_api_error_message(error_code: Option<&str>, error_path: Option<&str>) -> String {
    let mut parts = vec!["Claude API request failed".to_string()];
    if let Some(code) = error_code {
        parts.push(format!("code={}", code));
    }
    if let Some(path) = error_path {
        parts.push(format!("path={}", path));
    }
    parts.join(" ")
}

fn is_turn_boundary_event(
    event: &serde_json::Map<String, Value>,
    last_assistant_uuid: &str,
) -> bool {
    let entry_type = event
        .get("entry_type")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    let subtype = event
        .get("subtype")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if entry_type != "system" || subtype != "turn_duration" {
        return false;
    }
    if last_assistant_uuid.is_empty() {
        return false;
    }
    event
        .get("parent_uuid")
        .and_then(|v| v.as_str())
        .map(|s| s == last_assistant_uuid)
        .unwrap_or(false)
}

fn append_buffer(buffer: &str, text: &str) -> String {
    if buffer.is_empty() {
        text.to_string()
    } else {
        format!("{}\n{}", buffer, text)
    }
}

fn export_claude_runtime_state(submission: &ProviderSubmission) -> HashMap<String, Value> {
    let state = &submission.runtime_state;
    let mut out = HashMap::new();
    for key in [
        "mode",
        "state",
        "pane_id",
        "request_anchor",
        "next_seq",
        "anchor_seen",
        "no_wrap",
        "reply_buffer",
        "raw_buffer",
        "session_path",
        "last_assistant_uuid",
        "completion_dir",
        "prompt_text",
        "prompt_sent",
        "prompt_sent_at",
        "reply_delivery_complete_on_dispatch",
        "reply_delivery_require_ready",
        "ready_wait_started_at",
        "ready_timeout_s",
        "reader_state",
    ] {
        if let Some(value) = state.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Runtime state accessors
// ---------------------------------------------------------------------------

fn runtime_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn runtime_u64(state: &HashMap<String, Value>, key: &str) -> u64 {
    state.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn runtime_f64(state: &HashMap<String, Value>, key: &str, default: f64) -> f64 {
    state
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
        .max(0.0)
}

fn runtime_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn resolve_ready_timeout_s() -> f64 {
    std::env::var("CCB_CLAUDE_READY_TIMEOUT_S")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v.max(0.0))
        .unwrap_or(DEFAULT_READY_TIMEOUT_S)
}

// ---------------------------------------------------------------------------
// Protocol helpers
// ---------------------------------------------------------------------------

/// Wrap a Claude prompt with explicit begin/done markers.
/// Mirrors Python `provider_backends.claude.protocol_runtime.prompt.wrap_claude_prompt`.
pub fn wrap_claude_prompt(message: &str, req_id: &str) -> String {
    let body = build_prompt_body(message);
    format!(
        "{} {}\n\n{}Reply using exactly this format:\n{}{}>>\n<reply>\n{}{}>>\n",
        CLAUDE_REQ_ID_PREFIX, req_id, body, CLAUDE_BEGIN_PREFIX, req_id, CLAUDE_DONE_PREFIX, req_id
    )
}

/// Wrap a Claude turn prompt with only a request id marker.
/// Mirrors Python `provider_backends.claude.protocol_runtime.prompt.wrap_claude_turn_prompt`.
pub fn wrap_claude_turn_prompt(message: &str, req_id: &str) -> String {
    let body = build_prompt_body(message);
    format!("{} {}\n\n{}", CLAUDE_REQ_ID_PREFIX, req_id, body)
}

fn build_prompt_body(message: &str) -> String {
    let rendered = message.trim_end();
    let extras = prompt_extras(rendered);
    if extras.is_empty() {
        format!("{}\n\n", rendered)
    } else {
        format!("{}\n\n{}\n\n", rendered, extras)
    }
}

fn prompt_extras(message: &str) -> String {
    let mut lines = Vec::new();
    if wants_markdown_table(message) {
        lines.push("If asked for a Markdown table, output only pipe-and-dash Markdown table syntax (no box-drawing characters).");
    }
    if let Some(hint) = language_hint() {
        lines.push(hint);
    }
    lines.join("\n").trim().to_string()
}

fn wants_markdown_table(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("markdown") && (lower.contains("table") || message.contains("表格"))
}

fn language_hint() -> Option<&'static str> {
    let lang = std::env::var("CCB_REPLY_LANG")
        .or_else(|_| std::env::var("CCB_LANG"))
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    match lang.as_str() {
        "zh" | "cn" | "chinese" => Some("Reply in Chinese."),
        "en" | "english" => Some("Reply in English."),
        _ => None,
    }
}

/// Extract the reply for a request id from raw assistant text.
/// Mirrors Python `provider_backends.claude.protocol_runtime.reply.extract_reply_for_req`.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    protocol_extract_reply_for_req(text, req_id)
}

/// Strip done markers for a specific request id from assistant text.
fn strip_done_text_for_req(text: &str, req_id: &str) -> String {
    let done_marker = format!("{}{}>>", CLAUDE_DONE_PREFIX, req_id);
    text.replace(&done_marker, "").trim().to_string()
}

fn looks_ready(text: &str) -> bool {
    let normalized = text.trim();
    let lowered = normalized.to_lowercase();
    if normalized.lines().any(|line| {
        let stripped = line.trim_start();
        stripped.starts_with('❯') && stripped.chars().nth(1).is_none_or(char::is_whitespace)
    }) {
        return true;
    }
    if lowered.contains("type your message") || lowered.contains("esc to interrupt") {
        return true;
    }
    if lowered.contains("for shortcuts") {
        return true;
    }
    false
}

fn looks_like_claude_tui_chrome(text: &str) -> bool {
    let lowered = text.to_lowercase();
    lowered.contains("claude code")
        || lowered.contains("welcome back")
        || lowered.contains("api usage billing")
        || lowered.contains("esc to interrupt")
        || lowered.contains("type your message")
        || lowered.contains("for shortcuts")
        || (text.contains('╭') && text.contains('╮') && text.contains('╯'))
}

// ---------------------------------------------------------------------------
// Legacy pending_events test seam
// ---------------------------------------------------------------------------

fn process_pending_events(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    let events_value = submission.runtime_state.get("pending_events")?.clone();
    let events = events_value.as_array()?;
    if events.is_empty() {
        return None;
    }

    let mut poll = PollState::from_submission(submission);
    let mut items = Vec::new();

    for event in events {
        let obj = event.as_object()?;
        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("");
        match role {
            "user" => handle_user_event(submission, &mut poll, obj, now, &mut items),
            "system" => {
                if let Some(result) =
                    handle_system_event(submission, &mut poll, obj, now, &mut items)
                {
                    let mut updated = submission.clone();
                    updated.reply = poll.reply_buffer.clone();
                    updated.runtime_state = apply_poll_state(&updated.runtime_state, &poll);
                    updated
                        .runtime_state
                        .insert("pending_events".to_string(), Value::Array(Vec::new()));
                    return Some(merge_items(result, items));
                }
            }
            "assistant" if poll.anchor_seen => {
                handle_assistant_event(submission, &mut poll, obj, now, &mut items);
            }
            _ => {}
        }
        if poll.reached_turn_boundary {
            break;
        }
    }

    let mut updated = submission.clone();
    updated.reply = poll.reply_buffer.clone();
    updated.runtime_state = apply_poll_state(&updated.runtime_state, &poll);
    updated
        .runtime_state
        .insert("pending_events".to_string(), Value::Array(Vec::new()));

    if items.is_empty() {
        return None;
    }
    Some(ProviderPollResult::new(updated, items, None))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    use crate::execution::{with_prompt_target_override, PromptTarget, ProviderRuntimeContext};

    #[derive(Default, Clone)]
    struct MockTarget {
        sent: Arc<Mutex<Vec<(String, String)>>>,
        content: Arc<Mutex<String>>,
        fail_send: Arc<Mutex<bool>>,
    }

    impl PromptTarget for MockTarget {
        fn send_text(&self, pane_id: &str, text: &str) -> Result<(), String> {
            if *self.fail_send.lock().unwrap() {
                return Err("send failed".to_string());
            }
            self.sent
                .lock()
                .unwrap()
                .push((pane_id.to_string(), text.to_string()));
            Ok(())
        }

        fn get_pane_content(&self, _pane_id: &str, _lines: usize) -> Result<String, String> {
            Ok(self.content.lock().unwrap().clone())
        }
    }

    impl MockTarget {
        fn with_content(self, content: &str) -> Self {
            *self.content.lock().unwrap() = content.to_string();
            self
        }

        fn with_send_failure(self) -> Self {
            *self.fail_send.lock().unwrap() = true;
            self
        }
    }

    fn write_session(work_dir: &Path, extra: serde_json::Map<String, Value>) {
        let mut data = serde_json::Map::new();
        data.insert(
            "claude_session_path".to_string(),
            Value::String("/tmp/session.jsonl".to_string()),
        );
        data.insert("pane_id".to_string(), Value::String("%1".to_string()));
        data.extend(extra);
        std::fs::write(
            work_dir.join(".claude-session"),
            serde_json::to_string(&Value::Object(data)).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_manifest_has_profiles() {
        let m = manifest();
        assert_eq!(m.provider, "claude");
        assert!(m.supports_resume);
        assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
        assert!(m.supports_runtime_mode(&RuntimeMode::Headless));
    }

    #[test]
    fn test_backend_has_all_parts() {
        let b = backend();
        assert_eq!(b.provider(), "claude");
        assert!(b.session_binding.is_some());
        assert!(b.runtime_launcher.is_some());
    }

    #[test]
    fn test_wrap_claude_prompt() {
        let wrapped = wrap_claude_prompt("hello", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(wrapped.contains("<<BEGIN:req-12345678>>"));
        assert!(wrapped.contains("<<DONE:req-12345678>>"));
    }

    #[test]
    fn test_wrap_claude_turn_prompt() {
        let wrapped = wrap_claude_turn_prompt("hello", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(!wrapped.contains("<<DONE:req-12345678>>"));
    }

    #[test]
    fn test_extract_reply_for_req() {
        let text = "<<BEGIN:req-12345678>>\nhello world\n<<DONE:req-12345678>>";
        assert_eq!(extract_reply_for_req(text, "req-12345678"), "hello world");
    }

    #[test]
    fn test_start_active_submission() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        assert_eq!(sub.provider, "claude");
        assert_eq!(sub.runtime_state.get("mode").unwrap(), "active");
        assert!(!runtime_bool(&sub.runtime_state, "prompt_sent"));
    }

    #[test]
    fn test_start_fails_without_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let adapter = ClaudeExecutionAdapter;
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let ctx = ProviderRuntimeContext {
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        let sub = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
        assert_eq!(sub.runtime_state.get("mode").unwrap(), "error");
        assert!(runtime_str(&sub.runtime_state, "reason").contains("missing_claude_session"));
    }

    #[test]
    fn test_start_sends_prompt_when_ready() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_session(&work_dir, serde_json::Map::new());

        let target = Arc::new(MockTarget::default().with_content("❯ "));
        let sent = target.sent.clone();
        let sub = with_prompt_target_override(target, || {
            let adapter = ClaudeExecutionAdapter;
            let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });

        assert_eq!(sub.runtime_state.get("mode").unwrap(), "active");
        assert!(runtime_bool(&sub.runtime_state, "prompt_sent"));
        assert_eq!(sent.lock().unwrap().len(), 1);
        assert_eq!(sent.lock().unwrap()[0].0, "%1");
    }

    #[test]
    fn test_poll_dispatches_prompt() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        let result = poll_submission(&sub, "2025-01-01T00:00:01Z").unwrap();
        assert!(runtime_bool(
            &result.submission.runtime_state,
            "prompt_sent"
        ));
    }

    #[test]
    fn test_deferred_prompt_send_failure_stays_unsent() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub.runtime_state
            .insert("pane_id".to_string(), Value::String("%1".to_string()));
        sub.runtime_state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
        sub.runtime_state
            .insert("prompt_deferred_for_ready".to_string(), Value::Bool(true));

        let target = Arc::new(MockTarget::default().with_content("❯ ").with_send_failure());
        let result = with_prompt_target_override(target, || {
            dispatch_deferred_prompt_when_ready(&sub, "2025-01-01T00:00:01Z")
        })
        .expect("send failure should be surfaced as a poll update");

        assert!(!runtime_bool(
            &result.submission.runtime_state,
            "prompt_sent"
        ));
        assert!(runtime_str(&result.submission.runtime_state, "send_error").contains("send failed"));
    }

    #[test]
    fn test_poll_reply_delivery_completes() {
        let job = JobRecord::new("j1", "agent1", "claude")
            .with_request_body("do it")
            .with_request_message_type("reply_delivery");
        let sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        let dispatched = poll_submission(&sub, "2025-01-01T00:00:01Z").unwrap();
        let result = poll_submission(&dispatched.submission, "2025-01-01T00:00:02Z").unwrap();
        assert!(result.decision.as_ref().unwrap().terminal);
        assert_eq!(
            result.decision.as_ref().unwrap().status,
            CompletionStatus::Completed
        );
    }

    #[test]
    fn test_poll_events_produce_chunks() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let anchor = runtime_str(&sub.runtime_state, "request_anchor");
        let events = serde_json::json!([
            {"role": "user", "text": format!("{}\n\ndo it", anchor)},
            {"role": "assistant", "text": "chunk one"},
            {"role": "assistant", "text": "chunk two"}
        ]);
        sub.runtime_state
            .insert("pending_events".to_string(), events);

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        assert!(!result.items.is_empty());
        let chunks: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.kind == CompletionItemKind::AssistantChunk)
            .collect();
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn test_poll_hook_artifact_completes() {
        let tmp = TempDir::new().unwrap();
        let completion_dir = tmp.path().join("completion");
        let events_dir = completion_dir.join("events");
        std::fs::create_dir_all(&events_dir).unwrap();

        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let anchor = runtime_str(&sub.runtime_state, "request_anchor");
        sub.runtime_state.insert(
            "completion_dir".to_string(),
            Value::String(completion_dir.to_string_lossy().to_string()),
        );

        std::fs::write(
            events_dir.join(format!("{}.json", anchor)),
            serde_json::json!({
                "req_id": anchor,
                "status": "completed",
                "reply": "hook reply",
                "session_id": "session-1",
                "timestamp": "2025-01-01T00:00:05Z",
            })
            .to_string(),
        )
        .unwrap();

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        assert!(result.decision.as_ref().unwrap().terminal);
        assert_eq!(
            result.decision.as_ref().unwrap().status,
            CompletionStatus::Completed
        );
        assert_eq!(result.submission.reply, "hook reply");
    }

    #[test]
    fn test_pane_fallback_completes_only_when_pane_is_ready() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub.runtime_state
            .insert("prompt_sent".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("anchor_seen".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("pane_id".to_string(), Value::String("%1".to_string()));
        sub.runtime_state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
        sub.runtime_state.insert(
            "reply_buffer".to_string(),
            Value::String("final answer".to_string()),
        );

        let target = Arc::new(MockTarget::default().with_content("❯ "));
        let result = with_prompt_target_override(target, || {
            poll_pane_text_completion(&sub, "2025-01-01T00:00:02Z")
        })
        .expect("ready pane plus real reply should complete");

        assert_eq!(result.decision.unwrap().reply, "final answer");
    }

    #[test]
    fn test_pane_fallback_waits_when_structured_log_is_expected() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub.runtime_state
            .insert("prompt_sent".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("anchor_seen".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("pane_id".to_string(), Value::String("%1".to_string()));
        sub.runtime_state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
        sub.runtime_state.insert(
            "pane_text_buffer".to_string(),
            Value::String("Reply exactly: dirty\n✽ Ebbing…\n❯".to_string()),
        );
        sub.runtime_state.insert(
            "claude_projects_root".to_string(),
            Value::String("/tmp/.claude/projects".to_string()),
        );

        let target = Arc::new(MockTarget::default().with_content("❯ "));
        let result = with_prompt_target_override(target, || {
            poll_pane_text_completion(&sub, "2025-01-01T00:00:02Z")
        });

        assert!(result.is_none());
    }

    #[test]
    fn test_pane_fallback_rejects_claude_startup_chrome() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub.runtime_state
            .insert("prompt_sent".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("anchor_seen".to_string(), Value::Bool(true));
        sub.runtime_state
            .insert("pane_id".to_string(), Value::String("%1".to_string()));
        sub.runtime_state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
        sub.runtime_state.insert(
            "reply_buffer".to_string(),
            Value::String(
                "╭─ Claude Code ───────────────────────╮\n│ Welcome back! │\n╰─────────────────────────────────────╯"
                    .to_string(),
            ),
        );

        let target = Arc::new(MockTarget::default().with_content("❯ "));
        let result = with_prompt_target_override(target, || {
            poll_pane_text_completion(&sub, "2025-01-01T00:00:02Z")
        });

        assert!(
            result.is_none(),
            "startup chrome must not be delivered as an ask reply"
        );
    }

    #[test]
    fn test_poll_detects_stop_reason_end_turn() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let anchor = runtime_str(&sub.runtime_state, "request_anchor");
        let events = serde_json::json!([
            {"role": "user", "text": format!("{}\n\ndo it", anchor)},
            {"role": "assistant", "text": "final answer", "stop_reason": "end_turn", "uuid": "uuid-1"}
        ]);
        sub.runtime_state
            .insert("pending_events".to_string(), events);

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        let boundary = result
            .items
            .iter()
            .find(|i| i.kind == CompletionItemKind::TurnBoundary)
            .expect("turn boundary expected");
        assert_eq!(
            boundary.payload.get("reason").unwrap(),
            "assistant_end_turn"
        );
        assert_eq!(boundary.payload.get("stop_reason").unwrap(), "end_turn");
    }

    #[test]
    fn test_structured_reply_ignores_pane_text_buffer() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let anchor = runtime_str(&sub.runtime_state, "request_anchor");
        let events = serde_json::json!([
            {"role": "user", "text": format!("{}\n\ndo it", anchor)},
            {"role": "assistant", "text": "final answer", "stop_reason": "end_turn", "uuid": "uuid-1"}
        ]);
        sub.runtime_state
            .insert("pending_events".to_string(), events);
        sub.runtime_state.insert(
            "pane_text_buffer".to_string(),
            Value::String("Reply exactly: dirty\n✽ Combobulating…\n❯".to_string()),
        );

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        let boundary = result
            .items
            .iter()
            .find(|i| i.kind == CompletionItemKind::TurnBoundary)
            .expect("turn boundary expected");
        assert_eq!(
            boundary.payload.get("last_agent_message").unwrap(),
            "final answer"
        );
        assert_eq!(result.submission.reply, "final answer");
    }

    #[test]
    fn test_poll_accepts_claude_req_id_anchor() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let anchor = runtime_str(&sub.runtime_state, "request_anchor");
        let req_id = req_id_from_request_anchor(&anchor).expect("request id");
        let events = serde_json::json!([
            {"role": "user", "text": format!("{} {}\n\ndo it", CLAUDE_REQ_ID_PREFIX, req_id)},
            {"role": "assistant", "text": "final answer", "stop_reason": "end_turn", "uuid": "uuid-1"}
        ]);
        sub.runtime_state
            .insert("pending_events".to_string(), events);

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        let boundary = result
            .items
            .iter()
            .find(|i| i.kind == CompletionItemKind::TurnBoundary)
            .expect("turn boundary expected");
        assert_eq!(
            boundary.payload.get("last_agent_message").unwrap(),
            "final answer"
        );
    }

    #[test]
    fn test_api_error_terminalizes() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let mut sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        sub = poll_submission(&sub, "2025-01-01T00:00:01Z")
            .unwrap()
            .submission;

        let events = serde_json::json!([
            {
                "role": "system",
                "entry_type": "system",
                "subtype": "api_error",
                "retryAttempt": 3,
                "maxRetries": 3,
                "cause": {"code": "overloaded", "path": "/v1/messages"},
                "timestamp": "2025-01-01T00:00:05Z"
            }
        ]);
        sub.runtime_state
            .insert("pending_events".to_string(), events);

        let result = poll_submission(&sub, "2025-01-01T00:00:02Z").unwrap();
        assert!(result.decision.as_ref().unwrap().terminal);
        assert_eq!(
            result.decision.as_ref().unwrap().status,
            CompletionStatus::Failed
        );
        assert_eq!(
            result.decision.as_ref().unwrap().reason.as_deref(),
            Some("api_error")
        );
    }

    #[test]
    fn test_export_runtime_state() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        let exported = export_claude_runtime_state(&sub);
        assert!(exported.contains_key("mode"));
        assert!(exported.contains_key("request_anchor"));
    }
}
