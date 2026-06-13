use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::protocol::{
    extract_reply_for_req as protocol_extract_reply_for_req, is_done_text, make_req_id,
    request_anchor_for_job,
};
use serde_json::Value;

use crate::execution::common::{build_item, no_wrap_requested, request_anchor_from_runtime_state};
use crate::execution::{
    ExecutionAdapter, PersistedExecutionState, ProviderPollResult, ProviderRuntimeContext,
    ProviderSubmission,
};

pub const PROVIDER_NAME: &str = "claude";

const CLAUDE_REQ_ID_PREFIX: &str = "CCB_REQ_ID:";
const CLAUDE_BEGIN_PREFIX: &str = "<<BEGIN:";
const CLAUDE_DONE_PREFIX: &str = "<<DONE:";

const CLAUDE_SESSION_FILENAME: &str = ".claude-session";
const SESSION_ID_ATTR: &str = "claude_session_id";
const SESSION_PATH_ATTR: &str = "claude_session_path";
const DEFAULT_READY_TIMEOUT_S: f64 = 8.0;

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
        },
    );
    profiles.insert(
        RuntimeMode::Headless,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "headless".to_string(),
            poll_interval_ms: 500,
            timeout_ms: 300_000,
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
            session_id_attr: SESSION_ID_ATTR.to_string(),
            session_path_attr: SESSION_PATH_ATTR.to_string(),
        }),
        runtime_launcher: Some(ProviderRuntimeLauncher {
            provider: PROVIDER_NAME.to_string(),
            launch_mode: LaunchMode::SimpleTmux,
        }),
    }
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
        _persisted_state: &PersistedExecutionState,
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
}

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
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

    let (session_path, completion_dir) = context
        .and_then(|c| c.workspace_path.as_ref())
        .and_then(|ws| load_project_session(Path::new(ws), None))
        .map(|session| {
            let path = session.claude_session_path();
            let dir = session.completion_dir();
            (path, dir)
        })
        .unwrap_or_default();

    let prompt = if no_wrap {
        job.request.body.clone()
    } else if completion_dir.is_empty() {
        wrap_claude_prompt(&job.request.body, &make_req_id(&job.job_id))
    } else {
        wrap_claude_turn_prompt(&job.request.body, &make_req_id(&job.job_id))
    };

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_seen".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("raw_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("session_path".to_string(), Value::String(session_path));
    runtime_state.insert("completion_dir".to_string(), Value::String(completion_dir));
    runtime_state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("prompt_text".to_string(), Value::String(prompt));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(false));
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

    let diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "active",
        "workspace_path": context.and_then(|c| c.workspace_path.clone()).unwrap_or_default(),
    });

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
    if !runtime_bool(&submission.runtime_state, "prompt_sent") {
        return Some(dispatch_deferred_prompt(submission, now));
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

    if let Some(result) = process_pending_events(submission, now) {
        return Some(result);
    }

    None
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
        if let Ok(mut item) = CompletionItem::new(
            CompletionItemKind::AnchorSeen,
            now.to_string(),
            CompletionCursor {
                source_kind: updated.source_kind,
                event_seq: Some(next_seq),
                updated_at: Some(now.to_string()),
                session_path: session_path_opt.clone(),
                ..Default::default()
            },
            &updated.provider,
            &updated.agent_name,
            &updated.job_id,
        ) {
            item.payload
                .insert("turn_id".to_string(), Value::String(request_anchor));
            item.payload.insert(
                "session_path".to_string(),
                session_path_opt.map(Value::String).unwrap_or(Value::Null),
            );
            items.push(item);
        }
    }

    ProviderPollResult::new(updated, items, None)
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
        diagnostics: serde_json::Map::from_iter([
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
        ]),
    };

    let mut updated = submission.clone();
    updated.reply =
        request_anchor_from_runtime_state(&submission.runtime_state, &submission.job_id);
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
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
    let status = match status_str.to_lowercase().as_str() {
        "failed" => CompletionStatus::Failed,
        "cancelled" => CompletionStatus::Cancelled,
        "incomplete" => CompletionStatus::Incomplete,
        _ => CompletionStatus::Completed,
    };
    let diagnostics = obj
        .get("diagnostics")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let diagnostics_map = diagnostics.as_object().cloned().unwrap_or_default();
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

    let item = match CompletionItem::new(
        CompletionItemKind::AssistantFinal,
        timestamp.to_string(),
        cursor,
        &submission.provider,
        &submission.agent_name,
        &submission.job_id,
    ) {
        Ok(mut item) => {
            let mut payload = serde_json::Map::new();
            payload.insert("reply".to_string(), Value::String(reply.clone()));
            payload.insert("text".to_string(), Value::String(reply.clone()));
            payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
            payload.insert(
                "provider_turn_ref".to_string(),
                Value::String(provider_turn_ref.clone()),
            );
            payload.insert(
                "completion_source".to_string(),
                Value::String("hook_artifact".to_string()),
            );
            payload.insert(
                "hook_event_name".to_string(),
                obj.get("hook_event_name").cloned().unwrap_or(Value::Null),
            );
            payload.insert("status".to_string(), Value::String(status_str.to_string()));
            for (k, v) in &diagnostics_map {
                if !payload.contains_key(k) {
                    payload.insert(k.clone(), v.clone());
                }
            }
            item.payload = payload;
            item
        }
        Err(_) => return None,
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

fn merge_items(
    result: ProviderPollResult,
    mut prefix_items: Vec<CompletionItem>,
) -> ProviderPollResult {
    prefix_items.extend(result.items);
    ProviderPollResult::new(result.submission, prefix_items, result.decision)
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
    if !poll.request_anchor.is_empty() && text.contains(&poll.request_anchor) && !poll.anchor_seen {
        if let Ok(mut item) = CompletionItem::new(
            CompletionItemKind::AnchorSeen,
            now.to_string(),
            CompletionCursor {
                source_kind: submission.source_kind,
                event_seq: Some(poll.next_seq),
                updated_at: Some(now.to_string()),
                session_path: if poll.session_path.is_empty() {
                    None
                } else {
                    Some(poll.session_path.clone())
                },
                ..Default::default()
            },
            &submission.provider,
            &submission.agent_name,
            &submission.job_id,
        ) {
            item.payload.insert(
                "turn_id".to_string(),
                Value::String(poll.request_anchor.clone()),
            );
            items.push(item);
            poll.next_seq += 1;
            poll.anchor_seen = true;
        }
    }
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

        let item = build_item(
            submission,
            CompletionItemKind::Error,
            &timestamp,
            poll.next_seq,
            payload.into_iter().collect(),
        );
        items.push(item);
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
        let mut payload = serde_json::Map::new();
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
        let item = build_item(
            submission,
            CompletionItemKind::TurnBoundary,
            now,
            poll.next_seq,
            payload.into_iter().collect(),
        );
        items.push(item);
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

    let mut payload = serde_json::Map::new();
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

    let item = build_item(
        submission,
        CompletionItemKind::AssistantChunk,
        now,
        poll.next_seq,
        payload.into_iter().collect(),
    );
    items.push(item);
    poll.next_seq += 1;

    maybe_append_turn_boundary(submission, poll, now, items);
}

fn maybe_append_turn_boundary(
    submission: &ProviderSubmission,
    poll: &mut PollState,
    now: &str,
    items: &mut Vec<CompletionItem>,
) {
    if poll.request_anchor.is_empty() || !is_done_text(&poll.raw_buffer) {
        return;
    }
    let reply = protocol_extract_reply_for_req(&poll.raw_buffer, &poll.request_anchor);
    let reply = if reply.is_empty() {
        poll.reply_buffer.clone()
    } else {
        reply
    };

    let mut payload = serde_json::Map::new();
    payload.insert(
        "reason".to_string(),
        Value::String("task_complete".to_string()),
    );
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

    let item = build_item(
        submission,
        CompletionItemKind::TurnBoundary,
        now,
        poll.next_seq,
        payload.into_iter().collect(),
    );
    items.push(item);
    poll.next_seq += 1;
    poll.reached_turn_boundary = true;
}

fn terminal_api_error_payload(
    event: &serde_json::Map<String, Value>,
) -> Option<HashMap<String, Value>> {
    let error = event.get("error")?;
    let obj = error.as_object()?;
    let mut out = HashMap::new();
    out.insert(
        "error_code".to_string(),
        obj.get("code").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "error_path".to_string(),
        obj.get("path").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "retry_attempt".to_string(),
        obj.get("retry_attempt").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "max_retries".to_string(),
        obj.get("max_retries").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "timestamp".to_string(),
        obj.get("timestamp").cloned().unwrap_or(Value::Null),
    );
    Some(out)
}

fn is_turn_boundary_event(
    event: &serde_json::Map<String, Value>,
    last_assistant_uuid: &str,
) -> bool {
    event
        .get("turn_boundary")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && event
            .get("assistant_uuid")
            .and_then(|v| v.as_str())
            .map(|uuid| uuid == last_assistant_uuid)
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

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded Claude project session.
/// Mirrors Python `provider_backends.claude.session_runtime.model.ClaudeProjectSession`.
#[derive(Debug, Clone, Default)]
pub struct ClaudeProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl ClaudeProjectSession {
    pub fn claude_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn claude_session_path(&self) -> String {
        self.data
            .get(SESSION_PATH_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn completion_dir(&self) -> String {
        self.data
            .get("completion_artifact_dir")
            .and_then(|v| v.as_str())
            .or_else(|| self.data.get("runtime_dir").and_then(|v| v.as_str()))
            .map(|s| {
                PathBuf::from(s)
                    .join("completion")
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default()
    }
}

/// Find a project session file for a work directory.
/// Mirrors Python `provider_backends.claude.session_runtime.pathing.find_project_session_file`.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(CLAUDE_SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load a Claude project session.
/// Mirrors Python `provider_backends.claude.session.load_project_session`.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<ClaudeProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(ClaudeProjectSession { session_file, data })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_export_runtime_state() {
        let job = JobRecord::new("j1", "agent1", "claude").with_request_body("do it");
        let sub = start_active_submission(&job, None, "2025-01-01T00:00:00Z");
        let exported = export_claude_runtime_state(&sub);
        assert!(exported.contains_key("mode"));
        assert!(exported.contains_key("request_anchor"));
    }
}
