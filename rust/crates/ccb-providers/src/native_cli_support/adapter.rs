use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use ccb_completion::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItemKind,
    CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::protocol;
use serde_json::Value;

use crate::execution::{
    build_item, error_submission, no_wrap_requested, ExecutionAdapter, ProviderPollResult,
    ProviderRuntimeContext, ProviderSubmission,
};

use super::config::{
    default_done_markers, NativeCliExecutionConfig, NativeCliExecutionRequest, OutputKind,
};
use super::observation::{observe_jsonl_output, observe_stdout_output, NativeCliObservation};
use super::prompt::{clean_native_reply, wrap_native_prompt};

static RUN_PROCS: Mutex<Option<HashMap<String, Child>>> = Mutex::new(None);

const MAX_STDERR_CHARS: usize = 4000;

/// Generic execution adapter for native CLI providers.
///
/// Mirrors Python `provider_backends.native_cli_support.NativeCliSubprocessAdapter`.
pub struct NativeCliExecutionAdapter {
    config: NativeCliExecutionConfig,
}

impl NativeCliExecutionAdapter {
    pub fn new(config: NativeCliExecutionConfig) -> Self {
        Self { config }
    }

    pub fn provider(&self) -> &str {
        &self.config.provider
    }
}

impl ExecutionAdapter for NativeCliExecutionAdapter {
    fn provider(&self) -> &str {
        &self.config.provider
    }

    fn start(
        &self,
        job: &JobRecord,
        context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        _start_submission(&self.config, job, context, now)
    }

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        _poll_submission(&self.config, submission, now)
    }

    fn cancel(&self, submission: &ProviderSubmission) {
        _terminate_process(&submission.runtime_state, false);
    }
}

