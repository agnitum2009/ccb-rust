//! Mirrors Python `lib/ccbd/client_runtime/resolution.py`.
//!
//! Delegates to `ccb_provider_sessions::resolution` while accepting the
//! `ccb_provider_core::runtime_specs::ProviderClientSpec` used by the rest of
//! the daemon.

use std::path::{Path, PathBuf};

/// Convert a provider-core client spec into the provider-sessions equivalent.
fn to_session_spec(
    spec: &ccb_provider_core::runtime_specs::ProviderClientSpec,
) -> ccb_provider_sessions::files::ProviderClientSpec {
    ccb_provider_sessions::files::ProviderClientSpec {
        provider_key: spec.provider_key.clone(),
        enabled_env: spec.enabled_env.clone(),
        autostart_env: spec.autostart_env.clone(),
        state_file_env: spec.state_file_env.clone(),
        session_filename: spec.session_filename.clone(),
    }
}

/// Resolve the working directory from an explicit session file selection.
///
/// Mirrors Python `ccbd.client_runtime.resolution.resolve_work_dir`.
pub fn resolve_work_dir(
    spec: &ccb_provider_core::runtime_specs::ProviderClientSpec,
    cli_session_file: Option<&str>,
    env_session_file: Option<&str>,
    default_cwd: Option<&Path>,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let session_spec = to_session_spec(spec);
    ccb_provider_sessions::resolution::resolve_work_dir(
        &session_spec,
        cli_session_file,
        env_session_file,
        default_cwd,
    )
}

/// Resolve the working directory, falling back to project discovery and the
/// legacy registry-only environment variable guard.
///
/// Mirrors Python `ccbd.client_runtime.resolution.resolve_work_dir_with_registry`.
pub fn resolve_work_dir_with_registry(
    spec: &ccb_provider_core::runtime_specs::ProviderClientSpec,
    provider: &str,
    cli_session_file: Option<&str>,
    env_session_file: Option<&str>,
    default_cwd: Option<&Path>,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    let session_spec = to_session_spec(spec);
    ccb_provider_sessions::resolution::resolve_work_dir_with_registry(
        &session_spec,
        provider,
        cli_session_file,
        env_session_file,
        default_cwd,
        "CCB_REGISTRY_ONLY",
    )
}
