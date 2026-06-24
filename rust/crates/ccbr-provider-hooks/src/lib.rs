pub mod activity;
pub mod activity_runtime;
pub mod artifacts;
pub mod artifacts_runtime;
pub mod notifications;
pub mod settings;
pub mod settings_runtime;

// Top-level exports aligned with Python `provider_hooks.__init__.__all__` (24 items).
pub use activity::{
    activity_path, load_activity, normalize_activity_state, read_activity_evidence, write_activity,
    ProviderActivityEvidence,
};
pub use artifacts::{
    completion_dir_from_session_data, current_turn_req_id_from_transcript, event_path,
    extract_req_id, latest_req_id_from_transcript, load_event, write_event,
};
pub use notifications::{
    completion_status_label, completion_status_marker, default_reply_for_status,
    normalize_completion_status, COMPLETION_STATUS_CANCELLED, COMPLETION_STATUS_COMPLETED,
    COMPLETION_STATUS_FAILED, COMPLETION_STATUS_INCOMPLETE,
};
pub use settings::{
    build_activity_hook_command, build_hook_command, install_workspace_activity_hooks,
    install_workspace_completion_hooks,
};

// Submodule helpers that are public in Python `settings_runtime/common.py` and
// `settings_runtime/claude.py` and are therefore also exposed from the crate root.
pub use settings::{claude_hook_home_layout, load_json, save_json, workspace_key};

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] ccbr_storage::StorageError),
    #[error("unsupported state: {0}")]
    UnsupportedState(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, HookError>;