fn _start_submission(
    config: &NativeCliExecutionConfig,
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let provider = config.provider.clone();
    let work_dir = match _resolve_work_dir(job, context) {
        Some(path) => path,
        None => {
            return error_submission(
                job,
                &provider,
                now,
                CompletionSourceKind::StructuredResultStream,
                "runtime_unavailable",
                "work_dir_missing",
            )
        }
    };

    let session = match _load_session_for_job(&provider, &config.session_filename, &work_dir, job) {
        Some(session) => session,
        None => {
            return error_submission(
                job,
                &provider,
                now,
                CompletionSourceKind::StructuredResultStream,
                "runtime_unavailable",
                format!("{}_session_file_missing", provider),
            )
        }
    };

    let runtime_dir = _path_from_session(&session.data, "runtime_dir");
    let completion_dir = _path_from_session(&session.data, "completion_artifact_dir")
        .unwrap_or_else(|| {
            (runtime_dir.unwrap_or_else(|| work_dir.join(".ccb").join("runtime").join(&provider)))
                .join("completion")
        });
    if let Err(exc) = std::fs::create_dir_all(&completion_dir) {
        return error_submission(
            job,
            &provider,
            now,
            CompletionSourceKind::StructuredResultStream,
            "runtime_unavailable",
            format!("create_completion_dir_failed:{}", exc),
        );
    }

    let output_suffix = config.output_kind.file_suffix();
    let stdout_path =
        completion_dir.join(format!("{}.{}-run.{}", job.job_id, provider, output_suffix));
    let stderr_path = completion_dir.join(format!("{}.{}-run.stderr.log", job.job_id, provider));
    let request_anchor = protocol::request_anchor_for_job(&job.job_id);
    let provider_options = Value::Object(job.provider_options.clone());
    let no_wrap = no_wrap_requested(Some(&provider_options));
    let prompt = if no_wrap {
        job.request.body.clone()
    } else {
        wrap_native_prompt(&job.request.body, &protocol::make_req_id(&job.job_id))
    };

    let request = NativeCliExecutionRequest {
        provider: provider.clone(),
        job_id: job.job_id.clone(),
        work_dir: work_dir.clone(),
        session_data: session.data.clone(),
        prompt,
        request_anchor: request_anchor.clone(),
    };
    let cmd = config.command_builder.build_command(request.clone());
    let env = _native_cli_env(config, &request);

    let stdout_file = match std::fs::File::create(&stdout_path) {
        Ok(file) => file,
        Err(exc) => {
            return error_submission(
                job,
                &provider,
                now,
                CompletionSourceKind::StructuredResultStream,
                config.reason("start_failed"),
                format!("create_stdout_failed:{}", exc),
            )
        }
    };
    let stderr_file = match std::fs::File::create(&stderr_path) {
        Ok(file) => file,
        Err(exc) => {
            return error_submission(
                job,
                &provider,
                now,
                CompletionSourceKind::StructuredResultStream,
                config.reason("start_failed"),
                format!("create_stderr_failed:{}", exc),
            )
        }
    };

    let mut command = Command::new(&cmd[0]);
    command
        .args(&cmd[1..])
        .current_dir(&work_dir)
        .envs(&env)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));

    let proc = match command.spawn() {
        Ok(proc) => proc,
        Err(exc) => {
            return error_submission(
                job,
                &provider,
                now,
                CompletionSourceKind::StructuredResultStream,
                config.reason("start_failed"),
                format!("{}: {}", std::any::type_name_of_val(&exc), exc),
            )
        }
    };

    let pid = proc.id();
    let proc_key = _proc_key(&provider, &job.job_id);
    {
        let mut guard = RUN_PROCS.lock().unwrap();
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        guard.as_mut().unwrap().insert(proc_key, proc);
    }

    let mut state = HashMap::new();
    state.insert("mode".to_string(), Value::String(config.mode.clone()));
    state.insert("provider".to_string(), Value::String(provider.clone()));
    state.insert("job_id".to_string(), Value::String(job.job_id.clone()));
    state.insert(
        "request_anchor".to_string(),
        Value::String(request_anchor.clone()),
    );
    state.insert(
        "work_dir".to_string(),
        Value::String(work_dir.to_string_lossy().to_string()),
    );
    state.insert("started_at".to_string(), Value::String(now.to_string()));
    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    state.insert("next_seq".to_string(), Value::Number(1.into()));
    state.insert("anchor_emitted".to_string(), Value::Bool(no_wrap));
    state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    state.insert("reply_buffer".to_string(), Value::String(String::new()));
    state.insert(
        "stdout_path".to_string(),
        Value::String(stdout_path.to_string_lossy().to_string()),
    );
    state.insert(
        "stderr_path".to_string(),
        Value::String(stderr_path.to_string_lossy().to_string()),
    );
    state.insert("pid".to_string(), Value::Number(pid.into()));
    state.insert("returncode".to_string(), Value::Null);
    state.insert(
        "run_timeout_s".to_string(),
        Value::Number(
            serde_json::Number::from_f64(_effective_run_timeout_s(config)).unwrap_or(900.into()),
        ),
    );

    let diagnostics = serde_json::json!({
        "provider": provider,
        "mode": config.mode,
        "workspace_path": work_dir.to_string_lossy().to_string(),
        "stdout_path": stdout_path.to_string_lossy().to_string(),
        "stderr_path": stderr_path.to_string_lossy().to_string(),
        "pid": pid,
    });

    ProviderSubmission {
        job_id: job.job_id.clone(),
        agent_name: job.agent_name.clone(),
        provider,
        accepted_at: now.to_string(),
        ready_at: now.to_string(),
        source_kind: CompletionSourceKind::StructuredResultStream,
        reply: String::new(),
        status: CompletionStatus::Incomplete,
        reason: "in_progress".to_string(),
        confidence: CompletionConfidence::Observed,
        diagnostics: Some(diagnostics),
        runtime_state: state,
    }
}

