//! Mirrors Python `lib/provider_hooks/artifacts_runtime/` module
//!
//! This module consolidates Python files:
//! - `artifacts_runtime/events.py`
//! - `artifacts_runtime/paths.py`
//! - `artifacts_runtime/transcript.py`
//!
//! In the Rust implementation, most functionality is consolidated into `artifacts.rs`.

// Re-export all items from artifacts.rs for Python compatibility
pub use crate::artifacts::{
    completion_dir_from_session_data, current_turn_req_id_from_transcript,
    current_turn_req_id_from_transcript_text, event_path, extract_req_id, extract_outer_req_id,
    latest_last_prompt_req_id_from_transcript_text, latest_req_id_from_transcript,
    latest_req_id_from_transcript_text, latest_user_req_id_from_transcript_text, load_event,
    write_event, SCHEMA_VERSION,
};

// Specific implementation for `artifacts_runtime/paths.py`
// This function is split between artifacts.rs (completion_dir_from_session_data)
// and the main artifacts module. In Rust, completion_dir_from_session_data
// is already defined in artifacts.rs.
pub mod paths {
    //! Mirrors Python `lib/provider_hooks/artifacts_runtime/paths.py`

    // Re-export for Python compatibility
    pub use crate::artifacts::completion_dir_from_session_data;
}

// Specific implementation for `artifacts_runtime/transcript.py`
// All transcript parsing functions are already consolidated into artifacts.rs
pub mod transcript {
    //! Mirrors Python `lib/provider_hooks/artifacts_runtime/transcript.py`

    // Re-export all transcript-related functions from artifacts.rs
    pub use crate::artifacts::{
        current_turn_req_id_from_transcript, current_turn_req_id_from_transcript_text,
        extract_outer_req_id, extract_req_id, latest_last_prompt_req_id_from_transcript_text,
        latest_req_id_from_transcript, latest_req_id_from_transcript_text,
        latest_user_req_id_from_transcript_text,
    };
}
