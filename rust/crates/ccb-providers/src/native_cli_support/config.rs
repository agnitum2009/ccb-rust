use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

use super::observation::NativeCliObserver;

/// Default set of finish reasons that indicate a successful stop.
pub fn default_done_markers() -> Vec<String> {
    vec![
        "stop".to_string(),
        "end_turn".to_string(),
        "turn_end".to_string(),
        "completed".to_string(),
        "complete".to_string(),
        "done".to_string(),
        "finished".to_string(),
        "success".to_string(),
        "ok".to_string(),
    ]
}

/// Output format observed from a native CLI provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputKind {
    /// Line-delimited JSON events.
    #[default]
    Jsonl,
    /// Plain stdout text.
    Stdout,
}

impl OutputKind {
    pub fn file_suffix(&self) -> &'static str {
        match self {
            OutputKind::Jsonl => "jsonl",
            OutputKind::Stdout => "out",
        }
    }
}

/// A request passed to native CLI command/env builders.
#[derive(Debug, Clone)]
pub struct NativeCliExecutionRequest {
    pub provider: String,
    pub job_id: String,
    pub work_dir: PathBuf,
    pub session_data: HashMap<String, Value>,
    pub prompt: String,
    pub request_anchor: String,
}

impl NativeCliExecutionRequest {
    /// Resolve a provider-specific state path from session data.
    pub fn state_path(&self, key: &str, fallback: &str) -> PathBuf {
        let raw = self
            .session_data
            .get(key)
            .and_then(Value::as_str)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        if let Some(path) = raw {
            return expand_home(path);
        }
        let state_dir = self
            .session_data
            .get(&format!("{}_state_dir", self.provider))
            .and_then(Value::as_str)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(expand_home)
            .unwrap_or_else(|| self.work_dir.join(".ccbr").join(&self.provider));
        state_dir.join(fallback)
    }
}

/// Trait for building a native CLI command from an execution request.
pub trait CommandBuilder: Send + Sync {
    fn build_command(&self, request: NativeCliExecutionRequest) -> Vec<String>;
}

impl<F> CommandBuilder for F
where
    F: Fn(NativeCliExecutionRequest) -> Vec<String> + Send + Sync,
{
    fn build_command(&self, request: NativeCliExecutionRequest) -> Vec<String> {
        (self)(request)
    }
}

/// Trait for building extra environment variables for a native CLI run.
pub trait EnvBuilder: Send + Sync {
    fn build_env(&self, request: &NativeCliExecutionRequest) -> HashMap<String, String>;
}

impl<F> EnvBuilder for F
where
    F: Fn(&NativeCliExecutionRequest) -> HashMap<String, String> + Send + Sync,
{
    fn build_env(&self, request: &NativeCliExecutionRequest) -> HashMap<String, String> {
        (self)(request)
    }
}

/// Configuration for a native CLI execution adapter.
pub struct NativeCliExecutionConfig {
    pub provider: String,
    pub session_filename: String,
    pub command_builder: Box<dyn CommandBuilder>,
    pub env_builder: Option<Box<dyn EnvBuilder>>,
    pub observer: Option<Box<dyn NativeCliObserver>>,
    pub output_kind: OutputKind,
    pub mode: String,
    pub start_failed_reason: String,
    pub failed_reason: String,
    pub empty_reason: String,
    pub run_error_reason: String,
    pub complete_reason: String,
    pub process_exit_complete_reason: String,
    pub timeout_reason: String,
    pub run_timeout_s: f64,
    pub terminal_on_process_exit: bool,
    pub done_markers: Vec<String>,
}

