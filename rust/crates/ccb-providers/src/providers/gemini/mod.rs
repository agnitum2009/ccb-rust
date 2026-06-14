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
use ccb_provider_core::pathing::find_session_file_for_work_dir;
use ccb_provider_core::protocol;
use serde_json::Value;

use crate::execution::target::resolve_prompt_target;
use crate::execution::{
    build_item, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};

pub mod log_reader;

pub const PROVIDER_NAME: &str = "gemini";
pub const GEMINI_SESSION_ID_ATTR: &str = "gemini_session_id";
pub const GEMINI_SESSION_PATH_ATTR: &str = "gemini_session_path";

const GEMINI_SESSION_FILENAME: &str = ".gemini-session";

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;

/// Build the Gemini provider manifest.
///
/// Mirrors the Python `provider_backends.gemini.manifest.build_manifest`.
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
        },
    );
    profiles.insert(
        RuntimeMode::Headless,
        CompletionManifest {
            provider: provider.clone(),
            runtime_mode: "headless".to_string(),
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        },
    );
    ProviderManifest::new(
        provider, true,  // supports_resume
        true,  // supports_permission_auto
        true,  // supports_stream_watch
        false, // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build the full Gemini provider backend registration.
///
/// Mirrors the Python `provider_backends.gemini` bundle of manifest, execution
/// adapter, session binding and runtime launcher.
pub fn backend() -> ProviderBackend {
    let mut session_binding = ProviderSessionBinding::new(PROVIDER_NAME);
    session_binding.session_id_attr = GEMINI_SESSION_ID_ATTR.to_string();
    session_binding.session_path_attr = GEMINI_SESSION_PATH_ATTR.to_string();

    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(session_binding),
        runtime_launcher: Some(ProviderRuntimeLauncher::new(
            PROVIDER_NAME,
            LaunchMode::SimpleTmux,
        )),
    }
}

pub struct GeminiExecutionAdapter;

impl ExecutionAdapter for GeminiExecutionAdapter {
    fn provider(&self) -> &str {
        PROVIDER_NAME
    }

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        let request_anchor = request_anchor(&job.job_id);
        let req_id = make_req_id(&job.job_id);
        let prompt_text = wrap_gemini_prompt(&job.job_id, &req_id);

        let (session_data, work_dir) = context
            .and_then(|ctx| {
                let path = ctx.workspace_path.as_deref()?;
                if path.trim().is_empty() {
                    return None;
                }
                let work_dir = expand_path(path);
                load_project_session(&work_dir)
                    .map(|data| (data, work_dir.clone()))
                    .or_else(|| Some((HashMap::new(), work_dir)))
            })
            .unwrap_or_default();

        let layout = gemini_layout_from_session_data(Some(&session_data));
        let root = layout
            .as_ref()
            .map(|l| l.tmp_root.clone())
            .unwrap_or_else(current_gemini_tmp_root);
        let preferred_session = context
            .and_then(|ctx| ctx.session_ref.as_deref())
            .map(PathBuf::from)
            .or_else(|| {
                session_data
                    .get(GEMINI_SESSION_PATH_ATTR)
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
            })
            .filter(|p| p.is_file());

        let reader_state =
            log_reader::capture_reader_state(&root, &work_dir, preferred_session.as_deref());
        let session_path = reader_state
            .session_path
            .as_deref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut runtime_state = HashMap::new();
        runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
        runtime_state.insert(
            "request_anchor".to_string(),
            Value::String(request_anchor.clone()),
        );
        runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
        runtime_state.insert("anchor_emitted".to_string(), Value::Bool(false));
        runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
        runtime_state.insert("session_path".to_string(), Value::String(session_path));
        runtime_state.insert("completion_dir".to_string(), Value::String(String::new()));
        runtime_state.insert("no_wrap".to_string(), Value::Bool(false));
        runtime_state.insert("prompt_text".to_string(), Value::String(prompt_text));
        runtime_state.insert("prompt_sent".to_string(), Value::Bool(false));
        runtime_state.insert(
            "ready_wait_started_at".to_string(),
            Value::String(now.to_string()),
        );
        runtime_state.insert("ready_timeout_s".to_string(), Value::Number(20.into()));
        runtime_state.insert(
            "reader_state".to_string(),
            serde_json::to_value(&reader_state).unwrap_or(Value::Null),
        );
        runtime_state.insert(
            "gemini_root".to_string(),
            Value::String(root.to_string_lossy().to_string()),
        );
        if let Some(layout) = layout {
            runtime_state.insert(
                "gemini_home".to_string(),
                Value::String(layout.home_root.to_string_lossy().to_string()),
            );
        }
        if let Some(pane_id) = session_data.get("pane_id").and_then(Value::as_str) {
            runtime_state.insert("pane_id".to_string(), Value::String(pane_id.to_string()));
        }
        store_backend_config_from_session_data(&mut runtime_state, &session_data);

