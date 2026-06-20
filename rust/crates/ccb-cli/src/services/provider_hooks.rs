//! Mirrors Python `lib/cli/services/provider_hooks.py`.
//!
//! CLI-side orchestration for installing provider finish/activity hooks into a
//! managed provider home.

use camino::{Utf8Path, Utf8PathBuf};

use ccb_provider_hooks::settings::{
    build_activity_hook_command, build_hook_command, install_workspace_activity_hooks,
    install_workspace_completion_hooks_with_profile,
};

/// Prepare the completion (and optionally activity) hooks for a provider
/// workspace.
///
/// Mirrors Python `prepare_workspace_provider_hooks`. Only `claude` and
/// `gemini` receive completion hooks; only `claude` receives activity hooks
/// when both `project_id` and `runtime_dir` are supplied.
#[allow(clippy::too_many_arguments)]
pub fn prepare_workspace_provider_hooks(
    provider: &str,
    workspace_path: &Utf8Path,
    completion_dir: &Utf8Path,
    agent_name: &str,
    home_root: Option<&Utf8Path>,
    project_id: Option<&str>,
    runtime_dir: Option<&Utf8Path>,
    resolved_profile: Option<&serde_json::Value>,
) -> Option<Utf8PathBuf> {
    let normalized = provider.trim().to_lowercase();
    if !matches!(normalized.as_str(), "claude" | "gemini") {
        return None;
    }

    let finish_hook = provider_hook_binary_path("ccb-provider-finish-hook")?;
    let command = build_hook_command(
        &normalized,
        &finish_hook,
        "",
        completion_dir,
        agent_name,
        workspace_path,
    );
    let settings_path = install_workspace_completion_hooks_with_profile(
        &normalized,
        workspace_path,
        home_root,
        &command,
        resolved_profile,
    );

    if normalized == "claude" {
        if let (Some(project_id), Some(runtime_dir)) = (project_id, runtime_dir) {
            let activity_hook = provider_hook_binary_path("ccb-provider-activity-hook")?;
            let activity_command = build_activity_hook_command(
                &normalized,
                &activity_hook,
                "",
                project_id,
                agent_name,
                runtime_dir,
                workspace_path,
            );
            return install_workspace_activity_hooks(
                &normalized,
                workspace_path,
                home_root,
                &activity_command,
            )
            .or(settings_path);
        }
    }

    settings_path
}

/// Locate a native provider hook binary.
///
/// Search order:
/// 1. `CCB_HOOK_BIN_DIR` environment variable.
/// 2. Directory containing the currently running executable.
/// 3. `PATH` environment variable.
fn provider_hook_binary_path(name: &str) -> Option<Utf8PathBuf> {
    if let Ok(dir) = std::env::var("CCB_HOOK_BIN_DIR") {
        let candidate = Utf8PathBuf::from(dir).join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Utf8PathBuf::from_path_buf(candidate).ok();
            }
        }
    }

    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Utf8PathBuf::from_path_buf(candidate).ok();
            }
        }
    }

    None
}
