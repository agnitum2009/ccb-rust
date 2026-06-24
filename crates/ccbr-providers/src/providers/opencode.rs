use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_completion::models::{
    CompletionDecision, CompletionItemKind, CompletionSourceKind, CompletionStatus, JobRecord,
};
use ccbr_provider_core::contracts::{LaunchMode, ProviderBackend, ProviderRuntimeLauncher};
use ccbr_provider_core::manifest::{CompletionManifest, ProviderManifest, RuntimeMode};
use ccbr_provider_core::protocol::request_anchor_for_job;
use serde_json::Value;

use crate::execution::{
    build_item, error_submission, no_wrap_requested, ExecutionAdapter, ProviderPollResult,
    ProviderRuntimeContext, ProviderSubmission,
};

pub use crate::opencode::{
    build_runtime_launcher, build_session_binding, build_session_payload, build_start_cmd,
    find_project_session_file, load_project_session, prepare_launch_context, OpenCodeLaunchContext,
    OpenCodeLogReader, OpenCodeProjectSession,
};

pub const PROVIDER_NAME: &str = "opencode";
const OPENCODE_REQ_ID_PREFIX: &str = "CCBR_REQ_ID:";

const DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const DEFAULT_TIMEOUT_MS: u64 = 300_000;

/// Build the OpenCode provider manifest.
///
/// Mirrors Python `provider_backends.opencode.manifest.build_manifest`.
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

/// Build the full OpenCode provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
        execution_adapter: None,
        session_binding: Some(build_session_binding()),
        runtime_launcher: Some(ProviderRuntimeLauncher::new(
            PROVIDER_NAME,
            LaunchMode::SimpleTmux,
        )),
    }
}

/// OpenCode provider execution adapter.
pub struct OpenCodeExecutionAdapter;

impl ExecutionAdapter for OpenCodeExecutionAdapter {
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
        Some(submission.runtime_state.clone())
    }
}