        let diagnostics = serde_json::json!({
            "provider": PROVIDER_NAME,
            "mode": "active",
            "request_anchor": request_anchor,
            "workspace_path": work_dir.to_string_lossy().to_string(),
        });

        ProviderSubmission {
            job_id: job.job_id.clone(),
            agent_name: job.agent_name.clone(),
            provider: PROVIDER_NAME.to_string(),
            accepted_at: now.to_string(),
            ready_at: now.to_string(),
            source_kind: CompletionSourceKind::SessionSnapshot,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: CompletionConfidence::Observed,
            diagnostics: Some(diagnostics),
            runtime_state,
        }
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        if submission.is_terminal() {
            return None;
        }

        let request_anchor = submission
            .runtime_state
            .get("request_anchor")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_else(|| request_anchor(&submission.job_id));
        let req_id = make_req_id(&submission.job_id);

        // Legacy fallback: if the reply buffer already contains a done marker,
        // extract the reply and complete immediately.
        let reply_buffer = submission
            .runtime_state
            .get("reply_buffer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if is_done_text(&reply_buffer) {
            return terminal_from_reply_buffer(
                submission,
                now,
                &request_anchor,
                &req_id,
                &reply_buffer,
            );
        }

        // Dispatch the prompt to the pane if it has not been sent yet.
        let pane_id = submission
            .runtime_state
            .get("pane_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !state_bool(&submission.runtime_state, "prompt_sent") && !pane_id.is_empty() {
            if let Some(err) = dispatch_prompt(&submission.runtime_state, &pane_id) {
                let mut state = submission.runtime_state.clone();
                state.insert("send_error".to_string(), Value::String(err.clone()));
                return terminal_from_send_error(submission, &mut state, now, &err);
            }
            let mut state = submission.runtime_state.clone();
            state.insert("prompt_sent".to_string(), Value::Bool(true));
            state.insert("prompt_sent_at".to_string(), Value::String(now.to_string()));
            let updated = ProviderSubmission {
                runtime_state: state,
                ..submission.clone()
            };
            return Some(ProviderPollResult::new(updated, Vec::new(), None));
        }

