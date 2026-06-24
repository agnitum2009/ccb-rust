//! Mirrors Python `lib/cli/services/provider_hooks.py`.
//!
//! Re-exported from [`crate::services::provider_hooks`] for 1:1 path parity.

pub use crate::services::provider_hooks::{
    prepare_provider_workspace, prepare_workspace_provider_hooks, provider_hook_home_root,
    resolve_gemini_home_root,
};
