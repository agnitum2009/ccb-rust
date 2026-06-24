use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use ccbr_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::contracts::ProviderBackend;
use ccbr_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccbr_provider_core::protocol::request_anchor_for_job;
use serde_json::Value;

use crate::deepseek::native_log::{
    observe_deepseek_session, INTERRUPTED_STATUSES, PERMISSION_DENIED_STATUSES,
    TERMINAL_FAILURE_STATUSES, WAITING_USER_STATUSES,
};
use crate::deepseek::{build_runtime_launcher, build_session_binding, load_project_session};
use crate::execution::{
    backend_config_from_session_data, build_item, error_submission, resolve_prompt_target,
    resolve_prompt_target_for_session, store_backend_config, ExecutionAdapter, PromptTarget,
    ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};
use crate::native_cli_support::wrap_native_prompt;

pub use crate::deepseek::load_project_session as load_deepseek_project_session;

pub const PROVIDER_NAME: &str = "deepseek";

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;
const MAX_WAIT_SECS: f64 = 300.0;
const ANCHOR_WAIT_SECS: f64 = 120.0;

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the DeepSeek provider manifest.
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

/// Build the full DeepSeek provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(build_runtime_launcher()),
    }
}

// ---------------------------------------------------------------------------
// Execution adapter
// ---------------------------------------------------------------------------

/// DeepSeek provider execution adapter.
pub struct DeepSeekExecutionAdapter;