        // Native session snapshot reading.
        if let Some(reader_state) = reader_state_from_runtime_state(&submission.runtime_state) {
            let (reply, new_state) = log_reader::try_get_message(&reader_state);
            let mut state = submission.runtime_state.clone();
            state.insert(
                "reader_state".to_string(),
                serde_json::to_value(&new_state).unwrap_or(Value::Null),
            );
            state.insert(
                "session_path".to_string(),
                new_state
                    .session_path
                    .as_deref()
                    .map(|p| Value::String(p.to_string_lossy().to_string()))
                    .unwrap_or(Value::String(String::new())),
            );

            if let Some(reply) = reply {
                let mut items = Vec::new();
                let next_seq = state.get("next_seq").and_then(Value::as_u64).unwrap_or(1);
                let anchor_emitted = state
                    .get("anchor_emitted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);

                if !anchor_emitted {
                    let mut anchor_payload = HashMap::new();
                    anchor_payload
                        .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                    if let Some(path) = new_state.session_path.as_deref() {
                        anchor_payload.insert(
                            "session_path".to_string(),
                            Value::String(path.to_string_lossy().to_string()),
                        );
                    }
                    items.push(build_item(
                        submission,
                        CompletionItemKind::AnchorSeen,
                        now,
                        next_seq,
                        anchor_payload,
                    ));
                    state.insert("anchor_emitted".to_string(), Value::Bool(true));
                    state.insert("next_seq".to_string(), Value::Number((next_seq + 1).into()));
                }

                let done_seen = is_done_text(&reply);
                let cleaned = if done_seen {
                    extract_reply_for_req(&reply, &req_id)
                } else {
                    reply.clone()
                };

                let mut snapshot_payload = HashMap::new();
                snapshot_payload.insert("reply".to_string(), Value::String(cleaned.clone()));
                snapshot_payload.insert("text".to_string(), Value::String(cleaned.clone()));
                snapshot_payload.insert("content".to_string(), Value::String(cleaned.clone()));
                snapshot_payload
                    .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                snapshot_payload.insert("done_marker_seen".to_string(), Value::Bool(done_seen));
                if let Some(path) = new_state.session_path.as_deref() {
                    snapshot_payload.insert(
                        "session_path".to_string(),
                        Value::String(path.to_string_lossy().to_string()),
                    );
                }
                snapshot_payload.insert(
                    "message_count".to_string(),
                    Value::Number(new_state.msg_count.into()),
                );
                if let Some(id) = &new_state.last_gemini_id {
                    snapshot_payload.insert("message_id".to_string(), Value::String(id.clone()));
                }
                snapshot_payload.insert(
                    "tool_call_count".to_string(),
                    Value::Number(new_state.last_tool_call_count.into()),
                );
                snapshot_payload.insert(
                    "thought_count".to_string(),
                    Value::Number(new_state.last_thought_count.into()),
                );

                let next_seq = state.get("next_seq").and_then(Value::as_u64).unwrap_or(1);
                items.push(build_item(
                    submission,
                    CompletionItemKind::SessionSnapshot,
                    now,
                    next_seq,
                    snapshot_payload,
                ));
                state.insert("next_seq".to_string(), Value::Number((next_seq + 1).into()));
                state.insert("reply_buffer".to_string(), Value::String(cleaned.clone()));

                if done_seen && !cleaned.is_empty() {
                    let mut updated = submission.clone();
                    updated.reply = cleaned.clone();
                    updated.status = CompletionStatus::Completed;
                    updated.reason = "done".to_string();
                    updated.confidence = CompletionConfidence::Exact;
                    updated.runtime_state = state.clone();
                    updated
                        .runtime_state
                        .insert("anchor_emitted".to_string(), Value::Bool(true));
                    updated
                        .runtime_state
                        .insert("reply_buffer".to_string(), Value::String(cleaned.clone()));

                    let source_cursor = items.last().map(|item| item.cursor.clone());
                    let decision = CompletionDecision {
                        terminal: true,
                        status: CompletionStatus::Completed,
                        reason: Some("done".to_string()),
                        confidence: Some(CompletionConfidence::Exact),
                        reply: cleaned,
                        anchor_seen: true,
                        reply_started: true,
                        reply_stable: true,
                        provider_turn_ref: Some(request_anchor.clone()),
                        source_cursor,
                        finished_at: Some(now.to_string()),
                        diagnostics: Default::default(),
                    };
                    return Some(ProviderPollResult::new(updated, items, Some(decision)));
                }

                let updated = ProviderSubmission {
                    runtime_state: state.clone(),
                    reply: cleaned,
                    ..submission.clone()
                };
                return Some(ProviderPollResult::new(updated, items, None));
            }

            // No new reply but state may have changed (e.g., session path updated).
            if state != submission.runtime_state {
                let updated = ProviderSubmission {
                    runtime_state: state,
                    ..submission.clone()
                };
                return Some(ProviderPollResult::new(updated, Vec::new(), None));
            }
        }

        None
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

fn terminal_from_reply_buffer(
    submission: &ProviderSubmission,
    now: &str,
    request_anchor: &str,
    req_id: &str,
    reply_buffer: &str,
) -> Option<ProviderPollResult> {
    let cleaned = extract_reply_for_req(reply_buffer, req_id);
    let next_seq = submission
        .runtime_state
        .get("next_seq")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    let mut payload = HashMap::new();
    payload.insert("reply".to_string(), Value::String(cleaned.clone()));
    payload.insert("text".to_string(), Value::String(cleaned.clone()));
    payload.insert("content".to_string(), Value::String(cleaned.clone()));
    payload.insert(
        "turn_id".to_string(),
        Value::String(request_anchor.to_string()),
    );
    payload.insert(
        "done_marker_seen".to_string(),
        Value::Bool(is_done_text(reply_buffer)),
    );

    let item = build_item(
        submission,
        CompletionItemKind::SessionSnapshot,
        now,
        next_seq,
        payload,
    );

    let mut updated = submission.clone();
    updated.reply = cleaned.clone();
    updated.status = CompletionStatus::Completed;
    updated.reason = "done".to_string();
    updated.confidence = CompletionConfidence::Exact;
    updated
        .runtime_state
        .insert("anchor_emitted".to_string(), Value::Bool(true));
    updated.runtime_state.insert(
        "reply_buffer".to_string(),
        Value::String(reply_buffer.to_string()),
    );
    if let Some(seq) = updated
        .runtime_state
        .get("next_seq")
        .and_then(Value::as_u64)
    {
        updated
            .runtime_state
            .insert("next_seq".to_string(), Value::Number((seq + 1).into()));
    }

    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("done".to_string()),
        confidence: Some(CompletionConfidence::Exact),
        reply: cleaned,
        anchor_seen: true,
        reply_started: true,
        reply_stable: true,
        provider_turn_ref: Some(request_anchor.to_string()),
        source_cursor: Some(item.cursor.clone()),
        finished_at: Some(now.to_string()),
        diagnostics: Default::default(),
    };

    Some(ProviderPollResult::new(updated, vec![item], Some(decision)))
}

fn terminal_from_send_error(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    now: &str,
    err: &str,
) -> Option<ProviderPollResult> {
    let updated = ProviderSubmission {
        runtime_state: state.clone(),
        status: CompletionStatus::Failed,
        reason: format!("send_failed:{err}"),
        reply: String::new(),
        confidence: CompletionConfidence::Degraded,
        ..submission.clone()
    };
    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: state.get("next_seq").and_then(Value::as_u64).or(Some(1)),
        updated_at: Some(now.to_string()),
        ..Default::default()
    };
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(format!("send_failed:{err}")),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: Some(request_anchor(&submission.job_id)),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics: Default::default(),
    };
    Some(ProviderPollResult::new(updated, Vec::new(), Some(decision)))
}

