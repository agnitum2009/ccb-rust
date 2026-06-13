use crate::project_memory::hashing::sha256_text;
use crate::project_memory::renderer::render_memory_bundle;
use crate::project_memory::seed::ensure_project_memory;
use crate::project_memory::sources::load_memory_sources;
use crate::types::{
    MemoryError, ProjectMemoryMaterialization, ProjectMemorySource, ProjectMemorySourceRef,
};
use ccb_storage::atomic::atomic_write_text;
use ccb_storage::path_helpers::normalize_agent_name;
use ccb_storage::paths::PathLayout;
use std::path::{Path, PathBuf};

/// Path to the runtime memory bundle for an agent.
pub fn runtime_memory_bundle_path(layout: &PathLayout, agent_name: &str) -> PathBuf {
    layout
        .runtime_memory_bundle_path(
            &normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase()),
        )
        .into_std_path_buf()
}

/// Materialize the runtime memory bundle for an agent.
pub fn materialize_runtime_memory_bundle(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
    workspace_path: Option<&Path>,
    now: Option<&str>,
) -> Result<ProjectMemoryMaterialization, MemoryError> {
    let mut warnings: Vec<String> = Vec::new();

    let layout = PathLayout::new(
        camino::Utf8PathBuf::from_path_buf(project_root.to_path_buf()).map_err(|p| {
            MemoryError::InvalidArgument(format!("non-UTF-8 project root: {}", p.display()))
        })?,
    );
    let normalized_agent =
        normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase());

    let ensure_result = ensure_project_memory(&layout, now)?;
    if !ensure_result.warning.is_empty() {
        warnings.push(ensure_result.warning);
    }

    let sources = load_memory_sources(&layout, &normalized_agent, provider, &[], true, None);
    warnings.extend(
        sources
            .iter()
            .filter(|s| !s.warning.is_empty())
            .map(|s| s.warning.clone()),
    );

    let rendered = render_memory_bundle(
        layout.project_root.as_std_path(),
        &normalized_agent,
        provider,
        &sources,
        workspace_path,
    );
    let target = runtime_memory_bundle_path(&layout, &normalized_agent);
    let digest = sha256_text(&rendered);
    let current_digest = path_sha256(&target);

    if current_digest == digest {
        return Ok(ProjectMemoryMaterialization {
            path: target,
            written: false,
            unchanged: true,
            sha256: digest,
            sources: source_refs(&sources),
            warnings,
        });
    }

    let utf8_target = camino::Utf8Path::from_path(&target).ok_or_else(|| {
        MemoryError::InvalidArgument(format!(
            "non-UTF-8 runtime memory path: {}",
            target.display()
        ))
    })?;

    if let Err(e) = atomic_write_text(utf8_target, &rendered) {
        warnings.push(format!("failed_to_write_runtime_memory_bundle: {e}"));
        return Ok(ProjectMemoryMaterialization {
            path: target,
            written: false,
            unchanged: false,
            sha256: String::new(),
            sources: source_refs(&sources),
            warnings,
        });
    }

    Ok(ProjectMemoryMaterialization {
        path: target,
        written: true,
        unchanged: false,
        sha256: digest,
        sources: source_refs(&sources),
        warnings,
    })
}

fn source_refs(sources: &[ProjectMemorySource]) -> Vec<ProjectMemorySourceRef> {
    sources
        .iter()
        .map(|s| ProjectMemorySourceRef {
            kind: s.kind.clone(),
            path: s.path.clone(),
            exists: s.exists,
            sha256: if s.exists {
                sha256_text(&s.content)
            } else {
                String::new()
            },
            warning: s.warning.clone(),
            filtered: s.filtered,
            filter_names: s.filter_names.clone(),
        })
        .collect()
}

fn path_sha256(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => sha256_text(&text),
        Err(_) => String::new(),
    }
}
