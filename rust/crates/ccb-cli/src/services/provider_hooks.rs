//! Mirrors Python `lib/cli/services/provider_hooks.py`.
//!
//! CLI-side orchestration is now implemented in `ccb-providers` so that the
//! daemon can share it without depending on the CLI crate. This module
//! re-exports the public API for backward compatibility and 1:1 path parity.

pub use ccb_providers::workspace_preparation::{
    prepare_provider_workspace, prepare_workspace_provider_hooks, provider_hook_home_root,
    resolve_gemini_home_root,
};
