//! Mirrors Python `lib/ccbd/reload_handoff.py`.

use crate::app::CcbdApp;
use serde_json::Value;

/// Opaque handle for an in-progress reload handoff.
#[derive(Debug, Clone)]
pub struct ReloadHandoff {
    pub target_config_identity: Value,
}

/// Begin a reload handoff and return a handle.
pub fn begin_reload_handoff(
    _app: &mut CcbdApp,
    target_config_identity: Value,
) -> Option<ReloadHandoff> {
    Some(ReloadHandoff {
        target_config_identity,
    })
}

/// Clear any in-progress reload handoff.
pub fn clear_reload_handoff(_app: &mut CcbdApp) {}
