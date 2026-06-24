//! Mirrors Python `lib/provider_backends/claude/registry_support/logs_runtime/discovery.py`.

pub use super::discovery_runtime::{
    extract_session_id_from_start_cmd, find_log_for_session_id, scan_latest_log_for_work_dir,
};
