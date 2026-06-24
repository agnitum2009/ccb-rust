//! Helpers for rendering a provider home memory bundle.
//!
//! Provider runtimes (Claude, Codex, OpenCode, Gemini) should present a single
//! managed memory file to the provider. This module composes the same runtime
//! memory bundle used by the generic materializer and optionally appends a
//! filtered source-home provider user memory file.

use crate::project_memory::policy::SOURCE_PROVIDER_USER_MEMORY;
use crate::project_memory::renderer::render_memory_bundle;
use crate::project_memory::sources::{load_memory_sources, read_memory_source};
use crate::types::{MemoryError, Result};
use ccbr_storage::path_helpers::normalize_agent_name;
use ccbr_storage::paths::PathLayout;
use std::path::{Path, PathBuf};

/// Render the managed memory bundle for a provider home directory.
///
/// The bundle contains the standard CCBR runtime memory sections (shared project
/// memory, runtime coordination rules, agent private memory) and, if present,
/// the filtered source-home provider user memory file.
pub fn render_provider_home_memory(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
    workspace_path: Option<&Path>,
    provider_user_memory_path: Option<&Path>,
) -> Result<String> {
    let utf8_root =
        camino::Utf8PathBuf::from_path_buf(project_root.to_path_buf()).map_err(|p| {
            MemoryError::InvalidArgument(format!("non-UTF-8 project root: {}", p.display()))
        })?;
    let layout = PathLayout::new(utf8_root);
    let normalized_agent = normalize_agent_name(agent_name).map_err(|e| {
        MemoryError::InvalidArgument(format!("invalid agent name {agent_name}: {e}"))
    })?;

    let mut extra_sources = Vec::new();
    if let Some(user_path) = provider_user_memory_path {
        extra_sources.push(read_memory_source(
            SOURCE_PROVIDER_USER_MEMORY,
            "Provider User Memory",
            user_path,
            false,
        ));
    }

    let sources = load_memory_sources(
        &layout,
        &normalized_agent,
        provider,
        &extra_sources,
        true,
        None,
    );

    Ok(render_memory_bundle(
        project_root,
        &normalized_agent,
        provider,
        &sources,
        workspace_path,
    ))
}

/// Path to the runtime memory bundle for an agent, relative to the project root.
pub fn runtime_memory_bundle_relative_path(
    project_root: &Path,
    agent_name: &str,
) -> Option<PathBuf> {
    let layout =
        PathLayout::new(camino::Utf8PathBuf::from_path_buf(project_root.to_path_buf()).ok()?);
    let normalized = normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase());
    let absolute = layout
        .runtime_memory_bundle_path(&normalized)
        .into_std_path_buf();
    absolute.strip_prefix(project_root).map(PathBuf::from).ok()
}
