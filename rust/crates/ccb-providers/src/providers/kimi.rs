use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_completion::models::{
    CompletionConfidence, CompletionDecision, CompletionItemKind, CompletionSourceKind,
    CompletionStatus, JobRecord,
};
use ccb_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccb_provider_core::manifest::ProviderManifest;
use ccb_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccb_provider_core::protocol;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::execution::{
    build_item, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};
use crate::providers::pane_backed_manifest;

pub const PROVIDER_NAME: &str = "kimi";
pub const KIMI_CONTEXT_FILENAME: &str = "CCB_KIMI_CONTEXT.md";

const SESSION_FILENAME: &str = ".kimi-session";
const SESSION_ID_ATTR: &str = "kimi_session_id";
const SESSION_PATH_ATTR: &str = "kimi_session_path";
const CONTEXT_PATH_ATTR: &str = "kimi_context_path";

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the Kimi provider manifest.
pub fn manifest() -> ProviderManifest {
    pane_backed_manifest(PROVIDER_NAME, false)
}

/// Build the Kimi provider backend registration.
pub fn backend() -> ProviderBackend {
    ProviderBackend {
        manifest: manifest(),
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

/// Kimi execution adapter.
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
        let request_anchor = protocol::request_anchor_for_job(&job.job_id);
        let req_id = protocol::make_req_id(&job.job_id);
        let context_path = context.and_then(|ctx| {
            ctx.workspace_path
                .as_deref()
                .map(|workspace| kimi_context_path(Path::new(workspace), &job.agent_name))
        });
        let prompt_text = wrap_kimi_prompt(&job.request.body, &req_id, context_path.as_deref());

        let mut runtime_state = HashMap::new();
        runtime_state.insert("mode".to_string(), Value::String("active".to_string()));
        runtime_state.insert("request_anchor".to_string(), Value::String(request_anchor));
        runtime_state.insert("next_seq".to_string(), Value::Number(1.into()));
        runtime_state.insert("anchor_seen".to_string(), Value::Bool(false));
        runtime_state.insert("reply_buffer".to_string(), Value::String(String::new()));
        runtime_state.insert("session_path".to_string(), Value::String(String::new()));
        runtime_state.insert("no_wrap".to_string(), Value::Bool(false));
        runtime_state.insert("prompt_text".to_string(), Value::String(prompt_text));
        runtime_state.insert("prompt_sent".to_string(), Value::Bool(false));
        if let Some(path) = context_path {
            runtime_state.insert(
                CONTEXT_PATH_ATTR.to_string(),
                Value::String(path.to_string_lossy().to_string()),
            );
            runtime_state.insert(
                "kimi_context_projection".to_string(),
                Value::String("file".to_string()),
            );
            runtime_state.insert(
                "kimi_context_projection_version".to_string(),
                Value::Number(1.into()),
            );
        }

        let diagnostics = serde_json::json!({
            "provider": PROVIDER_NAME,
            "mode": "active",
        });

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

    fn poll(&self, submission: &ProviderSubmission, now: &str) -> Option<ProviderPollResult> {
        if submission.is_terminal() {
            return None;
        }

        let request_anchor = submission
            .runtime_state
            .get("request_anchor")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_else(|| protocol::request_anchor_for_job(&submission.job_id));

        let req_id = protocol::make_req_id(&submission.job_id);
        let reply_buffer = submission
            .runtime_state
            .get("reply_buffer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if !protocol::is_done_text(&reply_buffer) {
            return None;
        }

        let cleaned = extract_reply_for_req(&reply_buffer, &req_id);
        let next_seq = submission
            .runtime_state
            .get("next_seq")
            .and_then(Value::as_u64)
            .unwrap_or(1);

        let mut payload = HashMap::new();
        payload.insert("reply".to_string(), Value::String(cleaned.clone()));
        payload.insert("text".to_string(), Value::String(cleaned.clone()));
        payload.insert("turn_id".to_string(), Value::String(request_anchor.clone()));
        payload.insert(
            "done_marker_seen".to_string(),
            Value::Bool(protocol::is_done_text(&reply_buffer)),
        );

        let item = build_item(
            submission,
            CompletionItemKind::AssistantFinal,
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
            .insert("anchor_seen".to_string(), Value::Bool(true));
        updated
            .runtime_state
            .insert("reply_buffer".to_string(), Value::String(reply_buffer));

        let decision = CompletionDecision {
            terminal: true,
            status: CompletionStatus::Completed,
            reason: Some("done".to_string()),
            confidence: Some(CompletionConfidence::Exact),
            reply: cleaned,
            anchor_seen: true,
            reply_started: true,
            reply_stable: true,
            provider_turn_ref: Some(request_anchor),
            source_cursor: Some(item.cursor.clone()),
            finished_at: Some(now.to_string()),
            diagnostics: Default::default(),
        };

        Some(ProviderPollResult::new(updated, vec![item], Some(decision)))
    }
}

// ---------------------------------------------------------------------------
// Protocol helpers
// ---------------------------------------------------------------------------

/// A request to the Kimi provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KimiRequest {
    pub client_id: String,
    pub work_dir: String,
    pub timeout_s: f64,
    pub quiet: bool,
    pub message: String,
    #[serde(default)]
    pub req_id: Option<String>,
    #[serde(default = "default_caller")]
    pub caller: String,
}

fn default_caller() -> String {
    "claude".to_string()
}

/// The result of a Kimi request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KimiResult {
    pub exit_code: i32,
    pub reply: String,
    pub req_id: String,
    pub session_key: String,
    pub done_seen: bool,
    #[serde(default)]
    pub done_ms: Option<u64>,
    #[serde(default)]
    pub anchor_seen: bool,
    #[serde(default)]
    pub fallback_scan: bool,
    #[serde(default)]
    pub anchor_ms: Option<u64>,
}

