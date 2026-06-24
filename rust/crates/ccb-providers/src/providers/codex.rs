use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ccb_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::protocol::{
    request_anchor_for_job, strip_done_text, wrap_codex_prompt, REQ_ID_PREFIX,
};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::execution::{
    build_item, no_wrap_requested, passive_submission, request_anchor_from_runtime_state,
    ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};

pub const PROVIDER_NAME: &str = "codex";

const SESSION_FILENAME: &str = ".codex-session";
const SESSION_ID_PATTERN: &str = r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";

/// A Codex project session loaded from disk.
#[derive(Debug, Clone)]
pub struct CodexProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl CodexProjectSession {
    pub fn codex_session_path(&self) -> Option<&str> {
        self.data
            .get("codex_session_path")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn codex_session_root(&self) -> Option<&str> {
        self.data
            .get("codex_session_root")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn codex_home(&self) -> Option<&str> {
        self.data
            .get("codex_home")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn codex_session_id(&self) -> Option<&str> {
        self.data
            .get("codex_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }

    pub fn work_dir(&self) -> Option<&str> {
        self.data
            .get("work_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }
}

/// Find the Codex project session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load the Codex project session for a work directory.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<CodexProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let data = read_session_json(&session_file)?;
    if data.is_empty() {
        return None;
    }
    Some(CodexProjectSession { session_file, data })
}

/// Load a Codex project session for an agent without falling back to the
/// primary session when the agent is named.
///
/// Mirrors Python `provider_backends.codex.execution_runtime.start.load_session`.
pub fn load_session<F>(
    load_project_session_fn: F,
    work_dir: &Path,
    agent_name: &str,
) -> Option<CodexProjectSession>
where
    F: FnOnce(&Path, Option<&str>) -> Option<CodexProjectSession>,
{
    let instance =
        ccb_provider_core::instance_resolution::named_agent_instance(agent_name, "codex");
    load_project_session_fn(work_dir, instance.as_deref())
}

fn read_session_json(path: &Path) -> Option<HashMap<String, Value>> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let value: Value = serde_json::from_str(raw).ok()?;
    value
        .as_object()
        .cloned()
        .map(|obj| obj.into_iter().collect())
}

/// Build the Codex provider manifest.
///
/// Mirrors Python `provider_backends.codex.manifest.build_manifest`.
pub fn manifest() -> ProviderManifest {
    let provider = PROVIDER_NAME.to_string();
    let mut profiles = std::collections::HashMap::new();
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
        true,  // supports_stream_watch
        false, // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build the Codex provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(build_runtime_launcher()),
    }
}

/// Build the Codex session binding.
pub fn build_session_binding() -> ProviderSessionBinding {
    ProviderSessionBinding {
        provider: PROVIDER_NAME.to_string(),
        session_id_attr: "codex_session_id".to_string(),
        session_path_attr: "codex_session_path".to_string(),
    }
}

/// Build the Codex runtime launcher descriptor.
pub fn build_runtime_launcher() -> ProviderRuntimeLauncher {
    ProviderRuntimeLauncher::new(PROVIDER_NAME, LaunchMode::CodexTmux)
}

/// Codex provider execution adapter.
#[derive(Debug, Clone)]
pub struct CodexExecutionAdapter;

impl ExecutionAdapter for CodexExecutionAdapter {
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
        // All runtime state values are JSON-serializable.
        Some(submission.runtime_state.clone())
    }

    fn resume(
        &self,
        _job: &JobRecord,
        submission: &ProviderSubmission,
        context: Option<&ProviderRuntimeContext>,
        _persisted_state: &crate::execution::PersistedExecutionState,
        _now: &str,
    ) -> Option<ProviderSubmission> {
        resume_submission(submission, context)
    }
}

// ---------------------------------------------------------------------------
// Launcher / start helpers
// ---------------------------------------------------------------------------

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let request_anchor = request_anchor_for_job(&job.job_id);
    let no_wrap = no_wrap_requested(job.provider_options.get("no_wrap"));

    let workspace_path = context
        .and_then(|c| c.workspace_path.as_deref())
        .map(expand_tilde)
        .unwrap_or_default();
    if workspace_path.is_empty() {
        return passive_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::ProtocolEventStream,
            "missing_workspace",
        );
    }

    let workspace_path_buf = PathBuf::from(&workspace_path);
    let session = load_project_session(&workspace_path_buf, None);

    let session_path = session
        .as_ref()
        .and_then(|s| s.codex_session_path())
        .map(|s| s.to_string())
        .or_else(|| {
            context
                .and_then(|c| c.session_ref.as_deref())
                .filter(|s| s.ends_with(".jsonl") || s.ends_with(".log"))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| format!("{}/codex-session.jsonl", workspace_path));

    let pane_id = context
        .and_then(|c| c.runtime_ref.as_deref())
        .unwrap_or("")
        .to_string();

    let session_root = session
        .as_ref()
        .and_then(|s| codex_session_root_path(&s.data))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let prompt = if no_wrap {
        job.request.body.clone()
    } else {
        wrap_codex_prompt(
            &job.request.body,
            &ccb_provider_core::protocol::make_req_id(&job.job_id),
        )
    };

    let diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "active",
        "workspace_path": workspace_path,
    });

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert("state".to_string(), {
        let mut state = Map::new();
        state.insert("log_path".to_string(), Value::String(session_path.clone()));
        state.insert("offset".to_string(), Value::Number(0.into()));
        state.insert("last_rescan".to_string(), Value::Number(0.into()));
        Value::Object(state)
    });
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_seen".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("bound_turn_id".to_string(), Value::String(String::new()));
    runtime_state.insert("bound_task_id".to_string(), Value::String(String::new()));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert(
        "last_agent_message".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "last_final_answer".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "last_assistant_message".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "last_assistant_signature".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert(
        "session_path".to_string(),
        Value::String(session_path.clone()),
    );
    if !session_root.is_empty() {
        runtime_state.insert(
            "codex_session_root".to_string(),
            Value::String(session_root),
        );
    }
    runtime_state.insert("workspace_path".to_string(), Value::String(workspace_path));
    runtime_state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    runtime_state.insert(
        "delivery_state".to_string(),
        Value::String(
            if no_wrap {
                "not_required"
            } else {
                "pending_anchor"
            }
            .to_string(),
        ),
    );
    runtime_state.insert(
        "delivery_started_at".to_string(),
        Value::String(if no_wrap { "" } else { now }.to_string()),
    );
    runtime_state.insert(
        "delivery_timeout_s".to_string(),
        Value::Number(
            serde_json::Number::from_f64(if no_wrap {
                0.0
            } else {
                resolved_delivery_timeout_s()
            })
            .unwrap_or_else(|| 0.into()),
        ),
    );
    runtime_state.insert(
        "delivery_target_pane_id".to_string(),
        Value::String(pane_id),
    );
    runtime_state.insert(
        "delivery_target_session_path".to_string(),
        Value::String(session_path),
    );
    runtime_state.insert(
        "delivery_confirmed_at".to_string(),
        Value::String(String::new()),
    );
    runtime_state.insert("prompt".to_string(), Value::String(prompt));

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: PROVIDER_NAME.to_string(),
        accepted_at: now.to_string(),
        ready_at: now.to_string(),
        source_kind: CompletionSourceKind::ProtocolEventStream,
        reply: String::new(),
        status: CompletionStatus::Incomplete,
        reason: "in_progress".to_string(),
        confidence: CompletionConfidence::Observed,
        diagnostics: Some(diagnostics),
        runtime_state,
    }
}

