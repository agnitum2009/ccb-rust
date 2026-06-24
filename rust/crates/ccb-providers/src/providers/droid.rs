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
use serde_json::Value;

use crate::droid::{
    extract_reply_for_req, is_done_text, load_project_session, managed_droid_home_for_runtime,
    strip_done_text, wrap_droid_prompt, DroidLogReader, LogEvent,
};
use crate::execution::{
    no_wrap_requested, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext,
    ProviderSubmission,
};

pub const PROVIDER_NAME: &str = "droid";

const DROID_SESSION_ID_ATTR: &str = "droid_session_id";
const DROID_SESSION_PATH_ATTR: &str = "droid_session_path";

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// Build the Droid provider manifest.
///
/// Mirrors Python `provider_backends.droid.manifest.build_manifest`.
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
        provider, false, // supports_resume
        false, // supports_permission_auto
        false, // supports_stream_watch
        false, // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// Build a complete Droid provider backend registration.
///
/// Mirrors Python `provider_backends.droid.build_backend`.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(ProviderSessionBinding {
            provider: PROVIDER_NAME.to_string(),
            session_id_attr: DROID_SESSION_ID_ATTR.to_string(),
            session_path_attr: DROID_SESSION_PATH_ATTR.to_string(),
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

/// Droid provider execution adapter.
///
/// Mirrors Python `provider_backends.droid.execution.DroidProviderAdapter`.
#[derive(Debug, Clone)]
pub struct DroidExecutionAdapter;

