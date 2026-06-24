use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::ProviderBackend;
use ccb_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccb_provider_core::protocol;
use ccb_provider_core::runtime_shared::provider_start_parts;
use serde_json::Value;

use crate::execution::{
    build_item, error_submission, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext,
    ProviderSubmission,
};
use crate::mimo::{
    build_runtime_launcher, build_session_binding, load_project_session, wrap_mimo_prompt,
};

pub const PROVIDER_NAME: &str = "mimo";

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;

/// Build the Mimo provider manifest.
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
        true,  // supports_subagents
        true,  // supports_workspace_attach
        profiles,
    )
}

/// Build the full Mimo provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        // The execution adapter is registered with the ccb-providers execution
        // registry rather than the ccb-provider-core backend slot because the
        // two crates currently define distinct ExecutionAdapter traits.
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(build_runtime_launcher()),
    }
}

/// Mimo provider execution adapter.
pub struct MimoExecutionAdapter;

impl ExecutionAdapter for MimoExecutionAdapter {
    fn provider(&self) -> &str {
        PROVIDER_NAME
    }

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        start_mimo_run_submission(job, context, now)
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

fn start_mimo_run_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let work_dir = resolve_work_dir(job, context);
    if work_dir.as_os_str().is_empty() || !work_dir.exists() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::StructuredResultStream,
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
                CompletionSourceKind::StructuredResultStream,
                "runtime_unavailable",
                "mimo_session_file_missing",
            );
        }
    };

    let runtime_dir = session
        .data
        .get("runtime_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| work_dir.join(".ccb").join("runtime").join(PROVIDER_NAME));
    let completion_dir = session
        .data
        .get("completion_artifact_dir")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| runtime_dir.join("completion"));
    if let Err(e) = std::fs::create_dir_all(&completion_dir) {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::StructuredResultStream,
            "runtime_unavailable",
            format!("create_completion_dir_failed:{e}"),
        );
    }

    let stdout_path = completion_dir.join(format!("{}.mimo-run.jsonl", job.job_id));
    let stderr_path = completion_dir.join(format!("{}.mimo-run.stderr.log", job.job_id));

    let request_anchor = protocol::request_anchor_for_job(&job.job_id);
    let prompt = wrap_mimo_prompt(&job.request.body, &request_anchor);

    let mut cmd_parts = provider_start_parts(PROVIDER_NAME);
    cmd_parts.extend([
        "run".to_string(),
        "--pure".to_string(),
        "--format".to_string(),
        "json".to_string(),
        "--dir".to_string(),
        work_dir.to_string_lossy().to_string(),
        prompt.clone(),
    ]);

    let mut env = session_env(&session.data);
    env.insert("CCB_REQ_ID".to_string(), request_anchor.clone());

    let stdout_file = match std::fs::File::create(&stdout_path) {
        Ok(f) => f,
        Err(e) => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::StructuredResultStream,
                "mimo_run_start_failed",
                format!("create_stdout_failed:{e}"),
            );
        }
    };
    let stderr_file = match std::fs::File::create(&stderr_path) {
        Ok(f) => f,
        Err(e) => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::StructuredResultStream,
                "mimo_run_start_failed",
                format!("create_stderr_failed:{e}"),
            );
        }
    };

    let mut command = Command::new(&cmd_parts[0]);
    command
        .args(&cmd_parts[1..])
        .current_dir(&work_dir)
        .envs(&env)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .process_group(0);

    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::StructuredResultStream,
                "mimo_run_start_failed",
                format!("{e}"),
            );
        }
    };

    let pid = child.id();
    // Child is detached; we don't hold the handle so the process can outlive this function.
    std::mem::forget(child);

    let diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "mimo_run",
        "workspace_path": work_dir.to_string_lossy().to_string(),
        "stdout_path": stdout_path.to_string_lossy().to_string(),
        "stderr_path": stderr_path.to_string_lossy().to_string(),
        "pid": pid,
    });

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("mimo_run".to_string()));
    runtime_state.insert(
        "provider".to_string(),
        Value::String(PROVIDER_NAME.to_string()),
    );
    runtime_state.insert("job_id".to_string(), Value::String(job.job_id.clone()));
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert(
        "work_dir".to_string(),
        Value::String(work_dir.to_string_lossy().to_string()),
    );
    runtime_state.insert("started_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_emitted".to_string(), Value::Bool(false));
    runtime_state.insert("turn_boundary_emitted".to_string(), Value::Bool(false));
    runtime_state.insert("no_wrap".to_string(), Value::Bool(false));
    runtime_state.insert("pure_mode".to_string(), Value::Bool(true));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert(
        "stdout_path".to_string(),
        Value::String(stdout_path.to_string_lossy().to_string()),
    );
    runtime_state.insert(
        "stderr_path".to_string(),
        Value::String(stderr_path.to_string_lossy().to_string()),
    );
    runtime_state.insert("pid".to_string(), Value::Number(pid.into()));
    runtime_state.insert("returncode".to_string(), Value::Null);
    runtime_state.insert(
        "mimo_home".to_string(),
        Value::String(env.get("MIMOCODE_HOME").cloned().unwrap_or_default()),
    );
    runtime_state.insert(
        "mimo_config_path".to_string(),
        Value::String(session.mimo_config_path()),
    );
    runtime_state.insert(
        "prompt_sha256".to_string(),
        Value::String(sha256_hex(&prompt)),
    );

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider: PROVIDER_NAME.to_string(),
        accepted_at: now.to_string(),
        ready_at: now.to_string(),
        source_kind: CompletionSourceKind::StructuredResultStream,
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

    let mode = submission
        .runtime_state
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if mode == "passive" || mode == "error" {
        return Some(runtime_error_result(
            submission,
            now,
            &runtime_str(&submission.runtime_state, "reason").unwrap_or_default(),
            &runtime_str(&submission.runtime_state, "error").unwrap_or_default(),
        ));
    }
    if mode != "mimo_run" {
        return Some(runtime_error_result(
            submission,
            now,
            "runtime_state_corrupt",
            "",
        ));
    }

    let mut state = submission.runtime_state.clone();
    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    let next_seq = runtime_u64(&state, "next_seq").max(1);
    state.insert("next_seq".to_string(), Value::Number(next_seq.into()));

    let stdout_path = runtime_str(&state, "stdout_path").unwrap_or_default();
    let observation = read_mimo_run_output(&stdout_path);

    let request_anchor =
        runtime_str(&state, "request_anchor").unwrap_or_else(|| submission.job_id.clone());
    let mut items = Vec::new();

    if !runtime_bool(&state, "anchor_emitted") {
        let mut payload = HashMap::new();
        payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
        payload.insert(
            "source".to_string(),
            Value::String("mimo_run_prompt_submitted".to_string()),
        );
        items.push(build_item(
            submission,
            CompletionItemKind::AnchorSeen,
            now,
            next_seq,
            payload,
        ));
        state.insert("anchor_emitted".to_string(), Value::Bool(true));
    }

    let reply = observation.text.trim().to_string();
    if !reply.is_empty() && reply != runtime_str(&state, "reply_buffer").unwrap_or_default() {
        state.insert("reply_buffer".to_string(), Value::String(reply.clone()));
        let mut payload = HashMap::new();
        payload.insert("text".to_string(), Value::String(reply.clone()));
        payload.insert("reply".to_string(), Value::String(reply.clone()));
        payload.insert("final_answer".to_string(), Value::String(reply.clone()));
        payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
        payload.insert(
            "provider_turn_ref".to_string(),
            observation
                .turn_ref
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "completed_at".to_string(),
            observation.completed_at.clone().unwrap_or(Value::Null),
        );
        payload.insert(
            "finish_reason".to_string(),
            observation
                .finish_reason
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        items.push(build_item(
            submission,
            CompletionItemKind::AssistantFinal,
            now,
            next_seq + 1,
            payload,
        ));
    }

    let terminal = terminal_result_if_ready(submission, &mut state, &observation, &items, now);
    if terminal.is_some() {
        return terminal;
    }

    if items.is_empty() {
        return None;
    }

    let mut updated = submission.clone();
    updated.reply = runtime_str(&state, "reply_buffer").unwrap_or_default();
    updated.runtime_state = state;
    Some(ProviderPollResult::new(updated, items, None))
}