impl std::fmt::Debug for NativeCliExecutionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeCliExecutionConfig")
            .field("provider", &self.provider)
            .field("session_filename", &self.session_filename)
            .field("output_kind", &self.output_kind)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl NativeCliExecutionConfig {
    pub fn new(
        provider: impl Into<String>,
        command_builder: impl CommandBuilder + 'static,
    ) -> Self {
        let provider = provider.into().trim().to_lowercase();
        Self {
            session_filename: format!(".{}-session", provider),
            command_builder: Box::new(command_builder),
            env_builder: None,
            observer: None,
            output_kind: OutputKind::Jsonl,
            mode: format!("{}_run", provider),
            start_failed_reason: String::new(),
            failed_reason: String::new(),
            empty_reason: String::new(),
            run_error_reason: String::new(),
            complete_reason: String::new(),
            process_exit_complete_reason: String::new(),
            timeout_reason: String::new(),
            run_timeout_s: 900.0,
            terminal_on_process_exit: true,
            done_markers: default_done_markers(),
            provider,
        }
    }

    pub fn with_session_filename(mut self, filename: impl Into<String>) -> Self {
        self.session_filename = filename.into();
        self
    }

    pub fn with_env_builder(mut self, builder: impl EnvBuilder + 'static) -> Self {
        self.env_builder = Some(Box::new(builder));
        self
    }

    pub fn with_observer(mut self, observer: impl NativeCliObserver + 'static) -> Self {
        self.observer = Some(Box::new(observer));
        self
    }

    pub fn with_output_kind(mut self, kind: OutputKind) -> Self {
        self.output_kind = kind;
        self
    }

    pub fn with_reason(mut self, name: &str, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        match name {
            "start_failed" => self.start_failed_reason = reason,
            "failed" => self.failed_reason = reason,
            "empty" => self.empty_reason = reason,
            "run_error" => self.run_error_reason = reason,
            "complete" => self.complete_reason = reason,
            "process_exit_complete" => self.process_exit_complete_reason = reason,
            "timeout" => self.timeout_reason = reason,
            _ => {}
        }
        self
    }

    pub fn with_run_timeout_s(mut self, seconds: f64) -> Self {
        self.run_timeout_s = seconds.max(0.0);
        self
    }

    pub fn with_terminal_on_process_exit(mut self, terminal: bool) -> Self {
        self.terminal_on_process_exit = terminal;
        self
    }

    pub fn with_done_markers(mut self, markers: Vec<String>) -> Self {
        self.done_markers = markers;
        self
    }

    /// Return the configured reason string, falling back to a generated token.
    pub fn reason(&self, name: &str) -> String {
        let explicit = match name {
            "start_failed" => &self.start_failed_reason,
            "failed" => &self.failed_reason,
            "empty" => &self.empty_reason,
            "run_error" => &self.run_error_reason,
            "complete" => &self.complete_reason,
            "process_exit_complete" => &self.process_exit_complete_reason,
            "timeout" => &self.timeout_reason,
            _ => "",
        };
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        let stem = name.trim_end_matches("_reason");
        format!("{}_{}", self.provider, stem)
    }
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest.strip_prefix('/').unwrap_or(rest));
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_kind_suffix() {
        assert_eq!(OutputKind::Jsonl.file_suffix(), "jsonl");
        assert_eq!(OutputKind::Stdout.file_suffix(), "out");
    }

    #[test]
    fn test_config_defaults() {
        let config = NativeCliExecutionConfig::new("crush", |_req| vec!["crush".to_string()]);
        assert_eq!(config.provider, "crush");
        assert_eq!(config.session_filename, ".crush-session");
        assert_eq!(config.output_kind, OutputKind::Jsonl);
        assert_eq!(config.reason("complete"), "crush_complete");
    }

    #[test]
    fn test_config_custom_reason() {
        let config = NativeCliExecutionConfig::new("crush", |_req| vec!["crush".to_string()])
            .with_reason("complete", "custom_stop");
        assert_eq!(config.reason("complete"), "custom_stop");
    }

    #[test]
    fn test_request_state_path_uses_session_data() {
        let mut session_data = HashMap::new();
        session_data.insert(
            "crush_data_dir".to_string(),
            Value::String("/data".to_string()),
        );
        let req = NativeCliExecutionRequest {
            provider: "crush".to_string(),
            job_id: "j1".to_string(),
            work_dir: PathBuf::from("/tmp/ws"),
            session_data,
            prompt: "hi".to_string(),
            request_anchor: "a1".to_string(),
        };
        assert_eq!(
            req.state_path("crush_data_dir", "data"),
            PathBuf::from("/data")
        );
    }
}
