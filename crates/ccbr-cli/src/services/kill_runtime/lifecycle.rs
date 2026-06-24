//! Mirrors Python `lib/cli/services/kill_runtime/lifecycle.py`.

use ccbr_storage::paths::PathLayout;

/// Destroy the project namespace and clear the start policy store.
///
/// Mirrors Python `destroy_project_namespace(context, *, force, ...)`.
pub fn destroy_project_namespace(
    paths: &PathLayout,
    project_id: &str,
    force: bool,
) -> anyhow::Result<()> {
    let mut controller = ccbr_daemon::services::project_namespace_runtime::controller::ProjectNamespaceController::new(
        paths,
        project_id,
        None,
        None,
        None,
        None,
        1,
    )?;
    controller.destroy("kill", force)?;
    let _ = ccbr_daemon::services::start_policy::StartPolicyStore::new(paths).clear();
    Ok(())
}