fn terminal_result_if_ready(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    observation: &MimoRunObservation,
    items: &[ccb_completion::models::CompletionItem],
    now: &str,
) -> Option<ProviderPollResult> {
    if !observation.error.is_empty() {
        return Some(terminal_result(
            submission,
            state,
            CompletionStatus::Failed,
            "mimo_run_error",
            &runtime_str(state, "reply_buffer").unwrap_or_default(),
            CompletionConfidence::Degraded,
            now,
            Some(serde_json::json!({"error": observation.error.clone()})),
        ));
    }

    let reply = runtime_str(state, "reply_buffer").unwrap_or_default();

    if observation.finished {
        let (status, reason, confidence) =
            if observation.finish_reason.as_deref().is_some_and(|r| {
                !matches!(
                    r.trim().to_lowercase().as_str(),
                    "stop" | "end_turn" | "completed"
                )
            }) {
                (
                    CompletionStatus::Incomplete,
                    format!(
                        "mimo_run_finished:{}",
                        observation.finish_reason.as_deref().unwrap_or("unknown")
                    ),
                    CompletionConfidence::Observed,
                )
            } else if reply.is_empty() {
                (
                    CompletionStatus::Incomplete,
                    "mimo_run_empty_reply".to_string(),
                    CompletionConfidence::Degraded,
                )
            } else {
                (
                    CompletionStatus::Completed,
                    "mimo_run_stop".to_string(),
                    CompletionConfidence::Observed,
                )
            };

        if !runtime_bool(state, "turn_boundary_emitted") {
            let request_anchor =
                runtime_str(state, "request_anchor").unwrap_or_else(|| submission.job_id.clone());
            let mut payload = HashMap::new();
            payload.insert("reason".to_string(), Value::String(reason.clone()));
            payload.insert(
                "last_agent_message".to_string(),
                Value::String(reply.clone()),
            );
            payload.insert("turn_id".to_string(), Value::String(request_anchor));
            payload.insert(
                "provider_turn_ref".to_string(),
                observation
                    .turn_ref
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            payload.insert(
                "finish_reason".to_string(),
                observation
                    .finish_reason
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            payload.insert(
                "completed_at".to_string(),
                observation.completed_at.clone().unwrap_or(Value::Null),
            );
            let mut next_seq = runtime_u64(state, "next_seq");
            items.iter().for_each(|_| next_seq += 1);
            let boundary_item = build_item(
                submission,
                CompletionItemKind::TurnBoundary,
                now,
                next_seq,
                payload,
            );
            let mut items = items.to_vec();
            items.push(boundary_item);
            state.insert("turn_boundary_emitted".to_string(), Value::Bool(true));
            return Some(terminal_result(
                submission,
                state,
                status,
                &reason,
                &reply,
                confidence,
                now,
                Some(serde_json::json!({
                    "finish_reason": observation.finish_reason,
                    "stdout_path": runtime_str(state, "stdout_path"),
                    "stderr_path": runtime_str(state, "stderr_path"),
                })),
            ));
        }

        return Some(terminal_result(
            submission,
            state,
            status,
            &reason,
            &reply,
            confidence,
            now,
            Some(serde_json::json!({
                "finish_reason": observation.finish_reason,
                "stdout_path": runtime_str(state, "stdout_path"),
                "stderr_path": runtime_str(state, "stderr_path"),
            })),
        ));
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn terminal_result(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    status: CompletionStatus,
    reason: &str,
    reply: &str,
    confidence: CompletionConfidence,
    now: &str,
    diagnostics_extra: Option<Value>,
) -> ProviderPollResult {
    let reply = reply.to_string();
    let mut updated = submission.clone();
    updated.status = status;
    updated.reason = reason.to_string();
    updated.reply = reply.clone();
    updated.confidence = confidence;

    let next_seq = runtime_u64(state, "next_seq");
    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(next_seq),
        updated_at: Some(now.to_string()),
        ..Default::default()
    };

    let mut diagnostics: serde_json::Map<String, Value> = serde_json::json!({
        "mode": "mimo_run",
        "anchor_seen": runtime_bool(state, "anchor_emitted"),
        "reply_chars": reply.len(),
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    if let Some(extra) = diagnostics_extra.and_then(|v| v.as_object().cloned()) {
        diagnostics.extend(extra);
    }

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: reply.clone(),
        anchor_seen: runtime_bool(state, "anchor_emitted"),
        reply_started: !reply.is_empty(),
        reply_stable: !reply.is_empty() && status == CompletionStatus::Completed,
        provider_turn_ref: Some(
            runtime_str(state, "request_anchor").unwrap_or_else(|| submission.job_id.clone()),
        ),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

fn runtime_error_result(
    submission: &ProviderSubmission,
    now: &str,
    reason: &str,
    error: &str,
) -> ProviderPollResult {
    terminal_result(
        submission,
        &mut submission.runtime_state.clone(),
        CompletionStatus::Failed,
        reason,
        "",
        CompletionConfidence::Degraded,
        now,
        Some(serde_json::json!({"error": error})),
    )
}

#[derive(Debug, Clone, Default)]
struct MimoRunObservation {
    text: String,
    finished: bool,
    finish_reason: Option<String>,
    turn_ref: Option<String>,
    completed_at: Option<Value>,
    error: String,
}

fn read_mimo_run_output(path: &str) -> MimoRunObservation {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return MimoRunObservation::default();
    }
    let raw = match std::fs::read(&path) {
        Ok(r) => r,
        Err(e) => {
            return MimoRunObservation {
                error: format!("read_stdout_failed:{e}"),
                ..Default::default()
            };
        }
    };
    let text = String::from_utf8_lossy(&raw);

    let mut chunks: Vec<String> = Vec::new();
    let mut finished = false;
    let mut finish_reason: Option<String> = None;
    let mut turn_ref: Option<String> = None;
    let mut completed_at: Option<Value> = None;
    let mut error = String::new();

    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let event: Value = match serde_json::from_str(stripped) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = match event.as_object() {
            Some(o) => o,
            None => continue,
        };

        let top_type = event_type(event);
        let nested_type = event
            .get("part")
            .and_then(|v| v.as_object())
            .map(event_type)
            .unwrap_or_default();
        let effective_type = if !top_type.is_empty() {
            top_type
        } else {
            nested_type
        };

        match effective_type.as_str() {
            "text" => {
                let text = event_text(event);
                if !text.is_empty() {
                    chunks.push(text);
                }
                turn_ref = turn_ref.or_else(|| event_ref(event));
                completed_at = completed_at
                    .clone()
                    .or_else(|| event.get("time").cloned())
                    .or_else(|| event.get("timestamp").cloned());
            }
            "step_finish" | "turn_finish" | "finish" | "done" => {
                let reason = event_reason(event)
                    .or_else(|| finish_reason.clone())
                    .unwrap_or_else(|| "stop".to_string());
                if is_intermediate_finish_reason(&reason) {
                    finish_reason = Some(reason);
                    turn_ref = turn_ref.or_else(|| event_ref(event));
                    completed_at = completed_at
                        .clone()
                        .or_else(|| event.get("time").cloned())
                        .or_else(|| event.get("timestamp").cloned());
                    continue;
                }
                finished = true;
                finish_reason = Some(reason);
                turn_ref = turn_ref.or_else(|| event_ref(event));
                completed_at = completed_at
                    .clone()
                    .or_else(|| event.get("time").cloned())
                    .or_else(|| event.get("timestamp").cloned());
            }
            "error" | "failed" => {
                error = event_text(event);
                if error.is_empty() {
                    error = event_reason(event).unwrap_or_else(|| "mimo_run_error".to_string());
                }
            }
            _ => {}
        }
    }

    MimoRunObservation {
        text: chunks.join(""),
        finished,
        finish_reason,
        turn_ref,
        completed_at,
        error,
    }
}

fn is_intermediate_finish_reason(reason: &str) -> bool {
    matches!(
        reason.trim().to_lowercase().replace('-', "_").as_str(),
        "tool_calls" | "tool_call"
    )
}

fn event_type(event: &serde_json::Map<String, Value>) -> String {
    event
        .get("type")
        .or_else(|| event.get("event"))
        .or_else(|| event.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .replace('-', "_")
}

fn event_text(event: &serde_json::Map<String, Value>) -> String {
    for key in ["text", "content", "message"] {
        if let Some(Value::String(s)) = event.get(key) {
            if !s.is_empty() {
                return s.clone();
            }
        }
    }
    if let Some(payload) = event.get("payload").and_then(|v| v.as_object()) {
        let text = event_text(payload);
        if !text.is_empty() {
            return text;
        }
    }
    if let Some(part) = event.get("part").and_then(|v| v.as_object()) {
        let text = event_text(part);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

fn event_reason(event: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["reason", "finish_reason", "stop_reason", "status"] {
        if let Some(Value::String(s)) = event.get(key) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    for key in ["payload", "properties", "part"] {
        if let Some(nested) = event.get(key).and_then(|v| v.as_object()) {
            if let Some(reason) = event_reason(nested) {
                return Some(reason);
            }
        }
    }
    None
}

fn event_ref(event: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["id", "message_id", "messageID", "session_id", "sessionID"] {
        if let Some(Value::String(s)) = event.get(key) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    if let Some(payload) = event.get("payload").and_then(|v| v.as_object()) {
        if let Some(r) = event_ref(payload) {
            return Some(r);
        }
    }
    if let Some(part) = event.get("part").and_then(|v| v.as_object()) {
        if let Some(r) = event_ref(part) {
            return Some(r);
        }
    }
    None
}

fn session_env(session_data: &HashMap<String, Value>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    let mimo_home = session_data
        .get("mimo_home")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .unwrap_or("");
    if !mimo_home.is_empty() {
        std::fs::create_dir_all(mimo_home).ok();
        env.insert("MIMOCODE_HOME".to_string(), mimo_home.to_string());
    }
    let config_path = session_data
        .get("mimo_config_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .unwrap_or("");
    if !config_path.is_empty() && Path::new(config_path).is_file() {
        env.insert("MIMOCODE_CONFIG".to_string(), config_path.to_string());
    }
    env.insert(
        "MIMOCODE_DISABLE_AUTOUPDATE".to_string(),
        "true".to_string(),
    );
    env.insert("MIMOCODE_ENABLE_ANALYSIS".to_string(), "false".to_string());
    env
}

fn resolve_work_dir(_job: &JobRecord, context: Option<&ProviderRuntimeContext>) -> PathBuf {
    let candidate = context
        .and_then(|c| c.workspace_path.as_deref())
        .unwrap_or("");
    PathBuf::from(candidate)
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn runtime_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn runtime_u64(state: &HashMap<String, Value>, key: &str) -> u64 {
    state.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn runtime_str(state: &HashMap<String, Value>, key: &str) -> Option<String> {
    state
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, content: Value) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, serde_json::to_string(&content).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_manifest() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
        assert!(m.supports_runtime_mode(&ccb_provider_core::manifest::RuntimeMode::PaneBacked));
    }

    #[test]
    fn test_backend_has_session_binding_and_launcher() {
        let b = backend();
        assert_eq!(b.provider(), PROVIDER_NAME);
        assert!(b.session_binding.is_some());
        assert!(b.runtime_launcher.is_some());
        assert!(b.execution_adapter.is_none());
    }

    #[test]
    fn test_execution_adapter_provider_name() {
        let adapter = MimoExecutionAdapter;
        assert_eq!(adapter.provider(), PROVIDER_NAME);
    }

    #[test]
    fn test_start_fails_without_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let adapter = MimoExecutionAdapter;
        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let ctx = ProviderRuntimeContext {
            workspace_path: Some(work_dir.to_string_lossy().to_string()),
            ..Default::default()
        };
        let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
        assert!(submission
            .runtime_state
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("mimo_session_file_missing"));
    }

    #[test]
    fn test_poll_reads_stdout_jsonl() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".mimo-agent2-session",
            serde_json::json!({
                "mimo_session_id": "session-1",
                "runtime_dir": tmp.path().join("runtime").to_string_lossy().to_string(),
                "completion_artifact_dir": tmp.path().join("completion").to_string_lossy().to_string(),
            }),
        );
        std::fs::create_dir(tmp.path().join("completion")).unwrap();

        let stdout_path = tmp.path().join("completion").join("j2.mimo-run.jsonl");
        let mut file = std::fs::File::create(&stdout_path).unwrap();
        writeln!(file, r#"{{"type":"text","text":"hello"}}"#).unwrap();
        writeln!(file, r#"{{"type":"finish","reason":"stop"}}"#).unwrap();

        let mut state = HashMap::new();
        state.insert("mode".to_string(), Value::String("mimo_run".to_string()));
        state.insert("job_id".to_string(), Value::String("j2".to_string()));
        state.insert(
            "request_anchor".to_string(),
            Value::String("anchor-2".to_string()),
        );
        state.insert(
            "work_dir".to_string(),
            Value::String(work_dir.to_string_lossy().to_string()),
        );
        state.insert(
            "started_at".to_string(),
            Value::String("2025-01-01T00:00:00Z".to_string()),
        );
        state.insert(
            "last_poll_at".to_string(),
            Value::String("2025-01-01T00:00:00Z".to_string()),
        );
        state.insert("next_seq".to_string(), Value::Number(1.into()));
        state.insert("anchor_emitted".to_string(), Value::Bool(false));
        state.insert("turn_boundary_emitted".to_string(), Value::Bool(false));
        state.insert("reply_buffer".to_string(), Value::String(String::new()));
        state.insert(
            "stdout_path".to_string(),
            Value::String(stdout_path.to_string_lossy().to_string()),
        );
        state.insert(
            "stderr_path".to_string(),
            Value::String(tmp.path().join("stderr").to_string_lossy().to_string()),
        );
        state.insert("pid".to_string(), Value::Number(12345.into()));
        state.insert("returncode".to_string(), Value::Null);

        let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
        let submission = ProviderSubmission {
            job_id: job.job_id,
            agent_name: job.agent_name,
            provider: PROVIDER_NAME.to_string(),
            accepted_at: "2025-01-01T00:00:00Z".to_string(),
            ready_at: "2025-01-01T00:00:00Z".to_string(),
            source_kind: CompletionSourceKind::StructuredResultStream,
            reply: String::new(),
            status: CompletionStatus::Incomplete,
            reason: "in_progress".to_string(),
            confidence: CompletionConfidence::Observed,
            diagnostics: None,
            runtime_state: state,
        };

        let adapter = MimoExecutionAdapter;
        let result = adapter
            .poll(&submission, "2025-01-01T00:00:01Z")
            .expect("expected poll result");
        assert!(result.decision.is_some());
        assert_eq!(result.submission.reply, "hello");
    }

    #[test]
    fn test_wrap_mimo_prompt_format() {
        let wrapped = wrap_mimo_prompt("do the thing", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(wrapped.contains("do the thing"));
    }
}