/// Build the per-agent Kimi context projection path.
pub fn kimi_context_path(workspace_path: &Path, agent_name: &str) -> PathBuf {
    workspace_path
        .join(".ccb")
        .join("agents")
        .join(agent_name.trim())
        .join("provider-state")
        .join(PROVIDER_NAME)
        .join("home")
        .join(KIMI_CONTEXT_FILENAME)
}

/// Wrap a prompt with Kimi request/done marker instructions and context pointer.
pub fn wrap_kimi_prompt(message: &str, req_id: &str, context_path: Option<&Path>) -> String {
    let rendered = message.trim_end();
    let mut lines = vec![
        format!("{} {}", protocol::REQ_ID_PREFIX, req_id),
        String::new(),
    ];
    if let Some(path) = context_path {
        lines.push(format!(
            "Before answering, read the CCB Kimi context file if available: {}",
            path.display()
        ));
        lines.push(String::new());
    }
    lines.extend([
        rendered.to_string(),
        String::new(),
        "IMPORTANT:".to_string(),
        "- Reply with a concise execution receipt. Do not stay silent.".to_string(),
        "- Do not reply only with tool/progress/status text.".to_string(),
        "- End your reply with this exact final line (verbatim, on its own line):".to_string(),
        protocol::done_marker(req_id),
    ]);
    lines.join("\n") + "\n"
}

/// Generate a short request ID from a job ID.
pub fn make_req_id(job_id: &str) -> String {
    protocol::make_req_id(job_id)
}

/// Check whether a reply buffer contains any done marker.
pub fn is_done_text(text: &str) -> bool {
    protocol::is_done_text(text)
}

/// Strip done markers from a reply buffer.
pub fn strip_done_text(text: &str) -> String {
    protocol::strip_done_text(text)
}

/// Detect Kimi K2.7 prompt readiness in a pane snapshot.
pub fn looks_like_kimi_input_box_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains("│ >") && trimmed.contains("K2.7 Code") && trimmed.contains("context:")
}

/// Detect pane fallback text that is only Kimi tool/progress chatter.
pub fn looks_like_kimi_non_answer(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();
    let progress_prefixes = [
        "using ",
        "used ",
        "read ",
        "reading ",
        "run ",
        "running ",
        "todo",
        "thinking",
        "user wants",
        "user asks",
        "user requests",
        "user requested",
        "the user wants",
    ];
    progress_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Clean pane fallback text without dropping a full Kimi receipt.
pub fn clean_kimi_pane_reply(text: &str, req_id: &str) -> String {
    let done_marker = protocol::done_marker(req_id);
    let mut cleaned = text.replace(&done_marker, "");
    cleaned = cleaned
        .lines()
        .filter(|line| !looks_like_kimi_input_box_line(line))
        .collect::<Vec<_>>()
        .join("\n");
    cleaned.trim().to_string()
}

/// Extract the reply text for a specific request ID.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    let done_marker = protocol::done_marker(req_id);
    let req_index = text.find(req_id).map(|p| p + req_id.len()).unwrap_or(0);
    let end = text[req_index..]
        .find(&done_marker)
        .map(|p| req_index + p)
        .unwrap_or(text.len());
    let candidate = if req_index <= end {
        &text[req_index..end]
    } else {
        ""
    };
    let cleaned = clean_kimi_pane_reply(candidate, req_id);
    if looks_like_kimi_non_answer(&cleaned) {
        String::new()
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded Kimi project session.
#[derive(Debug, Clone, Default)]
pub struct KimiProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl KimiProjectSession {
    pub fn kimi_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn kimi_session_path(&self) -> String {
        self.data
            .get(SESSION_PATH_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn kimi_context_path(&self) -> String {
        self.data
            .get(CONTEXT_PATH_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }
}

/// Find a project session file for a work directory.
pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

/// Load a Kimi project session.
pub fn load_project_session(work_dir: &Path, instance: Option<&str>) -> Option<KimiProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(KimiProjectSession { session_file, data })
}
