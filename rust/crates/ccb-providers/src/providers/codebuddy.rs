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
use ccb_provider_core::runtime_shared::provider_start_parts;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::execution::{
    build_item, ExecutionAdapter, ProviderPollResult, ProviderRuntimeContext, ProviderSubmission,
};
use crate::native_cli_support::{
    NativeCliExecutionAdapter, NativeCliExecutionConfig, NativeCliExecutionRequest, OutputKind,
};
use crate::providers::pane_backed_manifest;

pub const PROVIDER_NAME: &str = "codebuddy";

const SESSION_FILENAME: &str = ".codebuddy-session";
const SESSION_ID_ATTR: &str = "codebuddy_session_id";
const SESSION_PATH_ATTR: &str = "codebuddy_session_path";

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the CodeBuddy provider manifest.
pub fn manifest() -> ProviderManifest {
    pane_backed_manifest(PROVIDER_NAME, false)
}

/// Build the CodeBuddy provider backend registration.
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
// Native CLI execution adapter
// ---------------------------------------------------------------------------

/// Build a generic native CLI execution adapter configured for CodeBuddy.
pub fn build_execution_adapter() -> NativeCliExecutionAdapter {
    NativeCliExecutionAdapter::new(
        NativeCliExecutionConfig::new(PROVIDER_NAME, _build_command)
            .with_output_kind(OutputKind::Jsonl)
            .with_reason("start_failed", "codebuddy_run_start_failed")
            .with_reason("failed", "codebuddy_run_failed")
            .with_reason("empty", "codebuddy_empty_reply")
            .with_reason("run_error", "codebuddy_run_error")
            .with_reason("complete", "codebuddy_run_stop")
            .with_reason("process_exit_complete", "codebuddy_run_exit")
            .with_reason("timeout", "codebuddy_run_timeout"),
    )
}

fn _build_command(request: NativeCliExecutionRequest) -> Vec<String> {
    let mut cmd = provider_start_parts(PROVIDER_NAME);
    cmd.push(request.prompt.clone());
    cmd
}

// ---------------------------------------------------------------------------
// Legacy stub execution adapter (kept for direct test compatibility)
// ---------------------------------------------------------------------------

/// CodeBuddy execution adapter.
pub struct CodeBuddyExecutionAdapter;

impl ExecutionAdapter for CodeBuddyExecutionAdapter {
    fn provider(&self) -> &str {
        PROVIDER_NAME
    }

