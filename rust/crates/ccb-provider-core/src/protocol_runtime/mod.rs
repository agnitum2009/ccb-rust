//! Python-aligned protocol runtime.
//!
//! This module mirrors the public API of Python's `provider_core.protocol_runtime`
//! and is re-exported at the crate root. The legacy `crate::protocol` module is
//! kept unchanged for existing consumers.

pub mod constants;
pub mod prompt;
pub mod reply;
pub mod request_id;

pub use constants::{
    done_line_re, is_trailing_noise_line, ANY_DONE_LINE_RE, ANY_REQ_ID_PATTERN, BEGIN_PREFIX,
    DONE_PREFIX, REQ_ID_BOUNDARY_PATTERN, REQ_ID_PREFIX,
};
pub use prompt::{wrap_codex_prompt, wrap_codex_turn_prompt};
pub use reply::{extract_reply_for_req, is_done_text, strip_done_text, strip_trailing_markers};
pub use request_id::{make_req_id, request_anchor_for_job};

pub mod reply_runtime;