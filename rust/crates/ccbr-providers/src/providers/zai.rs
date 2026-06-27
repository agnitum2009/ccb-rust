use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccbr_provider_core::contracts::{
    LaunchMode, ProviderBackend, ProviderRuntimeLauncher, ProviderSessionBinding,
};
use ccbr_provider_core::manifest::{
    CompletionFamily, CompletionManifest, CompletionSourceKind, ProviderManifest, RuntimeMode,
    SelectorFamily,
};
use ccbr_provider_core::pathing::{find_session_file_for_work_dir, session_filename_for_instance};
use ccbr_provider_core::runtime_shared::provider_start_parts;
use serde_json::Value;

use crate::native_cli_support::{
    NativeCliExecutionAdapter, NativeCliExecutionConfig, NativeCliExecutionRequest,
    NativeCliObservation, OutputKind,
};

pub const PROVIDER_NAME: &str = "zai";
const SESSION_FILENAME: &str = ".zai-session";
const SESSION_ID_ATTR: &str = "zai_session_id";
const SESSION_PATH_ATTR: &str = "zai_session_path";

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
            completion_family: CompletionFamily::StructuredResult,
            completion_source_kind: CompletionSourceKind::StructuredResultStream,
            supports_exact_completion: false,
            supports_observed_completion: true,
            supports_anchor_binding: true,
            supports_reply_stability: false,
            supports_terminal_reason: true,
            selector_family: SelectorFamily::StructuredResult,
        },
    );
    ProviderManifest::new(provider, false, false, false, true, true, profiles)
}

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

pub fn build_execution_adapter() -> NativeCliExecutionAdapter {
    NativeCliExecutionAdapter::new(
        NativeCliExecutionConfig::new(PROVIDER_NAME, build_command)
            .with_session_filename(SESSION_FILENAME)
            .with_env_builder(build_env)
            .with_observer(observe_zai_output)
            .with_output_kind(OutputKind::Jsonl)
            .with_reason("start_failed", "zai_run_start_failed")
            .with_reason("failed", "zai_run_failed")
            .with_reason("empty", "zai_empty_reply")
            .with_reason("run_error", "zai_run_error")
            .with_reason("complete", "zai_run_stop")
            .with_reason("process_exit_complete", "zai_run_exit")
            .with_reason("timeout", "zai_run_timeout"),
    )
}

fn build_command(request: NativeCliExecutionRequest) -> Vec<String> {
    let mut command = provider_start_parts(PROVIDER_NAME);
    command.push("--directory".to_string());
    command.push(request.work_dir.to_string_lossy().to_string());
    command.push("--no-color".to_string());
    command.push("--prompt".to_string());
    command.push(request.prompt);
    command
}

fn build_env(request: &NativeCliExecutionRequest) -> HashMap<String, String> {
    let zai_home = request.state_path("zai_home", "home");
    let _ = std::fs::create_dir_all(&zai_home);
    HashMap::from([("HOME".to_string(), zai_home.to_string_lossy().to_string())])
}