impl ExecutionAdapter for DroidExecutionAdapter {
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
        let mut state = submission.runtime_state.clone();
        state.insert("mode".to_string(), Value::String("active".to_string()));
        Some(state)
    }
}

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let workspace_path = context
        .and_then(|c| c.workspace_path.as_deref())
        .map(expand_tilde)
        .unwrap_or_default();

    let no_wrap = no_wrap_requested(job.provider_options.get("no_wrap"));
    let request_anchor = job.job_id.clone();

    // Resolve a session path. Prefer an explicit session_ref from the runtime
    // context, then fall back to a path derived from the loaded project session,
    // and finally leave it empty for the poll loop to discover dynamically.
    let session_path = context
        .and_then(|c| c.session_ref.as_deref())
        .map(|s| expand_tilde(s).to_string())
        .or_else(|| {
            if workspace_path.is_empty() {
                return None;
            }
            load_project_session(Path::new(&workspace_path), None)
                .and_then(|s| s.droid_session_path().map(|p| p.to_string()))
        })
        .unwrap_or_default();

    let prompt = if no_wrap {
        job.request.body.clone()
    } else {
        wrap_droid_prompt(&job.request.body, &request_anchor)
    };

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_seen".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("raw_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("session_path".to_string(), Value::String(session_path));
    runtime_state.insert("workspace_path".to_string(), Value::String(workspace_path));
    runtime_state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("prompt".to_string(), Value::String(prompt));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(false));

    let diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "active",
        "resume_supported": false,
    });

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: PROVIDER_NAME.to_string(),
        accepted_at: now.to_string(),
        ready_at: now.to_string(),
        source_kind: CompletionSourceKind::TerminalText,
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

    let request_anchor = submission
        .runtime_state
        .get("request_anchor")
        .and_then(Value::as_str)
        .unwrap_or(&submission.job_id)
        .to_string();

    let workspace_path = runtime_str(&submission.runtime_state, "workspace_path");
    let session_path = runtime_str(&submission.runtime_state, "session_path");
    if session_path.is_empty() {
        return None;
    }

    let work_dir = if workspace_path.is_empty() {
        PathBuf::from(&session_path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(&workspace_path)
    };

    let mut reader = DroidLogReader::new(None, Some(&work_dir));
    reader.set_preferred_session(Some(PathBuf::from(&session_path)));
    if let Some(hint) = submission
        .runtime_state
        .get("droid_session_id")
        .and_then(Value::as_str)
    {
        reader.set_session_id_hint(Some(hint.to_string()));
    }

    let state = submission
        .runtime_state
        .get("state")
        .cloned()
        .and_then(|v| serde_json::from_value::<HashMap<String, Value>>(v).ok())
        .unwrap_or_else(|| reader.capture_state());

    let (events, new_state) = reader.try_get_events(&state);
    if events.is_empty() {
        return None;
    }

    let mut next_seq = submission
        .runtime_state
        .get("next_seq")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let mut anchor_seen = runtime_bool(&submission.runtime_state, "anchor_seen");
    let mut raw_buffer = runtime_str(&submission.runtime_state, "raw_buffer");
    let mut reply_buffer = runtime_str(&submission.runtime_state, "reply_buffer");
    let mut done_seen = false;
    let mut items = Vec::new();

    for event in events {
        match event {
            LogEvent::User(text) => {
                if !anchor_seen && text.contains(&format!("CCB_REQ_ID: {}", request_anchor)) {
                    anchor_seen = true;
                    if let Ok(item) = CompletionItem::new(
                        CompletionItemKind::AnchorSeen,
                        now.to_string(),
                        CompletionCursor {
                            source_kind: submission.source_kind,
                            event_seq: Some(next_seq),
                            updated_at: Some(now.to_string()),
                            session_path: Some(session_path.clone()),
                            ..Default::default()
                        },
                        &submission.provider,
                        &submission.agent_name,
                        &submission.job_id,
                    ) {
                        let mut item = item;
                        item.payload
                            .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                        item.payload.insert(
                            "session_path".to_string(),
                            Value::String(session_path.clone()),
                        );
                        items.push(item);
                    }
                    next_seq += 1;
                }
            }
            LogEvent::Assistant(text) => {
                if !anchor_seen {
                    continue;
                }
                raw_buffer = merged_reply_text(&raw_buffer, &text);
                done_seen = is_done_text(&raw_buffer, &request_anchor);
                let cleaned = clean_reply(&raw_buffer, &request_anchor);
                if cleaned.is_empty() {
                    continue;
                }
                reply_buffer = cleaned.clone();

                let kind = if done_seen {
                    CompletionItemKind::AssistantFinal
                } else {
                    CompletionItemKind::AssistantChunk
                };
                if let Ok(item) = CompletionItem::new(
                    kind,
                    now.to_string(),
                    CompletionCursor {
                        source_kind: submission.source_kind,
                        event_seq: Some(next_seq),
                        updated_at: Some(now.to_string()),
                        session_path: Some(session_path.clone()),
                        ..Default::default()
                    },
                    &submission.provider,
                    &submission.agent_name,
                    &submission.job_id,
                ) {
                    let mut item = item;
                    item.payload
                        .insert("text".to_string(), Value::String(cleaned.clone()));
                    item.payload
                        .insert("reply".to_string(), Value::String(cleaned.clone()));
                    item.payload
                        .insert("merged_text".to_string(), Value::String(cleaned.clone()));
                    item.payload
                        .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                    item.payload.insert(
                        "session_path".to_string(),
                        Value::String(session_path.clone()),
                    );
                    item.payload
                        .insert("done_marker".to_string(), Value::Bool(done_seen));
                    item.payload
                        .insert("ccb_done".to_string(), Value::Bool(done_seen));
                    items.push(item);
                }
                next_seq += 1;

                if done_seen {
                    break;
                }
            }
        }
    }

    if items.is_empty() {
        return None;
    }

    let mut updated = submission.clone();
    updated.reply = reply_buffer.clone();
    updated.runtime_state.insert(
        "state".to_string(),
        Value::Object(new_state.into_iter().collect()),
    );
    updated
        .runtime_state
        .insert("next_seq".to_string(), Value::Number(next_seq.into()));
    updated
        .runtime_state
        .insert("anchor_seen".to_string(), Value::Bool(anchor_seen));
    updated.runtime_state.insert(
        "reply_buffer".to_string(),
        Value::String(reply_buffer.clone()),
    );
    updated
        .runtime_state
        .insert("raw_buffer".to_string(), Value::String(raw_buffer));
    updated
        .runtime_state
        .insert("done_seen".to_string(), Value::Bool(done_seen));

    let decision = if done_seen {
        let cursor = items.last().map(|i| i.cursor.clone());
        Some(CompletionDecision {
            terminal: true,
            status: CompletionStatus::Completed,
            reason: Some("done".to_string()),
            confidence: Some(CompletionConfidence::Exact),
            reply: reply_buffer,
            anchor_seen: true,
            reply_started: true,
            reply_stable: true,
            provider_turn_ref: Some(request_anchor.clone()),
            source_cursor: cursor,
            finished_at: Some(now.to_string()),
            diagnostics: Default::default(),
        })
    } else {
        None
    };

    Some(ProviderPollResult::new(updated, items, decision))
}

fn clean_reply(raw_buffer: &str, request_anchor: &str) -> String {
    let cleaned = extract_reply_for_req(raw_buffer, request_anchor);
    if cleaned.is_empty() {
        strip_done_text(raw_buffer, request_anchor)
    } else {
        cleaned
    }
}

fn merged_reply_text(raw_buffer: &str, text: &str) -> String {
    if raw_buffer.is_empty() {
        text.to_string()
    } else {
        format!("{}\n{}", raw_buffer, text)
    }
}

fn runtime_str(runtime_state: &HashMap<String, Value>, key: &str) -> String {
    runtime_state
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn runtime_bool(runtime_state: &HashMap<String, Value>, key: &str) -> bool {
    runtime_state
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    input.to_string()
}

// ---------------------------------------------------------------------------
// Launcher helpers
// ---------------------------------------------------------------------------

/// Build a Droid runtime launcher descriptor.
///
/// Mirrors Python `provider_backends.droid.launcher.build_runtime_launcher`.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::SimpleTmux)
}

/// Compute the managed Droid home directory for a runtime directory.
///
/// Re-exported from the provider helper module.
pub fn managed_droid_home(runtime_dir: &Path) -> PathBuf {
    managed_droid_home_for_runtime(runtime_dir)
}