fn _poll_submission(
    config: &NativeCliExecutionConfig,
    submission: &ProviderSubmission,
    now: &str,
) -> Option<ProviderPollResult> {
    let mode = submission
        .runtime_state
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if mode == "passive" || mode == "error" {
        return Some(_runtime_error_result(
            submission,
            now,
            submission
                .runtime_state
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("runtime_unavailable"),
            submission
                .runtime_state
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ));
    }
    if mode != config.mode {
        return Some(_runtime_error_result(
            submission,
            now,
            "runtime_state_corrupt",
            "",
        ));
    }

    let mut state = submission.runtime_state.clone();
    let provider = state
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or(&config.provider)
        .to_string();
    state.insert("last_poll_at".to_string(), Value::String(now.to_string()));
    let next_seq = _state_int(&state, "next_seq", 1);
    state.insert("next_seq".to_string(), Value::Number(next_seq.into()));

    let proc_key = _proc_key(&provider, &submission.job_id);
    {
        let mut guard = RUN_PROCS.lock().unwrap();
        if let Some(procs) = guard.as_mut() {
            if let Some(proc) = procs.get_mut(&proc_key) {
                let returncode = proc
                    .try_wait()
                    .ok()
                    .flatten()
                    .map(|s| s.code().unwrap_or(-1));
                state.insert(
                    "returncode".to_string(),
                    match returncode {
                        Some(code) => Value::Number(code.into()),
                        None => Value::Null,
                    },
                );
                if returncode.is_some() {
                    procs.remove(&proc_key);
                }
            }
        }
    }

    let observer = config
        .observer
        .as_ref()
        .map(|o| o.as_ref())
        .unwrap_or_else(|| match config.output_kind {
            OutputKind::Jsonl => {
                &observe_jsonl_output as &dyn super::observation::NativeCliObserver
            }
            OutputKind::Stdout => {
                &observe_stdout_output as &dyn super::observation::NativeCliObserver
            }
        });
    let stdout_path = PathBuf::from(
        state
            .get("stdout_path")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let mut observation = observer.observe(&stdout_path);

    // For stdout mode, also check configured done markers.
    if matches!(config.output_kind, OutputKind::Stdout) && !observation.finished {
        if let Some(marker) = _detect_done_marker(&observation.text, config) {
            observation.finished = true;
            if observation.finish_reason.is_empty() {
                observation.finish_reason = marker;
            }
        }
    }

    let mut items = Vec::new();
    if !_state_bool(&state, "anchor_emitted") {
        items.push(build_item(
            submission,
            CompletionItemKind::AnchorSeen,
            now,
            _next_seq(&mut state),
            {
                let mut payload = HashMap::new();
                payload.insert(
                    "turn_id".to_string(),
                    Value::String(
                        state
                            .get("request_anchor")
                            .and_then(Value::as_str)
                            .unwrap_or(&submission.job_id)
                            .to_string(),
                    ),
                );
                payload.insert(
                    "source".to_string(),
                    Value::String(format!("{}_native_cli_prompt_submitted", provider)),
                );
                payload
            },
        ));
        state.insert("anchor_emitted".to_string(), Value::Bool(true));
    }

    let request_anchor = state
        .get("request_anchor")
        .and_then(Value::as_str)
        .unwrap_or(&submission.job_id)
        .to_string();
    let reply = clean_native_reply(observation.text.trim(), &request_anchor);
    let previous_reply = state
        .get("reply_buffer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !reply.is_empty() && reply != previous_reply {
        state.insert("reply_buffer".to_string(), Value::String(reply.clone()));
        items.push(build_item(
            submission,
            CompletionItemKind::AssistantFinal,
            now,
            _next_seq(&mut state),
            {
                let mut payload = HashMap::new();
                payload.insert("text".to_string(), Value::String(reply.clone()));
                payload.insert("reply".to_string(), Value::String(reply.clone()));
                payload.insert("final_answer".to_string(), Value::String(reply.clone()));
                payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                if let Some(turn_ref) = &observation.turn_ref {
                    payload.insert(
                        "provider_turn_ref".to_string(),
                        Value::String(turn_ref.clone()),
                    );
                }
                if let Some(completed_at) = &observation.completed_at {
                    payload.insert("completed_at".to_string(), completed_at.clone());
                }
                if !observation.finish_reason.is_empty() {
                    payload.insert(
                        "finish_reason".to_string(),
                        Value::String(observation.finish_reason.clone()),
                    );
                }
                payload
            },
        ));
    }

    let returncode = _coerce_returncode(state.get("returncode"));
    let terminal = _terminal_result_if_ready(
        config,
        submission,
        &mut state,
        &observation,
        returncode,
        &mut items,
        now,
    );
    if let Some(result) = terminal {
        return Some(result);
    }

    let mut updated = submission.clone();
    updated.reply = state
        .get("reply_buffer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    updated.runtime_state = state.clone();
    if !items.is_empty() || updated.reply != submission.reply {
        return Some(ProviderPollResult::new(updated, items, None));
    }
    None
}

fn _terminal_result_if_ready(
    config: &NativeCliExecutionConfig,
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    observation: &NativeCliObservation,
    returncode: Option<i32>,
    items: &mut Vec<ccb_completion::models::CompletionItem>,
    now: &str,
) -> Option<ProviderPollResult> {
    let provider = config.provider.clone();
    let request_anchor = state
        .get("request_anchor")
        .and_then(Value::as_str)
        .unwrap_or(&submission.job_id)
        .to_string();
    let reply = state
        .get("reply_buffer")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| clean_native_reply(&observation.text, &request_anchor))
        .trim()
        .to_string();

    if !observation.error.is_empty() {
        return Some(_terminal(
            config,
            submission,
            state,
            items,
            now,
            CompletionStatus::Failed,
            &config.reason("run_error"),
            &reply,
            CompletionConfidence::Degraded,
            Some(serde_json::json!({"error": observation.error})),
            true,
        ));
    }

    let timeout_s = _state_float(state, "run_timeout_s", config.run_timeout_s);
    if returncode.is_none()
        && _run_timeout_elapsed(
            state
                .get("started_at")
                .and_then(Value::as_str)
                .unwrap_or(""),
            now,
            timeout_s,
        )
    {
        let stderr_tail = _stderr_tail(
            state
                .get("stderr_path")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let diagnostics = serde_json::json!({
            "run_timeout_s": timeout_s,
            "run_timeout_started_at": state.get("started_at").and_then(Value::as_str).unwrap_or(""),
            "stdout_path": state.get("stdout_path").and_then(Value::as_str).unwrap_or(""),
            "stderr_path": state.get("stderr_path").and_then(Value::as_str).unwrap_or(""),
            "stderr_tail": stderr_tail,
        });
        return Some(_terminal(
            config,
            submission,
            state,
            items,
            now,
            CompletionStatus::Incomplete,
            &config.reason("timeout"),
            &reply,
            CompletionConfidence::Degraded,
            Some(diagnostics),
            false,
        ));
    }

    if let Some(code) = returncode {
        if code != 0 {
            let stderr_tail = _stderr_tail(
                state
                    .get("stderr_path")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            );
            let diagnostics = serde_json::json!({
                "returncode": code,
                "stderr_tail": stderr_tail,
            });
            return Some(_terminal(
                config,
                submission,
                state,
                items,
                now,
                CompletionStatus::Failed,
                &config.reason("failed"),
                &reply,
                CompletionConfidence::Degraded,
                Some(diagnostics),
                true,
            ));
        }
    }

    if observation.finished {
        let reason = _normalized_reason(&observation.finish_reason);
        let (status, terminal_reason, confidence) =
            if !reason.is_empty() && !_is_done_reason(&reason) {
                (
                    CompletionStatus::Incomplete,
                    format!("{}_run_finished:{}", provider, reason),
                    CompletionConfidence::Observed,
                )
            } else if reply.is_empty() {
                (
                    CompletionStatus::Incomplete,
                    config.reason("empty"),
                    CompletionConfidence::Degraded,
                )
            } else {
                (
                    CompletionStatus::Completed,
                    config.reason("complete"),
                    CompletionConfidence::Observed,
                )
            };
        _append_turn_boundary(
            submission,
            state,
            items,
            now,
            &terminal_reason,
            &reply,
            observation,
        );
        let diagnostics = serde_json::json!({
            "finish_reason": observation.finish_reason,
            "stdout_path": state.get("stdout_path").and_then(Value::as_str).unwrap_or(""),
            "stderr_path": state.get("stderr_path").and_then(Value::as_str).unwrap_or(""),
            "returncode": returncode,
        });
        return Some(_terminal(
            config,
            submission,
            state,
            items,
            now,
            status,
            &terminal_reason,
            &reply,
            confidence,
            Some(diagnostics),
            true,
        ));
    }

    if returncode == Some(0) && config.terminal_on_process_exit {
        let finish_reason = _normalized_reason(&observation.finish_reason);
        if !finish_reason.is_empty() && !_is_done_reason(&finish_reason) {
            let terminal_reason = format!("{}_run_finished:{}", provider, finish_reason);
            _append_turn_boundary(
                submission,
                state,
                items,
                now,
                &terminal_reason,
                &reply,
                observation,
            );
            let diagnostics = serde_json::json!({
                "finish_reason": observation.finish_reason,
                "returncode": returncode,
            });
            return Some(_terminal(
                config,
                submission,
                state,
                items,
                now,
                CompletionStatus::Incomplete,
                &terminal_reason,
                &reply,
                CompletionConfidence::Degraded,
                Some(diagnostics),
                true,
            ));
        }
        if reply.is_empty() {
            let diagnostics = serde_json::json!({"returncode": returncode});
            return Some(_terminal(
                config,
                submission,
                state,
                items,
                now,
                CompletionStatus::Incomplete,
                &config.reason("empty"),
                "",
                CompletionConfidence::Degraded,
                Some(diagnostics),
                true,
            ));
        }
        _append_turn_boundary(
            submission,
            state,
            items,
            now,
            &config.reason("process_exit_complete"),
            &reply,
            observation,
        );
        let diagnostics = serde_json::json!({"returncode": returncode});
        return Some(_terminal(
            config,
            submission,
            state,
            items,
            now,
            CompletionStatus::Completed,
            &config.reason("process_exit_complete"),
            &reply,
            CompletionConfidence::Observed,
            Some(diagnostics),
            true,
        ));
    }

    None
}

fn _append_turn_boundary(
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    items: &mut Vec<ccb_completion::models::CompletionItem>,
    now: &str,
    reason: &str,
    reply: &str,
    observation: &NativeCliObservation,
) {
    if _state_bool(state, "turn_boundary_emitted") {
        return;
    }
    items.push(build_item(
        submission,
        CompletionItemKind::TurnBoundary,
        now,
        _next_seq(state),
        {
            let mut payload = HashMap::new();
            payload.insert("reason".to_string(), Value::String(reason.to_string()));
            payload.insert(
                "last_agent_message".to_string(),
                Value::String(reply.to_string()),
            );
            payload.insert(
                "turn_id".to_string(),
                Value::String(
                    state
                        .get("request_anchor")
                        .and_then(Value::as_str)
                        .unwrap_or(&submission.job_id)
                        .to_string(),
                ),
            );
            if let Some(turn_ref) = &observation.turn_ref {
                payload.insert(
                    "provider_turn_ref".to_string(),
                    Value::String(turn_ref.clone()),
                );
            }
            if !observation.finish_reason.is_empty() {
                payload.insert(
                    "finish_reason".to_string(),
                    Value::String(observation.finish_reason.clone()),
                );
            }
            if let Some(completed_at) = &observation.completed_at {
                payload.insert("completed_at".to_string(), completed_at.clone());
            }
            payload
        },
    ));
    state.insert("turn_boundary_emitted".to_string(), Value::Bool(true));
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::ptr_arg)]
fn _terminal(
    config: &NativeCliExecutionConfig,
    submission: &ProviderSubmission,
    state: &mut HashMap<String, Value>,
    items: &mut Vec<ccb_completion::models::CompletionItem>,
    now: &str,
    status: CompletionStatus,
    reason: &str,
    reply: &str,
    confidence: CompletionConfidence,
    diagnostics_extra: Option<Value>,
    terminate_grace: bool,
) -> ProviderPollResult {
    state.insert(
        "returncode".to_string(),
        match _coerce_returncode(state.get("returncode")) {
            Some(code) => Value::Number(code.into()),
            None => Value::Null,
        },
    );
    let mut updated = submission.clone();
    updated.runtime_state = state.clone();
    updated.status = status;
    updated.reason = reason.to_string();
    updated.reply = reply.to_string();
    updated.confidence = confidence;

    let cursor = items
        .last()
        .map(|item| item.cursor.clone())
        .unwrap_or_else(|| CompletionCursor {
            source_kind: submission.source_kind,
            event_seq: Some(_state_int(state, "next_seq", 1)),
            updated_at: Some(now.to_string()),
            ..Default::default()
        });

    let mut diagnostics = serde_json::Map::new();
    diagnostics.insert("mode".to_string(), Value::String(config.mode.clone()));
    diagnostics.insert(
        "anchor_seen".to_string(),
        Value::Bool(_state_bool(state, "anchor_emitted")),
    );
    diagnostics.insert("reply_chars".to_string(), Value::Number(reply.len().into()));
    if let Some(Value::Object(map)) = diagnostics_extra {
        for (k, v) in map {
            diagnostics.insert(k, v);
        }
    }

    let decision = CompletionDecision {
        terminal: true,
        status,
        reason: Some(reason.to_string()),
        confidence: Some(confidence),
        reply: reply.to_string(),
        anchor_seen: _state_bool(state, "anchor_emitted"),
        reply_started: !reply.is_empty(),
        reply_stable: !reply.is_empty() && status == CompletionStatus::Completed,
        provider_turn_ref: Some(
            state
                .get("request_anchor")
                .and_then(Value::as_str)
                .unwrap_or(&submission.job_id)
                .to_string(),
        ),
        source_cursor: Some(cursor),
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    _terminate_process(state, terminate_grace);
    ProviderPollResult::new(updated, items.clone(), Some(decision))
}

fn _runtime_error_result(
    submission: &ProviderSubmission,
    now: &str,
    reason: &str,
    error: &str,
) -> ProviderPollResult {
    let mut updated = submission.clone();
    updated.status = CompletionStatus::Failed;
    updated.reason = reason.to_string();
    updated.confidence = CompletionConfidence::Degraded;
    updated
        .runtime_state
        .insert("mode".to_string(), Value::String("error".to_string()));
    updated
        .runtime_state
        .insert("reason".to_string(), Value::String(reason.to_string()));
    updated
        .runtime_state
        .insert("error".to_string(), Value::String(error.to_string()));

    let mut diagnostics = serde_json::Map::new();
    diagnostics.insert("reason".to_string(), Value::String(reason.to_string()));
    diagnostics.insert("error".to_string(), Value::String(error.to_string()));
    let decision = CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(reason.to_string()),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: false,
        reply_started: false,
        reply_stable: false,
        provider_turn_ref: Some(submission.job_id.clone()),
        source_cursor: None,
        finished_at: Some(now.to_string()),
        diagnostics,
    };
    ProviderPollResult::new(updated, Vec::new(), Some(decision))
}

fn _native_cli_env(
    config: &NativeCliExecutionConfig,
    request: &NativeCliExecutionRequest,
) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(env_builder) = &config.env_builder {
        env.extend(env_builder.build_env(request));
    }
    env
}