fn resume_submission(
    submission: &ProviderSubmission,
    context: Option<&ProviderRuntimeContext>,
) -> Option<ProviderSubmission> {
    let context = context?;
    if get_str(&submission.runtime_state, "mode") != "active" {
        return None;
    }
    let workspace_path = context
        .workspace_path
        .as_deref()
        .map(expand_tilde)
        .filter(|s| !s.is_empty())?;
    let pane_id = context.runtime_ref.as_deref().unwrap_or("").to_string();

    let mut runtime_state = submission.runtime_state.clone();
    runtime_state.insert("workspace_path".to_string(), Value::String(workspace_path));
    runtime_state.insert("pane_id".to_string(), Value::String(pane_id));
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    if get_str(&runtime_state, "session_path").is_empty() {
        runtime_state.insert(
            "session_path".to_string(),
            Value::String(preferred_log_path(&runtime_state).unwrap_or_default()),
        );
    }
    Some(ProviderSubmission {
        runtime_state,
        ..submission.clone()
    })
}

fn preferred_log_path(runtime_state: &HashMap<String, Value>) -> Option<String> {
    runtime_state
        .get("state")
        .and_then(|v| v.as_object())
        .and_then(|obj| obj.get("log_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn resolved_delivery_timeout_s() -> f64 {
    std::env::var("CCB_CODEX_DELIVERY_TIMEOUT_S")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(120.0)
        .max(0.0)
}

// ---------------------------------------------------------------------------
// Polling / state machine
// ---------------------------------------------------------------------------

fn poll_submission(submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
    if get_str(&submission.runtime_state, "mode") != "active" {
        return None;
    }

    if let Some(failure) = delivery_acceptance_guard(submission, now) {
        return Some(failure);
    }

    let runtime_state = refresh_runtime_state(submission, now);
    let submission = ProviderSubmission {
        runtime_state,
        ..submission.clone()
    };

    let mut poll = build_poll_state(&submission);
    let session_path = poll.session_path.clone();
    if session_path.is_empty() {
        return None;
    }

    let state = submission
        .runtime_state
        .get("state")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    let current_log_path = state_session_path(&state).unwrap_or(session_path);
    apply_session_rotation(&submission, &mut poll, &current_log_path, now);

    let path = PathBuf::from(expand_tilde(&current_log_path));
    let offset = state.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
    let (entries, new_offset) = read_log_entries(&path, offset).unwrap_or_default();

    for entry in entries {
        update_binding_refs(&mut poll, &entry);
        match entry.role.as_str() {
            "user" => handle_user_entry(&submission, &mut poll, &entry.text, now),
            "assistant" if poll.anchor_seen => {
                handle_assistant_entry(&submission, &mut poll, &entry, now);
            }
            "system" if poll.anchor_seen => {
                handle_terminal_entry(&submission, &mut poll, &entry, now);
            }
            _ => {}
        }
        if poll.reached_terminal {
            break;
        }
    }

    let mut current_state = match state {
        Value::Object(obj) => obj,
        _ => Map::new(),
    };
    current_state.insert(
        "log_path".to_string(),
        Value::String(path.to_string_lossy().to_string()),
    );
    current_state.insert("offset".to_string(), Value::Number(new_offset.into()));

    finalize_poll_result(&submission, poll, Value::Object(current_state), now)
}

#[derive(Debug, Clone)]
struct CodexPollState {
    request_anchor: String,
    next_seq: u64,
    anchor_seen: bool,
    bound_turn_id: String,
    bound_task_id: String,
    reply_buffer: String,
    last_agent_message: String,
    last_final_answer: String,
    last_assistant_message: String,
    last_assistant_signature: String,
    session_path: String,
    no_wrap: bool,
    terminal_reason: String,
    items: Vec<CompletionItem>,
    reached_terminal: bool,
}

fn build_poll_state(submission: &ProviderSubmission) -> CodexPollState {
    let state = &submission.runtime_state;
    CodexPollState {
        request_anchor: request_anchor_from_runtime_state(state, &submission.job_id),
        next_seq: get_u64(state, "next_seq", 1),
        anchor_seen: get_bool(state, "anchor_seen"),
        bound_turn_id: get_str(state, "bound_turn_id"),
        bound_task_id: get_str(state, "bound_task_id"),
        reply_buffer: get_str(state, "reply_buffer"),
        last_agent_message: get_str(state, "last_agent_message"),
        last_final_answer: get_str(state, "last_final_answer"),
        last_assistant_message: get_str(state, "last_assistant_message"),
        last_assistant_signature: get_str(state, "last_assistant_signature"),
        session_path: get_str(state, "session_path"),
        no_wrap: get_bool(state, "no_wrap"),
        terminal_reason: String::new(),
        items: Vec::new(),
        reached_terminal: false,
    }
}

fn apply_session_rotation(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    new_session_path: &str,
    now: &str,
) {
    if new_session_path.is_empty() || new_session_path == poll.session_path {
        return;
    }
    let mut payload = HashMap::new();
    payload.insert(
        "session_path".to_string(),
        Value::String(new_session_path.to_string()),
    );
    if let Some(provider_session_id) = Path::new(new_session_path)
        .file_stem()
        .and_then(|s| s.to_str())
    {
        payload.insert(
            "provider_session_id".to_string(),
            Value::String(provider_session_id.to_string()),
        );
    }
    let item = build_item(
        submission,
        CompletionItemKind::SessionRotate,
        now,
        poll.next_seq,
        payload,
    );
    poll.items.push(item);
    poll.next_seq += 1;
    poll.session_path = new_session_path.to_string();
    poll.anchor_seen = poll.no_wrap;
    poll.bound_turn_id.clear();
    poll.bound_task_id.clear();
    poll.reply_buffer.clear();
    poll.last_agent_message.clear();
    poll.last_final_answer.clear();
    poll.last_assistant_message.clear();
    poll.last_assistant_signature.clear();
}

fn update_binding_refs(poll: &mut CodexPollState, entry: &CodexLogEntry) {
    if !entry.turn_id.is_empty() {
        poll.bound_turn_id = entry.turn_id.clone();
    }
    if !entry.task_id.is_empty() {
        poll.bound_task_id = entry.task_id.clone();
    }
}

fn handle_user_entry(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    text: &str,
    now: &str,
) {
    if poll.request_anchor.is_empty() || poll.anchor_seen {
        return;
    }
    let needle = format!("{} {}", REQ_ID_PREFIX, poll.request_anchor);
    if text.contains(&needle) {
        let mut payload = HashMap::new();
        payload.insert(
            "turn_id".to_string(),
            Value::String(if poll.bound_turn_id.is_empty() {
                poll.request_anchor.clone()
            } else {
                poll.bound_turn_id.clone()
            }),
        );
        if !poll.bound_task_id.is_empty() {
            payload.insert(
                "task_id".to_string(),
                Value::String(poll.bound_task_id.clone()),
            );
        }
        if !poll.session_path.is_empty() {
            payload.insert(
                "session_path".to_string(),
                Value::String(poll.session_path.clone()),
            );
        }
        let item = build_item(
            submission,
            CompletionItemKind::AnchorSeen,
            now,
            poll.next_seq,
            payload,
        );
        poll.items.push(item);
        poll.next_seq += 1;
        poll.anchor_seen = true;
    }
}

fn handle_assistant_entry(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    entry: &CodexLogEntry,
    now: &str,
) {
    if is_duplicate_assistant_entry(poll, entry) {
        return;
    }
    let cleaned = clean_codex_reply_text(&entry.text).trim().to_string();
    if cleaned.is_empty() {
        return;
    }
    poll.reply_buffer = append_reply_text(&poll.reply_buffer, &cleaned);
    poll.last_assistant_message = cleaned.clone();
    if entry.phase == "final_answer" {
        poll.last_final_answer = cleaned.clone();
    }

    let mut payload = HashMap::new();
    payload.insert("text".to_string(), Value::String(cleaned));
    payload.insert(
        "merged_text".to_string(),
        Value::String(poll.reply_buffer.clone()),
    );
    if !poll.bound_turn_id.is_empty() {
        payload.insert(
            "turn_id".to_string(),
            Value::String(poll.bound_turn_id.clone()),
        );
    }
    if !poll.bound_task_id.is_empty() {
        payload.insert(
            "task_id".to_string(),
            Value::String(poll.bound_task_id.clone()),
        );
    }
    if !entry.phase.is_empty() {
        payload.insert("phase".to_string(), Value::String(entry.phase.clone()));
    }
    if !poll.session_path.is_empty() {
        payload.insert(
            "session_path".to_string(),
            Value::String(poll.session_path.clone()),
        );
    }
    let item = build_item(
        submission,
        CompletionItemKind::AssistantChunk,
        now,
        poll.next_seq,
        payload,
    );
    poll.items.push(item);
    poll.next_seq += 1;
}

fn is_duplicate_assistant_entry(poll: &mut CodexPollState, entry: &CodexLogEntry) -> bool {
    let signature = assistant_signature(entry);
    if signature.is_empty() {
        return false;
    }
    if signature == poll.last_assistant_signature {
        return true;
    }
    poll.last_assistant_signature = signature;
    false
}

fn assistant_signature(entry: &CodexLogEntry) -> String {
    if entry.timestamp.is_empty() || entry.text.is_empty() {
        return String::new();
    }
    format!("{}\0{}\0{}", entry.timestamp, entry.phase, entry.text)
}

fn clean_codex_reply_text(text: &str) -> String {
    strip_done_text(text)
}

fn append_reply_text(buffer: &str, cleaned: &str) -> String {
    if buffer.is_empty() {
        cleaned.to_string()
    } else {
        format!("{}\n{}", buffer, cleaned)
    }
}

fn handle_terminal_entry(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    entry: &CodexLogEntry,
    now: &str,
) {
    match entry.payload_type.as_str() {
        "task_complete" => append_task_complete_item(submission, poll, entry, now),
        "turn_aborted" => append_turn_aborted_item(submission, poll, entry, now),
        _ => {}
    }
}

fn append_task_complete_item(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    entry: &CodexLogEntry,
    now: &str,
) {
    let text = entry.last_agent_message.trim();
    if !text.is_empty() {
        poll.last_agent_message = clean_codex_reply_text(text).trim().to_string();
    }
    poll.terminal_reason = "task_complete".to_string();
    let payload = task_complete_payload(poll);
    let item = build_item(
        submission,
        CompletionItemKind::TurnBoundary,
        now,
        poll.next_seq,
        payload,
    );
    poll.items.push(item);
    poll.next_seq += 1;
    poll.reached_terminal = true;
}

fn append_turn_aborted_item(
    submission: &ProviderSubmission,
    poll: &mut CodexPollState,
    entry: &CodexLogEntry,
    now: &str,
) {
    let reason = if entry.reason.is_empty() {
        "turn_aborted"
    } else {
        &entry.reason
    };
    poll.terminal_reason = reason.to_string();
    let payload = turn_aborted_payload(poll, reason, &entry.text);
    let item = build_item(
        submission,
        CompletionItemKind::TurnAborted,
        now,
        poll.next_seq,
        payload,
    );
    poll.items.push(item);
    poll.next_seq += 1;
    poll.reached_terminal = true;
}

fn task_complete_payload(poll: &CodexPollState) -> HashMap<String, Value> {
    let mut payload = HashMap::new();
    payload.insert(
        "reason".to_string(),
        Value::String("task_complete".to_string()),
    );
    payload.insert(
        "last_agent_message".to_string(),
        Value::String(selected_reply(poll)),
    );
    add_binding_payload(&mut payload, poll);
    payload
}

fn turn_aborted_payload(
    poll: &CodexPollState,
    reason: &str,
    error_text: &str,
) -> HashMap<String, Value> {
    let status = abort_status(reason);
    let mut payload = HashMap::new();
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    payload.insert("status".to_string(), Value::String(status.to_string()));
    payload.insert(
        "last_agent_message".to_string(),
        Value::String(selected_reply(poll)),
    );
    if !error_text.is_empty() {
        payload.insert("text".to_string(), Value::String(error_text.to_string()));
        payload.insert(
            "error_message".to_string(),
            Value::String(error_text.to_string()),
        );
    }
    add_binding_payload(&mut payload, poll);
    payload
}

fn add_binding_payload(payload: &mut HashMap<String, Value>, poll: &CodexPollState) {
    if !poll.bound_turn_id.is_empty() || !poll.request_anchor.is_empty() {
        payload.insert(
            "turn_id".to_string(),
            Value::String(if poll.bound_turn_id.is_empty() {
                poll.request_anchor.clone()
            } else {
                poll.bound_turn_id.clone()
            }),
        );
    }
    if !poll.bound_task_id.is_empty() {
        payload.insert(
            "task_id".to_string(),
            Value::String(poll.bound_task_id.clone()),
        );
    }
    if !poll.session_path.is_empty() {
        payload.insert(
            "session_path".to_string(),
            Value::String(poll.session_path.clone()),
        );
    }
}

fn selected_reply(poll: &CodexPollState) -> String {
    select_reply(
        &poll.last_agent_message,
        &poll.last_final_answer,
        &poll.last_assistant_message,
        &poll.reply_buffer,
    )
}

fn select_reply(
    last_agent_message: &str,
    last_final_answer: &str,
    last_assistant_message: &str,
    reply_buffer: &str,
) -> String {
    for candidate in [
        last_agent_message,
        last_final_answer,
        last_assistant_message,
        reply_buffer,
    ] {
        let text = candidate.trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }
    String::new()
}

fn abort_status(reason: &str) -> &'static str {
    let lowered = reason.to_lowercase();
    if lowered.contains("interrupt") || lowered.contains("cancel") || lowered.contains("abort") {
        "cancelled"
    } else {
        "failed"
    }
}

fn finalize_poll_result(
    submission: &ProviderSubmission,
    poll: CodexPollState,
    state: Value,
    now: &str,
) -> Option<ProviderPollResult> {
    let prior_state = submission
        .runtime_state
        .get("state")
        .cloned()
        .unwrap_or(Value::Object(Map::new()));
    let prior_session_path = get_str(&submission.runtime_state, "session_path");

    let mut runtime_state = submission.runtime_state.clone();
    runtime_state.insert("state".to_string(), state);
    runtime_state.insert("next_seq".to_string(), Value::Number(poll.next_seq.into()));
    runtime_state.insert("anchor_seen".to_string(), Value::Bool(poll.anchor_seen));
    runtime_state.insert(
        "bound_turn_id".to_string(),
        Value::String(poll.bound_turn_id.clone()),
    );
    runtime_state.insert(
        "bound_task_id".to_string(),
        Value::String(poll.bound_task_id.clone()),
    );
    runtime_state.insert(
        "reply_buffer".to_string(),
        Value::String(poll.reply_buffer.clone()),
    );
    runtime_state.insert(
        "last_agent_message".to_string(),
        Value::String(poll.last_agent_message.clone()),
    );
    runtime_state.insert(
        "last_final_answer".to_string(),
        Value::String(poll.last_final_answer.clone()),
    );
    runtime_state.insert(
        "last_assistant_message".to_string(),
        Value::String(poll.last_assistant_message.clone()),
    );
    runtime_state.insert(
        "last_assistant_signature".to_string(),
        Value::String(poll.last_assistant_signature.clone()),
    );
    runtime_state.insert(
        "session_path".to_string(),
        Value::String(poll.session_path.clone()),
    );

    if poll.anchor_seen && get_str(&runtime_state, "delivery_state") == "pending_anchor" {
        runtime_state.insert(
            "delivery_state".to_string(),
            Value::String("accepted".to_string()),
        );
        runtime_state.insert(
            "delivery_confirmed_at".to_string(),
            Value::String(now.to_string()),
        );
    }

    let reply = if poll.items.is_empty() {
        submission.reply.clone()
    } else {
        selected_reply(&poll)
    };

    let updated_submission = ProviderSubmission {
        reply,
        runtime_state,
        ..submission.clone()
    };

    let current_state = updated_submission
        .runtime_state
        .get("state")
        .cloned()
        .unwrap_or(Value::Null);
    if poll.items.is_empty()
        && prior_state == current_state
        && prior_session_path == poll.session_path
    {
        return None;
    }

    let decision = if poll.reached_terminal {
        let status = if abort_status(&poll.terminal_reason) == "cancelled" {
            CompletionStatus::Cancelled
        } else {
            CompletionStatus::Completed
        };
        Some(build_terminal_decision(
            &updated_submission,
            &poll,
            now,
            status,
        ))
    } else {
        None
    };

    Some(ProviderPollResult::new(
        updated_submission,
        poll.items,
        decision,
    ))
}

fn build_terminal_decision(
    submission: &ProviderSubmission,
    poll: &CodexPollState,
    now: &str,
    status: CompletionStatus,
) -> CompletionDecision {
    let request_anchor =
        request_anchor_from_runtime_state(&submission.runtime_state, &submission.job_id);
    let reply = selected_reply(poll);
    let confidence = if status == CompletionStatus::Cancelled {
        CompletionConfidence::Degraded
    } else {
        CompletionConfidence::Observed
    };
    let source_cursor = poll.items.last().map(|item| item.cursor.clone());
    let mut diagnostics = Map::new();
    diagnostics.insert(
        "reason".to_string(),
        Value::String(poll.terminal_reason.clone()),
    );
    diagnostics.insert("status".to_string(), Value::String(format!("{:?}", status)));
    diagnostics.insert("reply_empty".to_string(), Value::Bool(reply.is_empty()));
    diagnostics.insert("anchor_seen".to_string(), Value::Bool(poll.anchor_seen));
    if let Some(delivery_state) = submission.runtime_state.get("delivery_state") {
        diagnostics.insert("delivery_state".to_string(), delivery_state.clone());
    }
    CompletionDecision {
        terminal: true,
        status,
        reason: Some(poll.terminal_reason.clone()),
        confidence: Some(confidence),
        reply: reply.clone(),
        anchor_seen: poll.anchor_seen,
        reply_started: !reply.is_empty(),
        reply_stable: true,
        provider_turn_ref: Some(request_anchor),
        source_cursor,
        finished_at: Some(now.to_string()),
        diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Delivery acceptance guard
// ---------------------------------------------------------------------------

fn delivery_acceptance_guard(
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    let state = &submission.runtime_state;
    if get_str(state, "mode") != "active" {
        return None;
    }
    if get_bool(state, "anchor_seen") || get_bool(state, "no_wrap") {
        return None;
    }
    if get_str(state, "delivery_state") != "pending_anchor" {
        return None;
    }
    if get_str(state, "delivery_target_pane_id").is_empty() {
        return None;
    }

    let failure_kind = delivery_failure_kind(state, submission, now);
    let failure_kind = failure_kind.as_deref()?;

    let work_dir = submission_work_dir(submission, state)?;
    let session = load_project_session(&work_dir, None)?;
    let current_log = current_session_log(&session)?;
    if !current_log.exists() {
        return None;
    }

    let poll_state = state
        .get("state")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let offset = poll_state
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if !current_log_is_drained(&current_log, offset) {
        return None;
    }
    if active_anchor_fallback_log(state).is_some() {
        return None;
    }
    if anchor_fallback_log(
        submission,
        state,
        &poll_state,
        &session,
        &work_dir,
        &current_log,
    )
    .is_some()
    {
        return None;
    }

    Some(delivery_failure_result(
        submission,
        now,
        failure_kind,
        &current_log,
        codex_session_root_path(&session.data).as_deref(),
        &work_dir,
    ))
}

fn delivery_failure_kind(
    state: &HashMap<String, Value>,
    submission: &ProviderSubmission,
    now: &str,
) -> Option<String> {
    if delivery_timeout_elapsed(state, submission, now) {
        Some("delivery_anchor_missing".to_string())
    } else {
        None
    }
}

fn delivery_timeout_elapsed(
    state: &HashMap<String, Value>,
    submission: &ProviderSubmission,
    now: &str,
) -> bool {
    let timeout_s = delivery_timeout_s(state);
    if timeout_s <= 0.0 {
        return false;
    }
    let started_at = {
        let raw = get_str(state, "delivery_started_at");
        if !raw.is_empty() {
            raw
        } else if !submission.ready_at.is_empty() {
            submission.ready_at.clone()
        } else if !submission.accepted_at.is_empty() {
            submission.accepted_at.clone()
        } else {
            return false;
        }
    };
    if started_at.is_empty() {
        return false;
    }
    let started = parse_timestamp(&started_at);
    let now_ts = parse_timestamp(now);
    match (started, now_ts) {
        (Some(s), Some(n)) => {
            let elapsed = (n - s).num_seconds() as f64;
            elapsed >= timeout_s
        }
        _ => false,
    }
}

fn delivery_timeout_s(state: &HashMap<String, Value>) -> f64 {
    state
        .get("delivery_timeout_s")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(resolved_delivery_timeout_s)
        .max(0.0)
}

fn delivery_failure_result(
    submission: &ProviderSubmission,
    now: &str,
    failure_kind: &str,
    current_log: &Path,
    checked_root: Option<&Path>,
    work_dir: &Path,
) -> ProviderPollResult {
    let reason = "codex_prompt_delivery_failed";
    let state = &submission.runtime_state;
    let seq = get_u64(state, "next_seq", 1);
    let request_anchor = request_anchor_from_runtime_state(state, &submission.job_id);
    let mut diagnostics = Map::new();
    if let Some(existing) = submission.diagnostics.as_ref() {
        if let Some(obj) = existing.as_object() {
            for (k, v) in obj {
                diagnostics.insert(k.clone(), v.clone());
            }
        }
    }
    diagnostics.insert("reason".to_string(), Value::String(reason.to_string()));
    diagnostics.insert(
        "delivery_failure_kind".to_string(),
        Value::String(failure_kind.to_string()),
    );
    diagnostics.insert("delivery_retryable".to_string(), Value::Bool(true));
    diagnostics.insert(
        "delivery_state".to_string(),
        Value::String("failed".to_string()),
    );
    diagnostics.insert(
        "delivery_started_at".to_string(),
        Value::String(get_str(state, "delivery_started_at")),
    );
    diagnostics.insert(
        "delivery_timeout_s".to_string(),
        Value::Number(
            serde_json::Number::from_f64(delivery_timeout_s(state)).unwrap_or_else(|| 0.into()),
        ),
    );
    diagnostics.insert(
        "delivery_checked_session_root".to_string(),
        Value::String(
            checked_root
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    current_log
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                }),
        ),
    );
    diagnostics.insert(
        "delivery_current_log_path".to_string(),
        Value::String(current_log.to_string_lossy().to_string()),
    );
    diagnostics.insert(
        "delivery_workspace_path".to_string(),
        Value::String(work_dir.to_string_lossy().to_string()),
    );
    diagnostics.insert("delivery_anchor_seen".to_string(), Value::Bool(false));

    let mut payload = HashMap::new();
    payload.insert("reason".to_string(), Value::String(reason.to_string()));
    payload.insert(
        "delivery_failure_kind".to_string(),
        Value::String(failure_kind.to_string()),
    );
    payload.insert("delivery_retryable".to_string(), Value::Bool(true));
    let item = build_item(submission, CompletionItemKind::Error, now, seq, payload);

    let mut runtime_state = state.clone();
    runtime_state.insert("mode".to_string(), Value::String("passive".to_string()));
    runtime_state.insert(
        "next_seq".to_string(),
        Value::Number((item.cursor.event_seq.unwrap_or(seq) + 1).into()),
    );
    runtime_state.insert(
        "delivery_state".to_string(),
        Value::String("failed".to_string()),
    );
    runtime_state.insert(
        "delivery_failure_kind".to_string(),
        Value::String(failure_kind.to_string()),
    );
    runtime_state.insert(
        "delivery_failed_at".to_string(),
        Value::String(now.to_string()),
    );

    let updated = ProviderSubmission {
        runtime_state,
        diagnostics: Some(Value::Object(diagnostics.clone())),
        ..submission.clone()
    };
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(reason.to_string()),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: Some(request_anchor),
        source_cursor: Some(item.cursor.clone()),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(updated, vec![item], Some(decision))
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn submission_work_dir(
    submission: &ProviderSubmission,
    state: &HashMap<String, Value>,
) -> Option<PathBuf> {
    let raw = state
        .get("workspace_path")
        .and_then(|v| v.as_str())
        .or_else(|| {
            submission
                .diagnostics
                .as_ref()
                .and_then(|d| d.get("workspace_path"))
                .and_then(|v| v.as_str())
        })?;
    if raw.is_empty() {
        return None;
    }
    Some(PathBuf::from(expand_tilde(raw)))
}

fn current_session_log(session: &CodexProjectSession) -> Option<PathBuf> {
    session
        .codex_session_path()
        .map(|s| PathBuf::from(expand_tilde(s)))
        .filter(|p| !p.as_os_str().is_empty())
}

fn current_log_is_drained(log_path: &Path, offset: u64) -> bool {
    match std::fs::metadata(log_path) {
        Ok(meta) => meta.len() <= offset,
        Err(_) => false,
    }
}

fn current_log_has_unread_data(log_path: &Path, offset: u64) -> bool {
    match std::fs::metadata(log_path) {
        Ok(meta) => meta.len() > offset,
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Session refresh / anchor fallback scanning
// ---------------------------------------------------------------------------

fn refresh_runtime_state(submission: &ProviderSubmission, _now: &str) -> HashMap<String, Value> {
    let state = &submission.runtime_state;
    if get_str(state, "mode") != "active" {
        return state.clone();
    }
    let work_dir = match submission_work_dir(submission, state) {
        Some(p) => p,
        None => return state.clone(),
    };
    let session = match load_project_session(&work_dir, None) {
        Some(s) => s,
        None => return state.clone(),
    };

    let mut runtime_state = state.clone();
    if let Some(root) = codex_session_root_path(&session.data) {
        runtime_state.insert(
            "codex_session_root".to_string(),
            Value::String(root.to_string_lossy().to_string()),
        );
    }

    let current_log = match current_session_log(&session) {
        Some(p) => p,
        None => return runtime_state,
    };

    let mut poll_state = state
        .get("state")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let poll_state_log_str =
        normalized_path_string(poll_state.get("log_path").unwrap_or(&Value::Null));

    let fallback_log = active_anchor_fallback_log(state).or_else(|| {
        anchor_fallback_log(
            submission,
            &runtime_state,
            &poll_state,
            &session,
            &work_dir,
            &current_log,
        )
    });

    let target_log = if let Some(fallback) = fallback_log {
        let fallback_str = normalized_path_string_path(&fallback);
        if fallback_str != poll_state_log_str {
            runtime_state.insert(
                "codex_anchor_fallback_log".to_string(),
                Value::String(fallback_str.clone()),
            );
            if let Some(session_id) = extract_session_id(&fallback) {
                runtime_state.insert(
                    "codex_anchor_fallback_session_id".to_string(),
                    Value::String(session_id),
                );
            }
        }
        fallback
    } else {
        current_log
    };

    let target_log_str = normalized_path_string_path(&target_log);
    if target_log_str != poll_state_log_str {
        poll_state.insert("log_path".to_string(), Value::String(target_log_str));
        poll_state.insert("offset".to_string(), Value::Number(0.into()));
        poll_state.insert("last_rescan".to_string(), Value::Number(0.into()));
        runtime_state.insert("state".to_string(), Value::Object(poll_state));
    }

    runtime_state
}

fn active_anchor_fallback_log(state: &HashMap<String, Value>) -> Option<PathBuf> {
    let raw = get_str(state, "codex_anchor_fallback_log");
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(expand_tilde(&raw));
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn anchor_fallback_log(
    submission: &ProviderSubmission,
    state: &HashMap<String, Value>,
    poll_state: &Map<String, Value>,
    session: &CodexProjectSession,
    work_dir: &Path,
    current_log: &Path,
) -> Option<PathBuf> {
    if get_bool(state, "anchor_seen") || get_bool(state, "no_wrap") {
        return None;
    }
    let offset = poll_state
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if current_log_has_unread_data(current_log, offset) {
        return None;
    }
    let request_anchor = request_anchor_from_runtime_state(state, &submission.job_id);
    if request_anchor.is_empty() {
        return None;
    }
    let root = codex_session_root_path(&session.data)?;
    if !root.is_dir() {
        return None;
    }
    let target_work_dir = normalize_work_dir(work_dir)?;

    let current_path = normalized_resolved_path(current_log);
    let mut matches: Vec<PathBuf> = Vec::new();
    let candidates = walk_jsonl_files(&root);
    for candidate in candidates {
        if normalized_resolved_path(&candidate) == current_path {
            continue;
        }
        if !log_matches_work_dir(&candidate, &target_work_dir) {
            continue;
        }
        if extract_session_id(&candidate).is_none() {
            continue;
        }
        if log_contains_request_anchor(&candidate, &request_anchor) {
            matches.push(candidate);
            if matches.len() > 1 {
                return None;
            }
        }
    }
    if matches.len() != 1 {
        return None;
    }
    matches.into_iter().next()
}

fn walk_jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(dir) = queue.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                queue.push(path);
            } else if meta.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

fn log_matches_work_dir(log_path: &Path, target_work_dir: &str) -> bool {
    let raw = extract_cwd_from_log_file(log_path);
    let raw = match raw {
        Some(r) => r,
        None => return false,
    };
    let normalized = normalize_work_dir(&PathBuf::from(expand_tilde(&raw)));
    normalized.as_deref() == Some(target_work_dir)
}

fn log_contains_request_anchor(log_path: &Path, request_anchor: &str) -> bool {
    let needle = format!("{} {}", REQ_ID_PREFIX, request_anchor);
    let file = match File::open(log_path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if line.contains(&needle) {
            return true;
        }
    }
    false
}

fn extract_cwd_from_log_file(log_path: &Path) -> Option<String> {
    let file = File::open(log_path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(&line).ok()?;
    let entry_type = value.get("type").and_then(|v| v.as_str())?;
    if entry_type != "session_meta" {
        return None;
    }
    let cwd = value
        .get("payload")
        .and_then(|v| v.as_object())
        .and_then(|p| p.get("cwd"))
        .and_then(|v| v.as_str())?;
    let cwd = cwd.trim();
    if cwd.is_empty() {
        None
    } else {
        Some(cwd.to_string())
    }
}

fn extract_session_id(log_path: &Path) -> Option<String> {
    let name = log_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if let Some(m) = regex::Regex::new(SESSION_ID_PATTERN).ok()?.find(name) {
        return Some(m.as_str().to_lowercase());
    }
    let file = File::open(log_path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.is_empty() {
        return None;
    }
    let m = regex::Regex::new(SESSION_ID_PATTERN).ok()?.find(&line)?;
    Some(m.as_str().to_lowercase())
}

fn normalize_work_dir(work_dir: &Path) -> Option<String> {
    let expanded = PathBuf::from(expand_tilde(work_dir.to_str().unwrap_or("")));
    let canonical = expanded.canonicalize().unwrap_or(expanded);
    let s = canonical.to_string_lossy().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn codex_session_root_path(data: &HashMap<String, Value>) -> Option<PathBuf> {
    if let Some(raw) = data
        .get("codex_session_root")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(expand_tilde(raw)));
    }
    if let Some(raw) = data
        .get("codex_home")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(expand_tilde(raw)).join("sessions"));
    }
    if let Some(root) = codex_session_root_from_start_cmd(data) {
        return Some(root);
    }
    if let Some(raw) = data
        .get("codex_session_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let path = PathBuf::from(expand_tilde(raw));
        for parent in
            std::iter::once(path.parent().unwrap_or(&path)).chain(path.ancestors().skip(1))
        {
            if parent.file_name().and_then(|s| s.to_str()) == Some("sessions") {
                return Some(parent.to_path_buf());
            }
        }
    }
    let default = PathBuf::from(std::env::var("CODEX_SESSION_ROOT").unwrap_or_else(|_| {
        format!(
            "{}/.codex/sessions",
            std::env::var("HOME").unwrap_or_default()
        )
    }));
    Some(default)
}

fn codex_session_root_from_start_cmd(data: &HashMap<String, Value>) -> Option<PathBuf> {
    for key in ["codex_start_cmd", "start_cmd"] {
        let Some(cmd) = data.get(key).and_then(Value::as_str) else {
            continue;
        };
        if let Some(value) = extract_env_assignment(cmd, "CODEX_SESSION_ROOT") {
            return Some(PathBuf::from(expand_tilde(&value)));
        }
        if let Some(value) = extract_env_assignment(cmd, "CODEX_HOME") {
            return Some(PathBuf::from(expand_tilde(&value)).join("sessions"));
        }
    }
    None
}

fn extract_env_assignment(command: &str, name: &str) -> Option<String> {
    // Mirror Python _ENV_ASSIGNMENT_RE:
    // (?:^|[\s;])(?:export\s+)?NAME=('[^']*'|"[^"]*"|[^\s;]+)
    let pattern = format!(
        r#"(?:^|[\s;])(?:export\s+)?{}=('[^']*'|"[^"]*"|[^\s;]+)"#,
        regex::escape(name)
    );
    let re = regex::Regex::new(&pattern).ok()?;
    let caps = re.captures(command)?;
    let value = caps.get(1)?.as_str();
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'')
        || value.starts_with('"') && value.ends_with('"')
    {
        return Some(value[1..value.len() - 1].to_string());
    }
    Some(value.to_string())
}

fn normalized_path_string(value: &Value) -> String {
    match value {
        Value::String(s) => normalized_path_string_str(s),
        _ => String::new(),
    }
}

fn normalized_path_string_str(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    expand_tilde(value)
}

fn normalized_path_string_path(value: &Path) -> String {
    normalized_path_string_str(value.to_string_lossy().as_ref())
}

fn normalized_resolved_path(value: &Path) -> String {
    value
        .canonicalize()
        .unwrap_or_else(|_| value.to_path_buf())
        .to_string_lossy()
        .to_string()
}

// ---------------------------------------------------------------------------
// Log reading / entry extraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct CodexLogEntry {
    role: String,
    text: String,
    payload_type: String,
    phase: String,
    turn_id: String,
    task_id: String,
    reason: String,
    last_agent_message: String,
    timestamp: String,
}

fn read_log_entries(path: &Path, offset: u64) -> std::io::Result<(Vec<CodexLogEntry>, u64)> {
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset))?;
    let mut entries = Vec::new();
    let mut line = String::new();
    loop {
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if let Some(entry) = extract_entry(&value) {
                entries.push(entry);
            }
        }
        line.clear();
    }
    let new_offset = reader.stream_position()?;
    Ok((entries, new_offset))
}