impl ExecutionAdapter for DeepSeekExecutionAdapter {
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
                CompletionSourceKind::SessionSnapshot,
                "runtime_unavailable",
                "work_dir_missing",
            );
        }
    };

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
                CompletionSourceKind::SessionSnapshot,
                "runtime_unavailable",
                "deepseek_session_file_missing",
            );
        }
    };

    let pane_id = session.pane_id().unwrap_or("").to_string();
    if pane_id.is_empty() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::SessionSnapshot,
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
                CompletionSourceKind::SessionSnapshot,
                "backend_unavailable",
                "terminal_backend_unavailable",
            );
        }
    };

    let req_id = request_anchor_for_job(&job.job_id);
    let prompt = wrap_native_prompt(&job.request.body, &req_id);

    let send_error = send_prompt(&*target, &pane_id, &prompt);

    let mut diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "native_session_snapshot",
        "pane_id": pane_id,
        "req_id": req_id,
        "message_type": job.request.message_type,
        "workspace_path": work_dir.to_string_lossy().to_string(),
    });
    if let Some(err) = &send_error {
        diagnostics["send_error"] = Value::String(err.clone());
    }

    let mut runtime_state = HashMap::new();
    runtime_state.insert(
        "mode".to_string(),
        Value::String("native_session_snapshot".to_string()),
    );
    runtime_state.insert(
        "provider".to_string(),
        Value::String(PROVIDER_NAME.to_string()),
    );
    store_backend_config(&mut runtime_state, &backend_config);
    runtime_state.insert("pane_id".to_string(), Value::String(pane_id));
    runtime_state.insert("request_anchor".to_string(), Value::String(req_id.clone()));
    runtime_state.insert("req_id".to_string(), Value::String(req_id));
    runtime_state.insert(
        "work_dir".to_string(),
        Value::String(work_dir.to_string_lossy().to_string()),
    );
    runtime_state.insert("started_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(send_error.is_none()));
    if let Some(err) = send_error {
        runtime_state.insert("send_error".to_string(), Value::String(err));
    }
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
        source_kind: CompletionSourceKind::SessionSnapshot,
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
    let req_id = runtime_str(&state, "request_anchor");
    let req_id = if req_id.is_empty() {
        runtime_str(&state, "req_id")
    } else {
        req_id
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

    if resolve_prompt_target(&state).is_none() {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            "runtime_handle_lost",
            "",
            CompletionConfidence::Degraded,
            now,
        ));
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

    let work_dir_path = PathBuf::from(&work_dir);
    let observation = observe_deepseek_session(&work_dir_path, &req_id, None);

    if observation.is_none() {
        if total_secs >= ANCHOR_WAIT_SECS {
            return Some(terminal_result(
                submission,
                &mut state,
                CompletionStatus::Incomplete,
                "deepseek_native_anchor_missing",
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
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let session_path_opt = if session_path.is_empty() {
        None
    } else {
        Some(session_path.clone())
    };

    if !session_path.is_empty() && session_path != runtime_str(&state, "session_path") {
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

    let status = observation.status.to_lowercase();
    let reply = observation.reply.clone();

    if TERMINAL_FAILURE_STATUSES.contains(&status.as_str()) {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            "deepseek_native_failed",
            &reply,
            CompletionConfidence::Observed,
            now,
        ));
    }
    if INTERRUPTED_STATUSES.contains(&status.as_str()) {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Cancelled,
            "deepseek_native_interrupted",
            &reply,
            CompletionConfidence::Observed,
            now,
        ));
    }
    if WAITING_USER_STATUSES.contains(&status.as_str()) {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Incomplete,
            "deepseek_native_waiting_for_user",
            &reply,
            CompletionConfidence::Observed,
            now,
        ));
    }
    if PERMISSION_DENIED_STATUSES.contains(&status.as_str()) {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Incomplete,
            "deepseek_native_permission_denied",
            &reply,
            CompletionConfidence::Observed,
            now,
        ));
    }
    if observation.completed && reply.is_empty() {
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Incomplete,
            "deepseek_native_empty_reply",
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
        payload.insert("native_status".to_string(), Value::String(status.clone()));
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
        .or_else(|| observation.session_id.clone())
        .or_else(|| Some(session_path.clone()))
        .unwrap_or_else(|| req_id.clone());
    if observation.completed && boundary_ref != runtime_str(&state, "turn_boundary_ref") {
        let mut payload = HashMap::new();
        payload.insert(
            "reason".to_string(),
            Value::String("deepseek_session_completed".to_string()),
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
        payload.insert("native_status".to_string(), Value::String(status.clone()));
        payload.insert(
            "native_updated_at".to_string(),
            observation
                .updated_at
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
        state.insert("turn_boundary_ref".to_string(), Value::String(boundary_ref));
        items.push(boundary_item.clone());

        let decision = CompletionDecision {
            terminal: true,
            status: CompletionStatus::Completed,
            reason: Some("deepseek_session_completed".to_string()),
            confidence: Some(CompletionConfidence::Observed),
            reply: reply.clone(),
            anchor_seen: true,
            reply_started: !reply.is_empty(),
            reply_stable: !reply.is_empty(),
            provider_turn_ref: Some(req_id.clone()),
            source_cursor: Some(boundary_item.cursor),
            finished_at: Some(now.to_string()),
            diagnostics: serde_json::Map::new(),
        };

        let mut updated = submission.clone();
        updated.reply = reply;
        updated.runtime_state = state;
        return Some(ProviderPollResult::new(updated, items, Some(decision)));
    }

    if total_secs >= MAX_WAIT_SECS && !observation.completed {
        let reply_buffer = runtime_str(&state, "reply_buffer");
        return Some(terminal_result(
            submission,
            &mut state,
            CompletionStatus::Failed,
            "deepseek_native_turn_timeout",
            &reply_buffer,
            CompletionConfidence::Degraded,
            now,
        ));
    }

    if items.is_empty() {
        return None;
    }

    let mut updated = submission.clone();
    updated.runtime_state = state;
    Some(ProviderPollResult::new(updated, items, None))
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
    let cleaned_reply = reply.to_string();
    let mut updated = submission.clone();
    updated.runtime_state = state.clone();
    updated.status = status;
    updated.reason = reason.to_string();
    updated.reply = cleaned_reply.clone();
    updated.confidence = confidence;

    let seq = runtime_u64(state, "next_seq").max(1);
    let total_secs = runtime_str(state, "total_secs")
        .parse::<f64>()
        .unwrap_or(0.0);
    let cursor = CompletionCursor {
        source_kind: submission.source_kind,
        event_seq: Some(seq),
        updated_at: Some(now.to_string()),
        ..Default::default()
    };
    let mut diagnostics = serde_json::Map::new();
    diagnostics.insert(
        "mode".to_string(),
        Value::String("native_session_snapshot".to_string()),
    );
    diagnostics.insert(
        "total_secs".to_string(),
        Value::Number((total_secs as u64).into()),
    );
    diagnostics.insert(
        "anchor_seen".to_string(),
        Value::Bool(runtime_bool(state, "anchor_emitted")),
    );
    diagnostics.insert(
        "reply_chars".to_string(),
        Value::Number((cleaned_reply.len() as u64).into()),
    );

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: cleaned_reply,
        anchor_seen: runtime_bool(state, "anchor_emitted"),
        reply_started: !reply.is_empty(),
        reply_stable: !reply.is_empty() && status == CompletionStatus::Completed,
        provider_turn_ref: Some(runtime_str(state, "request_anchor")),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };

    ProviderPollResult::new(updated, vec![], Some(decision))
}

fn send_prompt(target: &dyn PromptTarget, pane_id: &str, prompt: &str) -> Option<String> {
    target.send_text(pane_id, prompt).err()
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

fn seconds_between(start: &str, end: &str) -> f64 {
    let start_dt = parse_now(start);
    let end_dt = parse_now(end);
    match (start_dt, end_dt) {
        (Some(s), Some(e)) => (e.duration_since(s).unwrap_or_default()).as_secs_f64(),
        _ => 0.0,
    }
}

fn parse_now(now: &str) -> Option<SystemTime> {
    if now.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(now)
        .ok()
        .map(|dt| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(dt.timestamp() as u64))
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

fn next_seq(state: &mut HashMap<String, Value>) -> u64 {
    let seq = runtime_u64(state, "next_seq").max(1);
    state.insert("next_seq".to_string(), Value::Number((seq + 1).into()));
    seq
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::with_prompt_target_override;
    use ccbr_provider_core::protocol;
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
            Ok(String::new())
        }
    }

    impl RecordingTarget {
        fn sent_count(&self) -> usize {
            self.sent.lock().unwrap().len()
        }
    }

    #[test]
    fn test_manifest() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
        assert!(m.supports_runtime_mode(&RuntimeMode::PaneBacked));
    }

    #[test]
    fn test_backend_has_binding_and_launcher() {
        let b = backend();
        assert_eq!(b.provider(), PROVIDER_NAME);
        assert!(b.session_binding.is_some());
        assert!(b.runtime_launcher.is_some());
    }

    #[test]
    fn test_execution_adapter_provider_name() {
        let adapter = DeepSeekExecutionAdapter;
        assert_eq!(adapter.provider(), PROVIDER_NAME);
    }

    #[test]
    fn test_start_submission_missing_session() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let adapter = DeepSeekExecutionAdapter;
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
            .contains("deepseek_session_file_missing"));
    }

    #[test]
    fn test_start_submission_sends_prompt() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();
        write_json(
            &work_dir,
            ".deepseek-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let target = Arc::new(RecordingTarget::default());
        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let adapter = DeepSeekExecutionAdapter;
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z")
        });

        assert_eq!(target.sent_count(), 1);
        let (pane, prompt) = target.sent.lock().unwrap().first().cloned().unwrap();
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
    fn test_poll_submission_detects_reply() {
        let tmp = TempDir::new().unwrap();
        let work_dir = tmp.path().join("workspace");
        std::fs::create_dir(&work_dir).unwrap();

        let req_id = request_anchor_for_job("j1");
        write_json(
            &work_dir,
            ".deepseek-session",
            serde_json::json!({
                "pane_id": "%1",
                "work_dir": work_dir.to_string_lossy().to_string(),
            }),
        );

        let home = tmp.path().join(".deepcode");
        let project_root =
            home.join("projects")
                .join(crate::deepseek::native_log::deepseek_project_code(
                    &work_dir,
                ));
        std::fs::create_dir_all(&project_root).unwrap();
        let session_path = project_root.join("sess1.jsonl");
        write_lines(
            &session_path,
            &[
                &format!(
                    r#"{{"role":"user","content":"{} {}"}}"#,
                    protocol::REQ_ID_PREFIX,
                    req_id
                ),
                r#"{"role":"assistant","content":"hello","id":"msg-1"}"#,
            ],
        );
        let index_path = project_root.join("sessions-index.json");
        std::fs::write(
            &index_path,
            serde_json::to_string(&serde_json::json!([
                {"id": "sess1", "status": "completed"}
            ]))
            .unwrap(),
        )
        .unwrap();

        let target = Arc::new(RecordingTarget::default());
        let adapter = DeepSeekExecutionAdapter;
        std::env::set_var("DEEPCODE_HOME", &home);
        let result = with_prompt_target_override(target.clone(), || {
            let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
            let ctx = ProviderRuntimeContext {
                workspace_path: Some(work_dir.to_string_lossy().to_string()),
                ..Default::default()
            };
            let submission = adapter.start(&job, Some(&ctx), "2025-01-01T00:00:00Z");
            adapter.poll(&submission, "2025-01-01T00:00:01Z")
        });
        std::env::remove_var("DEEPCODE_HOME");

        let result = result.expect("expected poll result");
        assert!(result.decision.is_some());
        assert_eq!(result.submission.reply, "hello");
        assert!(result.items.iter().any(|i| i.cursor.session_path.is_some()));
    }

    #[test]
    fn test_wrap_native_prompt_includes_req_id() {
        let prompt = wrap_native_prompt("do the thing", "job-123");
        assert!(prompt.contains("job-123"));
        assert!(prompt.contains("do the thing"));
        assert!(prompt.contains(protocol::REQ_ID_PREFIX));
    }
}