fn _resolve_work_dir(job: &JobRecord, context: Option<&ProviderRuntimeContext>) -> Option<PathBuf> {
    let value = context
        .and_then(|c| c.workspace_path.as_ref())
        .or(job.workspace_path.as_ref())?;
    if value.is_empty() {
        return None;
    }
    Some(expand_home(value))
}

fn _load_session_for_job(
    _provider: &str,
    session_filename: &str,
    work_dir: &Path,
    job: &JobRecord,
) -> Option<NativeCliProjectSession> {
    let instance = job
        .provider_instance
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| Some(job.agent_name.trim()).filter(|s| !s.is_empty()));

    let mut candidates: Vec<Option<String>> = Vec::new();
    if let Some(inst) = instance {
        candidates.push(Some(inst.to_string()));
    }
    candidates.push(None);

    for instance in candidates {
        let filename = session_filename_for_instance(session_filename, instance.as_deref());
        if let Some(session_file) = find_session_file_for_work_dir(work_dir, &filename) {
            if let Ok(raw) = std::fs::read_to_string(&session_file) {
                if let Ok(data) = serde_json::from_str::<HashMap<String, Value>>(&raw) {
                    return Some(NativeCliProjectSession { session_file, data });
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Default)]
struct NativeCliProjectSession {
    #[allow(dead_code)]
    session_file: PathBuf,
    data: HashMap<String, Value>,
}

fn _path_from_session(session_data: &HashMap<String, Value>, key: &str) -> Option<PathBuf> {
    let value = session_data.get(key).and_then(Value::as_str)?.trim();
    if value.is_empty() {
        return None;
    }
    Some(expand_home(value))
}

fn _detect_done_marker(text: &str, config: &NativeCliExecutionConfig) -> Option<String> {
    let normalized = text.to_lowercase();
    for marker in &config.done_markers {
        if normalized.contains(&marker.to_lowercase()) {
            return Some(marker.clone());
        }
    }
    None
}

fn _effective_run_timeout_s(config: &NativeCliExecutionConfig) -> f64 {
    let env_name = format!(
        "CCB_{}_RUN_TIMEOUT_S",
        config.provider.to_uppercase().replace('-', "_")
    );
    if let Ok(raw) = std::env::var(&env_name) {
        let raw = raw.trim();
        if !raw.is_empty() {
            if let Ok(value) = raw.parse::<f64>() {
                return value.max(0.0);
            }
        }
    }
    config.run_timeout_s.max(0.0)
}

fn _run_timeout_elapsed(started_at: &str, now: &str, timeout_s: f64) -> bool {
    if timeout_s <= 0.0 || started_at.is_empty() || now.is_empty() {
        return false;
    }
    let started = match chrono::DateTime::parse_from_rfc3339(started_at) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(_) => return false,
    };
    let now_dt = match chrono::DateTime::parse_from_rfc3339(now) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(_) => return false,
    };
    (now_dt - started).num_seconds() as f64 >= timeout_s
}