fn extract_entry(value: &Value) -> Option<CodexLogEntry> {
    let empty = Map::new();
    let entry_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let payload = value
        .get("payload")
        .and_then(|v| v.as_object())
        .unwrap_or(&empty);
    let payload_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let base = CodexLogEntry {
        payload_type: payload_type.to_string(),
        phase: payload
            .get("phase")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        turn_id: payload
            .get("turn_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        task_id: payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        reason: payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        timestamp: value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        ..Default::default()
    };

    if entry_type == "response_item" && payload_type == "message" {
        return response_message_entry(base, value, payload);
    }
    if entry_type != "event_msg" {
        return fallback_entry(base, value, payload);
    }
    event_message_entry(base, value, payload)
}

fn response_message_entry(
    mut base: CodexLogEntry,
    entry: &Value,
    payload: &Map<String, Value>,
) -> Option<CodexLogEntry> {
    let role = payload
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    base.role = role.clone();
    let text = if role == "user" {
        extract_user_message(entry, payload)
    } else {
        extract_message(entry, payload)
    };
    base.text = text?;
    Some(base)
}

fn event_message_entry(
    mut base: CodexLogEntry,
    entry: &Value,
    payload: &Map<String, Value>,
) -> Option<CodexLogEntry> {
    let payload_type = base.payload_type.as_str();
    if payload_type == "user_message" {
        base.role = "user".to_string();
        base.text = extract_user_message(entry, payload)?;
        return Some(base);
    }
    if matches!(
        payload_type,
        "agent_message" | "assistant_message" | "assistant" | "assistant_response" | "message"
    ) {
        let role = payload_role(payload);
        base.role = if role == "user" {
            "user".to_string()
        } else {
            "assistant".to_string()
        };
        base.text = if role == "user" {
            extract_user_message(entry, payload)?
        } else {
            extract_message(entry, payload)?
        };
        return Some(base);
    }
    if payload_type == "task_complete" {
        base.role = "system".to_string();
        base.last_agent_message = payload
            .get("last_agent_message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Some(base);
    }
    if payload_type == "turn_aborted" {
        base.role = "system".to_string();
        base.text = payload
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        base.reason = if base.reason.is_empty() {
            "turn_aborted".to_string()
        } else {
            base.reason
        };
        return Some(base);
    }
    fallback_entry(base, entry, payload)
}

fn fallback_entry(
    mut base: CodexLogEntry,
    entry: &Value,
    payload: &Map<String, Value>,
) -> Option<CodexLogEntry> {
    if let Some(text) = extract_user_message(entry, payload) {
        base.role = "user".to_string();
        base.text = text;
        return Some(base);
    }
    if let Some(text) = extract_message(entry, payload) {
        let role = payload_role(payload);
        base.role = if role.is_empty() {
            "assistant".to_string()
        } else {
            role
        };
        base.text = text;
        return Some(base);
    }
    None
}

fn payload_role(payload: &Map<String, Value>) -> String {
    payload
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

fn extract_message(entry: &Value, payload: &Map<String, Value>) -> Option<String> {
    let entry_type = entry
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if entry_type == "response_item" {
        return response_item_message(payload);
    }
    if entry_type == "event_msg" {
        return event_message(payload);
    }
    if payload_role(payload) == "assistant" {
        return first_nonempty_text3(
            payload.get("message"),
            payload.get("content"),
            payload.get("text"),
        );
    }
    None
}

fn extract_user_message(entry: &Value, payload: &Map<String, Value>) -> Option<String> {
    let entry_type = entry
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if entry_type == "event_msg"
        && payload.get("type").and_then(|v| v.as_str()) == Some("user_message")
    {
        return first_nonempty_text(payload.get("message"));
    }
    if entry_type == "response_item"
        && payload.get("type").and_then(|v| v.as_str()) == Some("message")
        && payload_role(payload) == "user"
    {
        return join_response_item_user_text(payload.get("content"));
    }
    None
}

fn response_item_message(payload: &Map<String, Value>) -> Option<String> {
    if payload.get("type").and_then(|v| v.as_str()) != Some("message")
        || payload_role(payload) == "user"
    {
        return None;
    }
    if let Some(content) = payload.get("content").and_then(|v| v.as_array()) {
        let text = join_response_item_assistant_text(content);
        if !text.is_empty() {
            return Some(text);
        }
    } else if let Some(text) = payload
        .get("content")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
    {
        if !text.is_empty() {
            return Some(text);
        }
    }
    first_nonempty_text(payload.get("message"))
}

fn event_message(payload: &Map<String, Value>) -> Option<String> {
    let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(
        payload_type,
        "agent_message" | "assistant_message" | "assistant" | "assistant_response" | "message"
    ) {
        return None;
    }
    if payload_role(payload) == "user" {
        return None;
    }
    first_nonempty_text3(
        payload.get("message"),
        payload.get("content"),
        payload.get("text"),
    )
}

fn join_response_item_assistant_text(content: &[Value]) -> String {
    let mut texts = Vec::new();
    for item in content {
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        let item_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !matches!(item_type, "output_text" | "text") {
            continue;
        }
        if let Some(text) = obj
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() {
        String::new()
    } else {
        texts.join("\n").trim().to_string()
    }
}

fn join_response_item_user_text(content: Option<&Value>) -> Option<String> {
    let content = content?.as_array()?;
    let mut texts = Vec::new();
    for item in content {
        let obj = item.as_object()?;
        if obj.get("type").and_then(|v| v.as_str()) != Some("input_text") {
            continue;
        }
        if let Some(text) = obj
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n").trim().to_string())
    }
}

fn first_nonempty_text(values: Option<&Value>) -> Option<String> {
    values
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn first_nonempty_text3(a: Option<&Value>, b: Option<&Value>, c: Option<&Value>) -> Option<String> {
    first_nonempty_text(a)
        .or_else(|| first_nonempty_text(b))
        .or_else(|| first_nonempty_text(c))
}

fn state_session_path(state: &Value) -> Option<String> {
    state
        .as_object()
        .and_then(|obj| obj.get("log_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Communicator / FIFO helpers
// ---------------------------------------------------------------------------

/// Minimal Codex communicator that writes turn prompts to a FIFO input file.
#[derive(Debug, Clone)]
pub struct CodexCommunicator {
    pub input_fifo: PathBuf,
}

impl CodexCommunicator {
    pub fn new(input_fifo: impl Into<PathBuf>) -> Self {
        Self {
            input_fifo: input_fifo.into(),
        }
    }

    /// Send a prompt asynchronously (non-blocking open best-effort).
    pub fn send_async(&self, message: &str) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(false)
            .open(&self.input_fifo)?;
        file.write_all(message.as_bytes())?;
        if !message.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        Ok(())
    }

    /// Wrap a message with the Codex begin/done markers for a full turn.
    pub fn wrap_turn_prompt(&self, message: &str, req_id: &str) -> String {
        ccb_provider_core::protocol::wrap_codex_turn_prompt(message, req_id)
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn get_str(state: &HashMap<String, Value>, key: &str) -> String {
    state
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn get_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn get_u64(state: &HashMap<String, Value>, key: &str, default: u64) -> u64 {
    state.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, rest);
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_entry_user_message() {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": { "type": "user_message", "message": "hello" }
        });
        let entry = extract_entry(&value).unwrap();
        assert_eq!(entry.role, "user");
        assert_eq!(entry.text, "hello");
    }

    #[test]
    fn test_extract_entry_assistant_response_item() {
        let value = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": "done" }]
            }
        });
        let entry = extract_entry(&value).unwrap();
        assert_eq!(entry.role, "assistant");
        assert_eq!(entry.text, "done");
    }

    #[test]
    fn test_extract_entry_task_complete() {
        let value = serde_json::json!({
            "type": "event_msg",
            "payload": { "type": "task_complete", "last_agent_message": "final" }
        });
        let entry = extract_entry(&value).unwrap();
        assert_eq!(entry.role, "system");
        assert_eq!(entry.last_agent_message, "final");
    }

    #[test]
    fn test_select_reply_prefers_agent_message() {
        assert_eq!(
            select_reply("agent", "final", "assistant", "buffer"),
            "agent"
        );
    }

    #[test]
    fn test_abort_status_detects_cancel() {
        assert_eq!(abort_status("user cancelled"), "cancelled");
        assert_eq!(abort_status("something broke"), "failed");
    }

    #[test]
    fn test_codex_session_root_path_prefers_explicit_fields() {
        let mut data = HashMap::new();
        data.insert(
            "codex_session_root".to_string(),
            Value::String("/tmp/codex-root".to_string()),
        );
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/tmp/codex-root"))
        );

        let mut data = HashMap::new();
        data.insert(
            "codex_home".to_string(),
            Value::String("/home/user/.codex".to_string()),
        );
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/home/user/.codex/sessions"))
        );
    }

    #[test]
    fn test_codex_session_root_path_derives_from_session_path() {
        let mut data = HashMap::new();
        data.insert(
            "codex_session_path".to_string(),
            Value::String("/home/user/.codex/sessions/proj/session.jsonl".to_string()),
        );
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/home/user/.codex/sessions"))
        );
    }

    #[test]
    fn test_codex_session_root_path_defaults_to_env_or_home() {
        let home = std::env::var("HOME").ok();
        let root_env = std::env::var("CODEX_SESSION_ROOT").ok();

        std::env::remove_var("CODEX_SESSION_ROOT");
        std::env::set_var("HOME", "/tmp/fakehome");
        let data = HashMap::new();
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/tmp/fakehome/.codex/sessions"))
        );

        std::env::set_var("CODEX_SESSION_ROOT", "/custom/sessions");
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/custom/sessions"))
        );

        match home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match root_env {
            Some(v) => std::env::set_var("CODEX_SESSION_ROOT", v),
            None => std::env::remove_var("CODEX_SESSION_ROOT"),
        }
    }

    #[test]
    fn test_codex_session_root_path_extracts_from_start_cmd() {
        let mut data = HashMap::new();
        data.insert(
            "codex_start_cmd".to_string(),
            Value::String("export CODEX_SESSION_ROOT='/legacy/sessions'; codex".to_string()),
        );
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/legacy/sessions"))
        );

        let mut data = HashMap::new();
        data.insert(
            "start_cmd".to_string(),
            Value::String("CODEX_HOME=/legacy/home codex".to_string()),
        );
        assert_eq!(
            codex_session_root_path(&data),
            Some(PathBuf::from("/legacy/home/sessions"))
        );
    }

    #[test]
    fn test_extract_env_assignment_parses_quoted_and_unquoted() {
        assert_eq!(
            extract_env_assignment("export FOO='/a b/c'", "FOO"),
            Some("/a b/c".to_string())
        );
        assert_eq!(
            extract_env_assignment("FOO=\"/a b/c\"", "FOO"),
            Some("/a b/c".to_string())
        );
        assert_eq!(
            extract_env_assignment("FOO=/a/b", "FOO"),
            Some("/a/b".to_string())
        );
        assert_eq!(extract_env_assignment("BAR=/a/b", "FOO"), None);
    }
}
