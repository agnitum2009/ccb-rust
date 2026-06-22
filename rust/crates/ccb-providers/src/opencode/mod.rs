pub mod launcher;
pub mod logs;
pub mod paths;
pub mod reader;
pub mod replies;
pub mod session;
pub mod storage;

pub use launcher::{
    build_runtime_launcher, build_session_payload, build_start_cmd,
    materialize_opencode_memory_config, prepare_launch_context, OpenCodeLaunchContext,
    OpenCodeMemoryConfigResult, OPENCODE_CONFIG_FILENAME,
};
pub use logs::{is_cancel_log_line, latest_opencode_log_file, parse_opencode_log_epoch_s};
pub use paths::{
    compute_opencode_project_id, default_opencode_log_root, default_opencode_storage_root,
    env_truthy, is_wsl, normalize_path_for_match, path_is_same_or_parent, path_matches, req_id_re,
};
pub use reader::OpenCodeLogReader;
pub use replies::{
    conversations_from_messages, extract_req_id_from_text, extract_text,
    find_new_assistant_reply_with_state, is_aborted_error, latest_message_from_messages,
    observe_latest_assistant,
};
pub use session::{
    build_session_binding, find_project_session_file, load_project_session, OpenCodeProjectSession,
    PROVIDER_NAME as OPENCODE_PROVIDER_NAME, SESSION_FILENAME,
};
pub use storage::OpenCodeStorageAccessor;
pub mod comm;
pub mod execution;
pub mod execution_runtime;
pub mod manifest;
pub mod runtime;
pub mod session_runtime;
