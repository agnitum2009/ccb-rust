use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccbr_provider_core::manifest::ProviderManifest;
use ccbr_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccbr_provider_core::runtime_shared::provider_start_parts;
use serde_json::Value;

use crate::native_cli_support::{
    NativeCliExecutionAdapter, NativeCliExecutionConfig, NativeCliExecutionRequest,
    NativeCliObservation, OutputKind,
};
use crate::providers::pane_backed_manifest;

pub const PROVIDER_NAME: &str = "pi";

const SESSION_FILENAME: &str = ".pi-session";
const SESSION_ID_ATTR: &str = "pi_session_id";
const SESSION_PATH_ATTR: &str = "pi_session_path";

// ---------------------------------------------------------------------------
// Manifest / backend
// ---------------------------------------------------------------------------

/// Build the Pi provider manifest.
pub fn manifest() -> ProviderManifest {
    pane_backed_manifest(PROVIDER_NAME, false)
}

/// Build the Pi provider backend registration.
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

/// Build a generic native CLI execution adapter configured for Pi.
pub fn build_execution_adapter() -> NativeCliExecutionAdapter {
    NativeCliExecutionAdapter::new(
        NativeCliExecutionConfig::new(PROVIDER_NAME, _build_command)
            .with_env_builder(_build_env)
            .with_observer(observe_pi_json_output)
            .with_output_kind(OutputKind::Jsonl)
            .with_reason("start_failed", "pi_run_start_failed")
            .with_reason("failed", "pi_run_failed")
            .with_reason("empty", "pi_empty_reply")
            .with_reason("run_error", "pi_run_error")
            .with_reason("complete", "pi_run_stop")
            .with_reason("process_exit_complete", "pi_run_exit")
            .with_reason("timeout", "pi_run_timeout"),
    )
}

fn _build_command(request: NativeCliExecutionRequest) -> Vec<String> {
    let session_dir = request.state_path("pi_session_dir", "sessions");
    let _ = std::fs::create_dir_all(&session_dir);
    let mut cmd = provider_start_parts(PROVIDER_NAME);
    cmd.push("--mode".to_string());
    cmd.push("json".to_string());
    cmd.push("--session-dir".to_string());
    cmd.push(session_dir.to_string_lossy().to_string());
    cmd.push("--no-approve".to_string());
    cmd.push("--name".to_string());
    cmd.push(request.job_id.clone());
    cmd.push(request.prompt.clone());
    cmd
}

fn _build_env(request: &NativeCliExecutionRequest) -> HashMap<String, String> {
    let pi_home = request.state_path("pi_home", "home");
    let session_dir = request.state_path("pi_session_dir", "sessions");
    let _ = std::fs::create_dir_all(&pi_home);
    let _ = std::fs::create_dir_all(&session_dir);
    let mut env = HashMap::new();
    env.insert(
        "PI_CODING_AGENT_DIR".to_string(),
        pi_home.to_string_lossy().to_string(),
    );
    env.insert(
        "PI_CODING_AGENT_SESSION_DIR".to_string(),
        session_dir.to_string_lossy().to_string(),
    );
    env.insert("PI_SKIP_VERSION_CHECK".to_string(), "1".to_string());
    env.insert("PI_TELEMETRY".to_string(), "0".to_string());
    env
}

/// Custom JSONL observer for Pi output.
pub fn observe_pi_json_output(path: &Path) -> NativeCliObservation {
    if path.as_os_str().is_empty() || !path.is_file() {
        return NativeCliObservation::default();
    }
    let lines = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(exc) => {
            return NativeCliObservation {
                error: format!("read_stdout_failed:{}", exc),
                ..Default::default()
            }
        }
    };

    let mut finished = false;
    let mut finish_reason = String::new();
    let mut turn_ref: Option<String> = None;
    let mut completed_at: Option<Value> = None;
    let mut error = String::new();
    let mut intermediate = false;
    let mut delta_chunks: Vec<String> = Vec::new();
    let mut latest_message_text = String::new();
    let mut final_text = String::new();

    for line in lines.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let event: serde_json::Map<String, Value> = match serde_json::from_str::<Value>(stripped) {
            Ok(Value::Object(event)) => event,
            _ => continue,
        };

        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_lowercase()
            .replace('-', "_");
        if event_type.contains("error") || event_type.contains("failed") {
            error =
                pi_text(event.get("message").unwrap_or(&Value::Null)).unwrap_or(event_type.clone());
            continue;
        }
        if event_type.contains("tool") {
            intermediate = true;
            if finish_reason.is_empty() {
                finish_reason = "tool_calls".to_string();
            }
            continue;
        }

        if let Some(Value::Object(message)) = event.get("message") {
            if pi_message_role(message) == "assistant" {
                if let Some(text) = pi_message_text(message) {
                    latest_message_text = text;
                    turn_ref = turn_ref.or_else(|| pi_ref(message));
                    completed_at = completed_at.clone().or_else(|| pi_time(&event));
                }
            }
        }

        if let Some(Value::Object(assistant_event)) = event.get("assistantMessageEvent") {
            if let Some(Value::String(delta)) = assistant_event.get("delta") {
                if !delta.is_empty() {
                    delta_chunks.push(delta.clone());
                }
            }
        }

        if event_type == "turn_end" {
            finished = true;
            finish_reason = "turn_end".to_string();
            final_text = if let Some(Value::Object(message)) = event.get("message") {
                pi_message_text(message).unwrap_or_default()
            } else {
                latest_message_text.clone()
            };
            turn_ref = turn_ref.or_else(|| pi_ref(&event));
            completed_at = completed_at.clone().or_else(|| pi_time(&event));
        } else if event_type == "agent_end" {
            finished = true;
            if finish_reason.is_empty() {
                finish_reason = "agent_end".to_string();
            }
            if final_text.is_empty() {
                final_text = last_assistant_message_text(event.get("messages"))
                    .unwrap_or_else(|| latest_message_text.clone());
            }
            turn_ref = turn_ref.or_else(|| pi_ref(&event));
            completed_at = completed_at.clone().or_else(|| pi_time(&event));
        }
    }

    let text = if !final_text.is_empty() {
        final_text
    } else if !latest_message_text.is_empty() {
        latest_message_text
    } else {
        delta_chunks.join("")
    };

    NativeCliObservation {
        text,
        finished,
        finish_reason,
        turn_ref,
        completed_at,
        error,
        intermediate,
    }
}

