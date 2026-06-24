//! Mirrors Python `lib/ccbrd/reload_runtime_unload.py`.

use crate::app::CcbdApp;
use crate::reload_apply_models::ServiceGraph;
use crate::reload_plan::ReloadPlan;

/// Check for a blocker that requires pre-namespace unload before applying.
pub fn pre_namespace_unload_blocker(
    _app: &CcbdApp,
    _old_graph: &ServiceGraph,
    _plan: &ReloadPlan,
) -> Option<(String, String)> {
    None
}