fn _stderr_tail(path_str: &str) -> String {
    let path = Path::new(path_str);
    if !path.is_file() {
        return String::new();
    }
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let start = text.len().saturating_sub(MAX_STDERR_CHARS);
            text[start..].to_string()
        }
        Err(exc) => format!("read_stderr_failed:{}", exc),
    }
}

fn _terminate_process(state: &HashMap<String, Value>, _grace: bool) {
    let provider = state.get("provider").and_then(Value::as_str).unwrap_or("");
    let job_id = state.get("job_id").and_then(Value::as_str).unwrap_or("");
    let proc_key = _proc_key(provider, job_id);
    let mut guard = RUN_PROCS.lock().unwrap();
    let Some(procs) = guard.as_mut() else {
        return;
    };
    let Some(mut proc) = procs.remove(&proc_key) else {
        return;
    };
    drop(guard);

    if proc.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = proc.kill();
}

fn _proc_key(provider: &str, job_id: &str) -> String {
    format!("{}:{}", provider, job_id)
}

fn _next_seq(state: &mut HashMap<String, Value>) -> u64 {
    let seq = _state_int(state, "next_seq", 1);
    state.insert("next_seq".to_string(), Value::Number((seq + 1).into()));
    seq
}

fn _state_int(state: &HashMap<String, Value>, key: &str, default: u64) -> u64 {
    state.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
}