    fn start(
        &self,
        job: &JobRecord,
        _context: Option<&ProviderRuntimeContext>,
        now: &str,
    ) -> ProviderSubmission {
        let request_anchor = protocol::request_anchor_for_job(&job.job_id);
        let req_id = protocol::make_req_id(&job.job_id);
        let prompt_text = wrap_codebuddy_prompt(&job.request.body, &req_id);

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

/// A request to the CodeBuddy provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBuddyRequest {
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

/// The result of a CodeBuddy request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBuddyResult {
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

/// Wrap a prompt with CodeBuddy request/done marker instructions.
///
/// Mirrors Python `provider_backends.codebuddy.protocol_runtime.wrap_codebuddy_prompt`,
/// adapted to the Rust `ccb-provider-core` marker format (`<<DONE:req-id>>`).
pub fn wrap_codebuddy_prompt(message: &str, req_id: &str) -> String {
    let rendered = message.trim_end();
    let lines = [
        format!("{} {}", protocol::REQ_ID_PREFIX, req_id),
        String::new(),
        rendered.to_string(),
        String::new(),
        "IMPORTANT:".to_string(),
        "- Reply with an execution summary, in English. Do not stay silent.".to_string(),
        "- End your reply with this exact final line (verbatim, on its own line):".to_string(),
        protocol::done_marker(req_id),
    ];
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

/// Extract the reply text for a specific request ID.
pub fn extract_reply_for_req(text: &str, req_id: &str) -> String {
    let begin_marker = format!("{}{}>>", protocol::BEGIN_PREFIX, req_id);
    let done_marker = format!("{}{}>>", protocol::DONE_PREFIX, req_id);
    let spaced_done_marker = format!("{} {}>>", protocol::DONE_PREFIX, req_id);

    let start = text
        .find(&begin_marker)
        .map(|p| p + begin_marker.len())
        .unwrap_or(0);

    if let Some(end) = text
        .find(&done_marker)
        .or_else(|| text.find(&spaced_done_marker))
    {
        return if start <= end {
            text[start..end].trim().to_string()
        } else {
            String::new()
        };
    }

    // A done marker for a different request id means this turn is not ours.
    if text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(protocol::DONE_PREFIX) && trimmed.ends_with(">>")
    }) {
        return String::new();
    }

    text.trim().to_string()
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded CodeBuddy project session.
#[derive(Debug, Clone, Default)]
pub struct CodebuddyProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl CodebuddyProjectSession {
    pub fn codebuddy_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn codebuddy_session_path(&self) -> String {
        self.data
            .get(SESSION_PATH_ATTR)
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

/// Load a CodeBuddy project session.
pub fn load_project_session(
    work_dir: &Path,
    instance: Option<&str>,
) -> Option<CodebuddyProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(CodebuddyProjectSession { session_file, data })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_execution_adapter_provider_name() {
        let adapter = CodeBuddyExecutionAdapter;
        assert_eq!(adapter.provider(), PROVIDER_NAME);
    }

    #[test]
    fn test_start_creates_active_submission() {
        let adapter = CodeBuddyExecutionAdapter;
        let job = JobRecord::new("j1", "agent1", PROVIDER_NAME);
        let submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");

        assert_eq!(submission.job_id, "j1");
        assert_eq!(submission.provider, PROVIDER_NAME);
        assert_eq!(
            submission.source_kind,
            CompletionSourceKind::ProtocolEventStream
        );
        assert!(!submission.is_terminal());
        assert_eq!(submission.runtime_state.get("mode").unwrap(), "active");
        assert!(!submission
            .runtime_state
            .get("prompt_sent")
            .unwrap()
            .as_bool()
            .unwrap());
    }

    #[test]
    fn test_poll_returns_none_without_done_marker() {
        let adapter = CodeBuddyExecutionAdapter;
        let job = JobRecord::new("j2", "agent2", PROVIDER_NAME);
        let submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");
        assert!(adapter.poll(&submission, "2025-01-01T00:00:01Z").is_none());
    }

    #[test]
    fn test_poll_emits_terminal_decision_on_done_marker() {
        let adapter = CodeBuddyExecutionAdapter;
        let job = JobRecord::new("j3", "agent3", PROVIDER_NAME);
        let mut submission = adapter.start(&job, None, "2025-01-01T00:00:00Z");

        let req_id = make_req_id(&job.job_id);
        let reply = format!(
            "{}\nhello codebuddy\n{}{}>>",
            protocol::request_anchor_for_job(&job.job_id),
            protocol::DONE_PREFIX,
            req_id
        );
        submission
            .runtime_state
            .insert("reply_buffer".to_string(), Value::String(reply));

        let result = adapter
            .poll(&submission, "2025-01-01T00:00:01Z")
            .expect("expected poll result");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].kind, CompletionItemKind::AssistantFinal);

        let decision = result.decision.expect("expected terminal decision");
        assert!(decision.terminal);
        assert_eq!(decision.status, CompletionStatus::Completed);
        assert_eq!(decision.reply, "hello codebuddy");
    }

    #[test]
    fn test_wrap_codebuddy_prompt_format() {
        let wrapped = wrap_codebuddy_prompt("do the thing", "req-12345678");
        assert!(wrapped.contains("req-12345678"));
        assert!(wrapped.contains("do the thing"));
        assert!(wrapped.contains(protocol::DONE_PREFIX));
        assert!(wrapped.ends_with('\n'));
    }

    #[test]
    fn test_extract_reply_for_req() {
        let text = "<<BEGIN:req-12345678>>\nhello codebuddy\n<<DONE:req-12345678>>";
        assert_eq!(
            extract_reply_for_req(text, "req-12345678"),
            "hello codebuddy"
        );
    }

    #[test]
    fn test_request_and_result_defaults() {
        let req = CodeBuddyRequest {
            client_id: "client-1".into(),
            work_dir: "/tmp".into(),
            timeout_s: 60.0,
            quiet: false,
            message: "hello".into(),
            req_id: None,
            caller: default_caller(),
        };
        assert_eq!(req.caller, "claude");
        assert!(req.req_id.is_none());

        let result = CodeBuddyResult {
            exit_code: 0,
            reply: "test".into(),
            req_id: "abc".into(),
            session_key: "codebuddy:xyz".into(),
            done_seen: true,
            done_ms: Some(1500),
            anchor_seen: false,
            fallback_scan: false,
            anchor_ms: None,
        };
        assert!(result.done_seen);
        assert!(!result.anchor_seen);
    }

    #[test]
    fn test_load_project_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_path = tmp.path().join(SESSION_FILENAME);
        std::fs::write(&session_path, r#"{"codebuddy_session_id":"s1"}"#).unwrap();

        let session = load_project_session(tmp.path(), None).unwrap();
        assert_eq!(session.codebuddy_session_id(), "s1");
        assert_eq!(session.session_file, session_path);
    }
}