fn pi_message_role(message: &serde_json::Map<String, Value>) -> String {
    message
        .get("role")
        .or_else(|| message.get("sender"))
        .or_else(|| message.get("author"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

fn pi_message_text(message: &serde_json::Map<String, Value>) -> Option<String> {
    pi_text(message.get("content").unwrap_or(&Value::Null))
}

fn last_assistant_message_text(messages: Option<&Value>) -> Option<String> {
    let messages = messages?.as_array()?;
    for message in messages.iter().rev() {
        if let Value::Object(obj) = message {
            if pi_message_role(obj) == "assistant" {
                if let Some(text) = pi_message_text(obj) {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn pi_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(arr) => {
            let text: String = arr.iter().filter_map(pi_text).collect();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Value::Object(obj) => {
            for key in [
                "text", "delta", "content", "message", "payload", "data", "part",
            ] {
                if let Some(v) = obj.get(key) {
                    if let Some(text) = pi_text(v) {
                        return Some(text);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn pi_ref(value: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["id", "message_id", "session_id", "turn_id", "request_id"] {
        if let Some(Value::String(s)) = value.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for key in ["message", "payload", "data"] {
        if let Some(Value::Object(nested)) = value.get(key) {
            if let Some(ref_value) = pi_ref(nested) {
                return Some(ref_value);
            }
        }
    }
    None
}

fn pi_time(value: &serde_json::Map<String, Value>) -> Option<Value> {
    for key in [
        "completed_at",
        "timestamp",
        "time",
        "created_at",
        "updated_at",
    ] {
        if let Some(v) = value.get(key) {
            if !v.is_null() {
                return Some(v.clone());
            }
        }
    }
    for key in ["message", "payload", "data"] {
        if let Some(Value::Object(nested)) = value.get(key) {
            if let Some(time) = pi_time(nested) {
                return Some(time);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Session helpers
// ---------------------------------------------------------------------------

/// A loaded Pi project session.
#[derive(Debug, Clone, Default)]
pub struct PiProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl PiProjectSession {
    pub fn pi_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn pi_session_path(&self) -> String {
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

/// Load a Pi project session.
pub fn load_project_session(work_dir: &Path, instance: Option<&str>) -> Option<PiProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(PiProjectSession { session_file, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest() {
        let m = manifest();
        assert_eq!(m.provider, PROVIDER_NAME);
        assert!(m.supports_runtime_mode(&ccbr_provider_core::manifest::RuntimeMode::PaneBacked));
    }

    #[test]
    fn test_backend_has_session_binding_and_launcher() {
        let b = backend();
        assert_eq!(b.provider(), PROVIDER_NAME);
        assert!(b.session_binding.is_some());
        assert!(b.runtime_launcher.is_some());
    }

    #[test]
    fn test_build_execution_adapter_provider_name() {
        let adapter = build_execution_adapter();
        assert_eq!(adapter.provider(), PROVIDER_NAME);
    }

    #[test]
    fn test_observe_pi_json_output_turn_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("pi.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"assistantMessageEvent","assistantMessageEvent":{"delta":"hello "}}
{"type":"turn_end","message":{"role":"assistant","content":{"text":"world"}}}
"#,
        )
        .unwrap();
        let obs = observe_pi_json_output(&path);
        assert_eq!(obs.text, "world");
        assert!(obs.finished);
        assert_eq!(obs.finish_reason, "turn_end");
    }

    #[test]
    fn test_observe_pi_json_output_agent_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("pi.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"agent_end","messages":[{"role":"assistant","content":{"text":"done"}}]}
"#,
        )
        .unwrap();
        let obs = observe_pi_json_output(&path);
        assert_eq!(obs.text, "done");
        assert!(obs.finished);
    }

    #[test]
    fn test_load_project_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_path = tmp.path().join(SESSION_FILENAME);
        std::fs::write(&session_path, r#"{"pi_session_id":"s1"}"#).unwrap();

        let session = load_project_session(tmp.path(), None).unwrap();
        assert_eq!(session.pi_session_id(), "s1");
        assert_eq!(session.session_file, session_path);
    }
}
