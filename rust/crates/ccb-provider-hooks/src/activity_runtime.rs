//! Mirrors Python `lib/provider_hooks/activity_runtime/events.py`
//!
//! This module is consolidated into `activity.rs` in the Rust implementation.
//! All functionality is re-exported from the parent module.

// Re-export all items from activity.rs for Python compatibility
pub use crate::activity::{
    activity_path, load_activity, normalize_activity_state, read_activity_evidence, write_activity,
    ProviderActivityEvidence,
    ACTIVITY_ACTIVE, ACTIVITY_FAILED, ACTIVITY_IDLE, ACTIVITY_PENDING, ACTIVITY_STATES, SCHEMA_VERSION,
};