fn dispatch_prompt(state: &HashMap<String, Value>, pane_id: &str) -> Option<String> {
    let prompt = state
        .get("prompt_text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if prompt.is_empty() {
        return Some("prompt_text_missing".to_string());
    }
    let target = resolve_prompt_target(state)?;
    target.send_text(pane_id, &prompt).err()
}

fn state_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn expand_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest.trim_start_matches('/'));
        }
    }
    PathBuf::from(path)
}

fn load_project_session(work_dir: &Path) -> Option<HashMap<String, Value>> {
    let path = find_session_file_for_work_dir(work_dir, GEMINI_SESSION_FILENAME)?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn store_backend_config_from_session_data(
    state: &mut HashMap<String, Value>,
    data: &HashMap<String, Value>,
) {
    let socket_name = data
        .get("tmux_socket_name")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let socket_path = data
        .get("tmux_socket_path")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    if socket_name.is_some() || socket_path.is_some() {
        state.insert(
            "backend_type".to_string(),
            Value::String("tmux".to_string()),
        );
    }
    if let Some(name) = socket_name {
        state.insert("tmux_socket_name".to_string(), Value::String(name));
    }
    if let Some(path) = socket_path {
        state.insert("tmux_socket_path".to_string(), Value::String(path));
    }
}

fn reader_state_from_runtime_state(
    runtime_state: &HashMap<String, Value>,
) -> Option<log_reader::GeminiReaderState> {
    runtime_state
        .get("reader_state")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

// ---------------------------------------------------------------------------
// Protocol helpers
// ---------------------------------------------------------------------------

/// Generate a short request ID from a job ID.
pub fn make_req_id(job_id: &str) -> String {
    protocol::make_req_id(job_id)
}

/// Build the request anchor marker for a job.
pub fn request_anchor(job_id: &str) -> String {
    protocol::request_anchor_for_job(job_id)
}

/// Wrap a prompt with Gemini request/done marker instructions.
///
/// Mirrors `provider_backends.gemini.protocol_runtime.wrap_gemini_prompt`.
pub fn wrap_gemini_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    let lines = [
        format!("{} {}", protocol::REQ_ID_PREFIX, req_id),
        String::new(),
        rendered.to_string(),
        String::new(),
        "IMPORTANT — you MUST follow these rules:".to_string(),
        "1. Reply in English with an execution summary. Do not stay silent.".to_string(),
        "2. Your FINAL line MUST be exactly (copy verbatim, no extra text):".to_string(),
        format!("   {} {}", protocol::DONE_PREFIX, req_id),
        "3. Do NOT omit, modify, or paraphrase the line above.".to_string(),
    ];
    lines.join("\n") + "\n"
}

/// Wrap a prompt as a simple turn prompt with a request ID header.
///
/// Mirrors `provider_backends.gemini.protocol_runtime.wrap_gemini_turn_prompt`.
pub fn wrap_gemini_turn_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    format!("{}\n\n{}\n", req_id, rendered)
}