fn start_active_submission(
    job: &JobRecord,
    context: Option<&ProviderRuntimeContext>,
    now: &str,
) -> ProviderSubmission {
    let workspace_path = context
        .and_then(|c| c.workspace_path.as_deref())
        .map(PathBuf::from)
        .unwrap_or_default();
    if workspace_path.as_os_str().is_empty() || !workspace_path.exists() {
        return error_submission(
            job,
            PROVIDER_NAME,
            now,
            CompletionSourceKind::SessionSnapshot,
            "missing_workspace",
            "workspace path missing or does not exist",
        );
    }

    let agent_name = context
        .map(|c| c.agent_name.as_str())
        .unwrap_or(&job.agent_name);
    let session = match load_project_session(&workspace_path, Some(agent_name)) {
        Some(s) => s,
        None => {
            return error_submission(
                job,
                PROVIDER_NAME,
                now,
                CompletionSourceKind::SessionSnapshot,
                "missing_opencode_session",
                "session file not found",
            );
        }
    };

    let request_anchor = request_anchor_for_job(&job.job_id);
    let no_wrap = no_wrap_requested(job.provider_options.get("no_wrap"));
    let prompt = if no_wrap {
        job.request.body.clone()
    } else {
        wrap_opencode_prompt(&job.request.body, &request_anchor)
    };

    let project_id = session
        .opencode_project_id()
        .unwrap_or("global")
        .to_string();
    let session_id_filter = session.opencode_session_id_filter();
    let session_path = session.session_file.to_string_lossy().to_string();
    let work_dir = workspace_path.to_string_lossy().to_string();
    let storage_root = resolve_storage_root(&workspace_path);

    let reader = OpenCodeLogReader::new(
        Some(&storage_root),
        &workspace_path,
        &project_id,
        session_id_filter.clone(),
    );
    let reader_state = reader.capture_state();

    let diagnostics = serde_json::json!({
        "provider": PROVIDER_NAME,
        "mode": "active",
        "workspace_path": work_dir,
    });

    let mut runtime_state = HashMap::new();
    runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
    runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
    runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
    runtime_state.insert("anchor_emitted".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
    runtime_state.insert("session_path".to_string(), Value::String(session_path));
    runtime_state.insert("no_wrap".to_string(), Value::Bool(no_wrap));
    runtime_state.insert("work_dir".to_string(), Value::String(work_dir));
    runtime_state.insert("project_id".to_string(), Value::String(project_id));
    runtime_state.insert(
        "session_id_filter".to_string(),
        session_id_filter.map(Value::String).unwrap_or(Value::Null),
    );
    runtime_state.insert(
        "storage_root".to_string(),
        Value::String(storage_root.to_string_lossy().to_string()),
    );
    runtime_state.insert(
        "reader_state".to_string(),
        Value::Object(reader_state.into_iter().collect()),
    );
    runtime_state.insert("prompt".to_string(), Value::String(prompt));
    runtime_state.insert("prompt_sent".to_string(), Value::Bool(false));

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
        confidence: ccbr_completion::models::CompletionConfidence::Observed,
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
    if mode != "active" {
        return None;
    }

    let work_dir = PathBuf::from(runtime_str(&submission.runtime_state, "work_dir"));
    let project_id = runtime_str(&submission.runtime_state, "project_id");
    let session_id_filter = submission
        .runtime_state
        .get("session_id_filter")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let storage_root = PathBuf::from(runtime_str(&submission.runtime_state, "storage_root"));

    let reader = OpenCodeLogReader::new(
        Some(&storage_root),
        &work_dir,
        &project_id,
        session_id_filter,
    );

    let prev_state: HashMap<String, Value> = submission
        .runtime_state
        .get("reader_state")
        .and_then(|v| v.as_object().cloned().map(|m| m.into_iter().collect()))
        .unwrap_or_else(|| reader.capture_state());

    let (reply, next_state) = reader.try_get_message(&prev_state);

    let mut items = Vec::new();
    let request_anchor = request_anchor_from_runtime_state(&submission.runtime_state);
    let mut next_seq = runtime_u64(&submission.runtime_state, "next_seq").max(1);
    let mut anchor_emitted = runtime_bool(&submission.runtime_state, "anchor_emitted");
    let mut reply_buffer = runtime_str(&submission.runtime_state, "reply_buffer");
    let session_path = runtime_str(&submission.runtime_state, "session_path");

    let mut decision = None;
    if let Some(reply) = reply {
        let observed_req_id = next_state
            .get("last_assistant_req_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let no_wrap = runtime_bool(&submission.runtime_state, "no_wrap");

        if reply_matches_request(&request_anchor, observed_req_id, no_wrap) {
            let cleaned = reply.trim().to_string();
            if !cleaned.is_empty() {
                reply_buffer = cleaned.clone();
                let session_path_opt = if session_path.is_empty() {
                    None
                } else {
                    Some(session_path.clone())
                };

                if !anchor_emitted {
                    let mut anchor_payload = HashMap::new();
                    anchor_payload
                        .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                    anchor_payload.insert(
                        "session_path".to_string(),
                        session_path_opt
                            .as_ref()
                            .map(|s| Value::String(s.clone()))
                            .unwrap_or(Value::Null),
                    );
                    items.push(build_item(
                        submission,
                        CompletionItemKind::AnchorSeen,
                        now,
                        next_seq,
                        anchor_payload,
                    ));
                    next_seq += 1;
                    anchor_emitted = true;
                }

                let assistant_id = next_state
                    .get("last_assistant_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let parent_id = next_state
                    .get("last_assistant_parent_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let completed_at = next_state.get("last_assistant_completed").cloned();
                let session_path_opt = if session_path.is_empty() {
                    None
                } else {
                    Some(session_path.clone())
                };

                let mut final_payload = HashMap::new();
                final_payload.insert("text".to_string(), Value::String(cleaned.clone()));
                final_payload.insert("reply".to_string(), Value::String(cleaned.clone()));
                final_payload.insert("final_answer".to_string(), Value::String(cleaned.clone()));
                final_payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                final_payload.insert(
                    "session_path".to_string(),
                    session_path_opt
                        .as_ref()
                        .map(|s| Value::String(s.clone()))
                        .unwrap_or(Value::Null),
                );
                final_payload.insert(
                    "provider_turn_ref".to_string(),
                    assistant_id
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                final_payload.insert(
                    "message_id".to_string(),
                    assistant_id
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                final_payload.insert(
                    "parent_message_id".to_string(),
                    parent_id.clone().map(Value::String).unwrap_or(Value::Null),
                );
                final_payload.insert(
                    "completed_at".to_string(),
                    completed_at.clone().unwrap_or(Value::Null),
                );
                items.push(build_item(
                    submission,
                    CompletionItemKind::AssistantFinal,
                    now,
                    next_seq,
                    final_payload,
                ));
                next_seq += 1;

                let mut boundary_payload = HashMap::new();
                boundary_payload.insert(
                    "reason".to_string(),
                    Value::String("assistant_completed".to_string()),
                );
                boundary_payload.insert(
                    "last_agent_message".to_string(),
                    Value::String(cleaned.clone()),
                );
                boundary_payload
                    .insert("turn_id".to_string(), Value::String(request_anchor.clone()));
                boundary_payload.insert(
                    "session_path".to_string(),
                    session_path_opt
                        .as_ref()
                        .map(|s| Value::String(s.clone()))
                        .unwrap_or(Value::Null),
                );
                boundary_payload.insert(
                    "provider_turn_ref".to_string(),
                    assistant_id
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                boundary_payload.insert(
                    "message_id".to_string(),
                    assistant_id
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                boundary_payload.insert(
                    "parent_message_id".to_string(),
                    parent_id.clone().map(Value::String).unwrap_or(Value::Null),
                );
                boundary_payload.insert(
                    "completed_at".to_string(),
                    completed_at.clone().unwrap_or(Value::Null),
                );
                let boundary_item = build_item(
                    submission,
                    CompletionItemKind::TurnBoundary,
                    now,
                    next_seq,
                    boundary_payload,
                );
                next_seq += 1;
                let source_cursor = Some(boundary_item.cursor.clone());
                items.push(boundary_item);

                decision = Some(CompletionDecision {
                    terminal: true,
                    status: CompletionStatus::Completed,
                    reason: Some("assistant_completed".to_string()),
                    confidence: Some(ccbr_completion::models::CompletionConfidence::Observed),
                    reply: cleaned.clone(),
                    anchor_seen: true,
                    reply_started: true,
                    reply_stable: true,
                    provider_turn_ref: Some(request_anchor.clone()),
                    source_cursor,
                    finished_at: Some(now.to_string()),
                    diagnostics: serde_json::Map::new(),
                });
            }
        }
    }

    if items.is_empty() {
        return None;
    }

    let mut updated = submission.clone();
    updated.reply = reply_buffer.clone();
    let mut new_runtime_state = submission.runtime_state.clone();
    new_runtime_state.insert(
        "reader_state".to_string(),
        Value::Object(next_state.into_iter().collect()),
    );
    new_runtime_state.insert("next_seq".to_string(), Value::Number(next_seq.into()));
    new_runtime_state.insert("anchor_emitted".to_string(), Value::Bool(anchor_emitted));
    new_runtime_state.insert("reply_buffer".to_string(), Value::String(reply_buffer));
    new_runtime_state.insert("session_path".to_string(), Value::String(session_path));
    updated.runtime_state = new_runtime_state;

    Some(ProviderPollResult::new(updated, items, decision))
}

fn wrap_opencode_prompt(message: &str, req_id: &str) -> String {
    let message = message.trim_end();
    format!("{} {}\n\n{}\n", OPENCODE_REQ_ID_PREFIX, req_id, message)
}

fn resolve_storage_root(work_dir: &Path) -> PathBuf {
    if let Ok(env_root) = std::env::var("OPENCODE_STORAGE_ROOT") {
        let trimmed = env_root.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(parent) = work_dir.parent() {
        let candidate = parent.join("storage");
        if candidate.is_dir() {
            return candidate;
        }
    }
    crate::opencode::default_opencode_storage_root().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local")
            .join("share")
            .join("opencode")
            .join("storage")
    })
}

fn reply_matches_request(
    request_anchor: &str,
    observed_req_id: Option<String>,
    no_wrap: bool,
) -> bool {
    if no_wrap {
        return true;
    }
    match observed_req_id {
        None => true,
        Some(observed) => {
            let observed = observed.trim().to_lowercase();
            !observed.is_empty() && observed == request_anchor.trim().to_lowercase()
        }
    }
}

fn request_anchor_from_runtime_state(runtime_state: &HashMap<String, Value>) -> String {
    runtime_state
        .get("request_anchor")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
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

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_opencode_prompt() {
        let wrapped = wrap_opencode_prompt("hello", "job-123");
        assert!(wrapped.contains("CCBR_REQ_ID:"));
        assert!(wrapped.contains("job-123"));
        assert!(wrapped.contains("hello"));
    }

    #[test]
    fn test_reply_matches_request() {
        assert!(reply_matches_request(
            "job-123",
            Some("job-123".to_string()),
            false
        ));
        assert!(!reply_matches_request(
            "job-123",
            Some("job-456".to_string()),
            false
        ));
        assert!(reply_matches_request("job-123", None, false));
        assert!(reply_matches_request(
            "job-123",
            Some("job-456".to_string()),
            true
        ));
    }
}