fn _state_float(state: &HashMap<String, Value>, key: &str, default: f64) -> f64 {
    state
        .get(key)
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
        .max(0.0)
}

fn _state_bool(state: &HashMap<String, Value>, key: &str) -> bool {
    state.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn _coerce_returncode(value: Option<&Value>) -> Option<i32> {
    value.and_then(|v| match v {
        Value::Number(n) => n.as_i64().map(|i| i as i32),
        Value::String(s) => s.parse::<i32>().ok(),
        _ => None,
    })
}

fn _normalized_reason(reason: &str) -> String {
    reason.trim().to_lowercase().replace('-', "_")
}

fn _is_done_reason(reason: &str) -> bool {
    default_done_markers().iter().any(|marker| marker == reason)
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest.strip_prefix('/').unwrap_or(rest));
        }
    }
    PathBuf::from(path)
}

/// Clear all tracked processes.
///
/// Used in tests to ensure test isolation.
pub fn clear_tracked_processes() {
    let mut guard = RUN_PROCS.lock().unwrap();
    if let Some(procs) = guard.as_mut() {
        for (_key, mut proc) in procs.drain() {
            let _ = proc.kill();
        }
    }
    *guard = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_provider_name() {
        let adapter =
            NativeCliExecutionAdapter::new(NativeCliExecutionConfig::new("crush", |_req| {
                vec!["echo".to_string(), "hi".to_string()]
            }));
        assert_eq!(adapter.provider(), "crush");
    }
}