/// Check whether a reply buffer contains any done marker.
pub fn is_done_text(text: &str) -> bool {
    protocol::is_done_text(text)
}

/// Strip done markers from a reply buffer.
pub fn strip_done_text(text: &str) -> String {
    protocol::strip_done_text(text)
}

/// Extract the reply text for a specific request ID.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    protocol::extract_reply_for_req(text, req_id)
}

// ---------------------------------------------------------------------------
// Home layout helpers
// ---------------------------------------------------------------------------

const GEMINI_HOME_ENV: &str = "GEMINI_ROOT";

/// Layout of a Gemini home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiHomeLayout {
    pub home_root: PathBuf,
    pub gemini_dir: PathBuf,
    pub settings_path: PathBuf,
    pub trusted_folders_path: PathBuf,
    pub tmp_root: PathBuf,
}

/// Build a `GeminiHomeLayout` from a home root path.
pub fn gemini_layout_for_home(home_root: impl AsRef<Path>) -> GeminiHomeLayout {
    let root = home_root.as_ref().to_path_buf();
    let gemini_dir = root.join(".gemini");
    GeminiHomeLayout {
        home_root: root.clone(),
        gemini_dir: gemini_dir.clone(),
        settings_path: gemini_dir.join("settings.json"),
        trusted_folders_path: gemini_dir.join("trustedFolders.json"),
        tmp_root: gemini_dir.join("tmp"),
    }
}

/// Resolve the current Gemini home root.
///
/// Prefers the `GEMINI_ROOT` environment variable, otherwise falls back to the
/// user's home directory.
pub fn current_gemini_home_root() -> PathBuf {
    if let Some(root) = env_root() {
        if let Some(home) = home_root_from_tmp_root(&root) {
            return home;
        }
    }
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Resolve the current Gemini temp root.
pub fn current_gemini_tmp_root() -> PathBuf {
    env_root().unwrap_or_else(|| gemini_layout_for_home(current_gemini_home_root()).tmp_root)
}

/// Derive a home layout from session data.
pub fn gemini_layout_from_session_data(
    data: Option<&HashMap<String, Value>>,
) -> Option<GeminiHomeLayout> {
    let data = data?;
    if let Some(home) = path_or_none(data.get("gemini_home")) {
        return Some(gemini_layout_for_home(home));
    }
    if let Some(tmp_root) = path_or_none(data.get("gemini_root")) {
        if let Some(home) = home_root_from_tmp_root(&tmp_root) {
            return Some(gemini_layout_for_home(home));
        }
    }
    if let Some(session_path) = path_or_none(data.get("gemini_session_path")) {
        if let Some(home) = home_root_from_session_path(&session_path) {
            return Some(gemini_layout_for_home(home));
        }
    }
    None
}

fn env_root() -> Option<PathBuf> {
    std::env::var(GEMINI_HOME_ENV).ok().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    })
}

fn home_root_from_tmp_root(tmp_root: &Path) -> Option<PathBuf> {
    if tmp_root.file_name()? != "tmp" {
        return None;
    }
    let parent = tmp_root.parent()?;
    if parent.file_name()? != ".gemini" {
        return None;
    }
    Some(parent.parent()?.to_path_buf())
}

fn home_root_from_session_path(session_path: &Path) -> Option<PathBuf> {
    for parent in session_path.ancestors() {
        if parent.file_name()? == ".gemini" {
            return parent.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn path_or_none(value: Option<&Value>) -> Option<PathBuf> {
    value
        .and_then(Value::as_str)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

// Minimal home-dir shim to avoid pulling in the `dirs` dependency if it is
// already transitively available; otherwise fall back to `$HOME`.
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
    }
}
