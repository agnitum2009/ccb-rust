use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// A single message in a conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub role: String,
    pub content: String,
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<serde_json::Value>,
}

/// A complete tool execution with input and result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    pub tool_id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<String>,
    #[serde(default)]
    pub is_error: bool,
}

/// Statistics about a session's activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    #[serde(default)]
    pub tool_calls: HashMap<String, u32>,
    #[serde(default)]
    pub tool_executions: Vec<ToolExecution>,
    #[serde(default)]
    pub files_written: Vec<String>,
    #[serde(default)]
    pub files_read: Vec<String>,
    #[serde(default)]
    pub files_edited: Vec<String>,
    #[serde(default)]
    pub bash_commands: Vec<String>,
    #[serde(default)]
    pub tasks_created: u32,
    #[serde(default)]
    pub tasks_completed: u32,
}

/// Context prepared for transfer to another provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferContext {
    pub conversations: Vec<(String, String)>,
    pub source_session_id: String,
    pub token_estimate: u32,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub stats: Option<SessionStats>,
    #[serde(default = "default_provider")]
    pub source_provider: String,
}

fn default_provider() -> String {
    "claude".into()
}

/// Information about a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub session_path: String,
    pub project_path: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
    pub last_modified: Option<f64>,
    pub provider: Option<String>,
}

/// Errors raised by the memory crate.
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session parse error: {0}")]
    SessionParse(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("storage error: {0}")]
    Storage(#[from] ccbr_storage::StorageError),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

// ---------------------------------------------------------------------------
// Project memory types
// ---------------------------------------------------------------------------

use std::path::PathBuf;

/// Result of ensuring a project memory seed file exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMemoryEnsureResult {
    pub path: PathBuf,
    pub seed_path: PathBuf,
    pub created: bool,
    pub seed_written: bool,
    pub sha256: String,
    pub warning: String,
}

/// A loaded project memory source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMemorySource {
    pub kind: String,
    pub title: String,
    pub path: PathBuf,
    pub content: String,
    pub exists: bool,
    pub warning: String,
    pub filtered: bool,
    pub filter_names: Vec<String>,
}

impl ProjectMemorySource {
    pub fn new(
        kind: impl Into<String>,
        title: impl Into<String>,
        path: impl Into<PathBuf>,
        content: impl Into<String>,
        exists: bool,
    ) -> Self {
        Self {
            kind: kind.into(),
            title: title.into(),
            path: path.into(),
            content: content.into(),
            exists,
            warning: String::new(),
            filtered: false,
            filter_names: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warning = warning.into();
        self
    }

    pub fn with_filtered(mut self, filter_names: Vec<String>) -> Self {
        self.filtered = !filter_names.is_empty();
        self.filter_names = filter_names;
        self
    }
}

/// A reference to a project memory source used in materialization results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMemorySourceRef {
    pub kind: String,
    pub path: PathBuf,
    pub exists: bool,
    pub sha256: String,
    pub warning: String,
    pub filtered: bool,
    pub filter_names: Vec<String>,
}

/// Result of materializing a runtime memory bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMemoryMaterialization {
    pub path: PathBuf,
    pub written: bool,
    pub unchanged: bool,
    pub sha256: String,
    pub sources: Vec<ProjectMemorySourceRef>,
    pub warnings: Vec<String>,
}
