//! Mirrors Python `lib/ccbd/services/health_assessment/tmux_runtime/ownership.py`.

use super::backend::{OwnershipResult, TmuxBackend};

/// Inspect whether a tmux pane is owned by the current project namespace.
///
/// Mirrors Python `inspect_tmux_pane_ownership`. The default implementation
/// delegates to the backend's `inspect_ownership` method so that tests can
/// substitute project-namespace membership logic.
pub fn inspect_tmux_pane_ownership<B, S>(
    _session: &S,
    backend: &B,
    pane_id: &str,
) -> OwnershipResult
where
    B: TmuxBackend,
{
    backend.inspect_ownership(pane_id)
}