pub fn observe_zai_output(path: &Path) -> NativeCliObservation {
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

    let mut assistant_chunks = Vec::new();
    let mut raw_chunks = Vec::new();
    let mut turn_ref = None;
    let mut completed_at = None;
    let mut error = String::new();
    let mut saw_json = false;

    for line in lines.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let event = match serde_json::from_str::<Value>(stripped) {
            Ok(Value::Object(event)) => event,
            Ok(_) => continue,
            Err(_) => {
                raw_chunks.push(stripped.to_string());
                continue;
            }
        };
        saw_json = true;
        let role = nested_text(&Value::Object(event.clone()), &["role", "sender", "author"])
            .trim()
            .to_lowercase();
        if role == "user" {
            continue;
        }
        let event_type = nested_text(
            &Value::Object(event.clone()),
            &["type", "event", "kind", "name"],
        )
        .trim()
        .to_lowercase();
        if matches!(role.as_str(), "error" | "system_error") || event_type.contains("error") {
            error = content_text(&Value::Object(event.clone()));
            if error.is_empty() {
                error = event_type;
            }
            if error.is_empty() {
                error = "zai_error".to_string();
            }
            continue;
        }
        if matches!(role.as_str(), "assistant" | "agent" | "model")
            || event_type.contains("assistant")
        {
            let text = content_text(&Value::Object(event.clone()));
            if is_progress_text(&text) {
                continue;
            }
            if !text.is_empty() {
                assistant_chunks.push(text);
                turn_ref = turn_ref.or_else(|| event_ref(&Value::Object(event.clone())));
                completed_at = completed_at.or_else(|| event_time(&Value::Object(event.clone())));
            }
        }
    }

    let mut text = assistant_chunks.join("").trim().to_string();
    if text.is_empty() && !saw_json {
        text = raw_chunks.join("\n").trim().to_string();
    }
    NativeCliObservation {
        text,
        turn_ref,
        completed_at,
        error,
        ..Default::default()
    }
}

fn is_progress_text(text: &str) -> bool {
    matches!(
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
            .as_str(),
        "using tools to help you..." | "thinking..."
    )
}

fn content_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(content_text).collect(),
        Value::Object(obj) => {
            for key in [
                "content", "text", "reply", "answer", "output", "response", "message", "data",
                "payload",
            ] {
                if let Some(nested) = obj.get(key) {
                    let text = content_text(nested);
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn nested_text(value: &Value, keys: &[&str]) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| nested_text(item, keys))
            .find(|s| !s.is_empty())
            .unwrap_or_default(),
        Value::Object(obj) => {
            for key in keys {
                if let Some(Value::String(s)) = obj.get(*key) {
                    if !s.is_empty() {
                        return s.clone();
                    }
                }
            }
            for key in ["message", "payload", "data", "result"] {
                if let Some(nested) = obj.get(key) {
                    let text = nested_text(nested, keys);
                    if !text.is_empty() {
                        return text;
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn event_ref(value: &Value) -> Option<String> {
    let obj = value.as_object()?;
    for key in ["id", "message_id", "session_id", "turn_id", "request_id"] {
        if let Some(Value::String(s)) = obj.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for key in ["message", "payload", "data", "result"] {
        if let Some(found) = obj.get(key).and_then(event_ref) {
            return Some(found);
        }
    }
    None
}

fn event_time(value: &Value) -> Option<Value> {
    let obj = value.as_object()?;
    for key in [
        "completed_at",
        "timestamp",
        "time",
        "created_at",
        "updated_at",
    ] {
        if let Some(found) = obj.get(key) {
            if !found.is_null() {
                return Some(found.clone());
            }
        }
    }
    for key in ["message", "payload", "data", "result"] {
        if let Some(found) = obj.get(key).and_then(event_time) {
            return Some(found);
        }
    }
    None
}

#[derive(Debug, Clone, Default)]
pub struct ZaiProjectSession {
    pub session_file: PathBuf,
    pub data: HashMap<String, Value>,
}

impl ZaiProjectSession {
    pub fn zai_session_id(&self) -> String {
        self.data
            .get(SESSION_ID_ATTR)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn zai_session_path(&self) -> String {
        self.data
            .get(SESSION_PATH_ATTR)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }
}

pub fn find_project_session_file(work_dir: &Path, instance: Option<&str>) -> Option<PathBuf> {
    let filename = session_filename_for_instance(SESSION_FILENAME, instance);
    find_session_file_for_work_dir(work_dir, &filename)
}

pub fn load_project_session(work_dir: &Path, instance: Option<&str>) -> Option<ZaiProjectSession> {
    let session_file = find_project_session_file(work_dir, instance)?;
    let raw = std::fs::read_to_string(&session_file).ok()?;
    let data: HashMap<String, Value> = serde_json::from_str(&raw).ok()?;
    Some(ZaiProjectSession { session_file, data })
}
