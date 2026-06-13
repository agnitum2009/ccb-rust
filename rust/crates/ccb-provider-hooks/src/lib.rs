pub mod activity;
pub mod artifacts;
pub mod notifications;
pub mod settings;

pub use activity::{
    activity_path, load_activity, normalize_activity_state, read_activity_evidence, write_activity,
    ProviderActivityEvidence, ACTIVITY_ACTIVE, ACTIVITY_FAILED, ACTIVITY_IDLE, ACTIVITY_PENDING,
    ACTIVITY_STATES, SCHEMA_VERSION as ACTIVITY_SCHEMA_VERSION,
};
pub use artifacts::{
    completion_dir_from_session_data, current_turn_req_id_from_transcript,
    current_turn_req_id_from_transcript_text, event_path, extract_outer_req_id, extract_req_id,
    latest_last_prompt_req_id_from_transcript_text, latest_req_id_from_transcript,
    latest_req_id_from_transcript_text, latest_user_req_id_from_transcript_text, load_event,
    write_event, SCHEMA_VERSION as ARTIFACTS_SCHEMA_VERSION,
};
pub use notifications::{
    completion_status_label, completion_status_marker, default_reply_for_status,
    normalize_completion_status, COMPLETION_STATUS_CANCELLED, COMPLETION_STATUS_COMPLETED,
    COMPLETION_STATUS_FAILED, COMPLETION_STATUS_INCOMPLETE, VALID_COMPLETION_STATUSES,
};
pub use settings::{
    build_activity_hook_command, build_hook_command, install_claude_activity_hooks,
    install_claude_hooks, install_gemini_hooks, install_workspace_activity_hooks,
    install_workspace_completion_hooks, trust_claude_workspace, trust_gemini_workspace,
};

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),
    #[error("unsupported state: {0}")]
    UnsupportedState(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

pub type Result<T> = std::result::Result<T, HookError>;
