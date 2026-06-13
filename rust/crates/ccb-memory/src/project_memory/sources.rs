use crate::project_memory::filters::filter_memory_source;
use crate::project_memory::policy::{
    should_include_source, SOURCE_AGENT_PRIVATE, SOURCE_CCB_SHARED, SOURCE_PROVIDER_NATIVE_PROJECT,
};
use crate::types::ProjectMemorySource;
use ccb_storage::path_helpers::normalize_agent_name;
use ccb_storage::paths::PathLayout;
use std::path::{Path, PathBuf};

const PROVIDER_NATIVE_FILES: &[(&str, &str)] = &[
    ("claude", "CLAUDE.md"),
    ("codex", "AGENTS.md"),
    ("opencode", "AGENTS.md"),
    ("gemini", "GEMINI.md"),
];

/// Path to an agent's private memory file.
pub fn agent_private_memory_path(layout: &PathLayout, agent_name: &str) -> PathBuf {
    layout
        .agent_private_memory_path(
            &normalize_agent_name(agent_name).unwrap_or_else(|_| agent_name.to_lowercase()),
        )
        .into_std_path_buf()
}

/// Path to a provider's native project memory file, if supported.
pub fn provider_native_memory_path(layout: &PathLayout, provider: &str) -> Option<PathBuf> {
    let filename = PROVIDER_NATIVE_FILES
        .iter()
        .find(|(p, _)| *p == provider.trim().to_lowercase())
        .map(|(_, f)| *f)?;
    Some(layout.project_root.as_std_path().join(filename))
}

/// Load all memory sources for an agent and provider.
pub fn load_memory_sources(
    layout: &PathLayout,
    agent_name: &str,
    provider: &str,
    extra_sources: &[ProjectMemorySource],
    include_missing: bool,
    include_provider_native_project: Option<bool>,
) -> Vec<ProjectMemorySource> {
    let mut sources: Vec<ProjectMemorySource> = extra_sources
        .iter()
        .map(|s| filter_memory_source(s, &filters_for_source(provider, &s.kind)))
        .collect();

    sources.push(read_source(
        SOURCE_CCB_SHARED,
        "CCB Shared Project Memory",
        &crate::project_memory::seed::project_memory_path(layout),
        true,
    ));

    let include_native = include_provider_native_project
        .unwrap_or_else(|| should_include_source(provider, SOURCE_PROVIDER_NATIVE_PROJECT));
    if include_native {
        if let Some(provider_path) = provider_native_memory_path(layout, provider) {
            let provider_source = read_source(
                SOURCE_PROVIDER_NATIVE_PROJECT,
                "Provider-Native Project Memory",
                &provider_path,
                include_missing,
            );
            sources.push(provider_source);
        }
    }

    sources.extend(role_memory_sources(
        layout.project_root.as_std_path(),
        agent_name,
    ));

    sources.push(read_source(
        SOURCE_AGENT_PRIVATE,
        "Agent Private Memory",
        &agent_private_memory_path(layout, agent_name),
        true,
    ));

    sources
}

fn filters_for_source(provider: &str, kind: &str) -> Vec<String> {
    crate::project_memory::policy::filters_for_source(provider, kind)
}

fn role_memory_sources(_project_root: &Path, _agent_name: &str) -> Vec<ProjectMemorySource> {
    // Rolepack runtime lookup is not ported to Rust in this migration.
    Vec::new()
}

/// Read a single memory source.
pub fn read_memory_source(
    kind: &str,
    title: &str,
    path: &Path,
    include_missing: bool,
) -> ProjectMemorySource {
    read_source(kind, title, path, include_missing)
}

fn read_source(kind: &str, title: &str, path: &Path, include_missing: bool) -> ProjectMemorySource {
    if !path.is_file() {
        if include_missing {
            return ProjectMemorySource::new(kind, title, path, "", false);
        }
        return ProjectMemorySource::new(kind, title, path, "", false);
    }

    match std::fs::read_to_string(path) {
        Ok(content) => ProjectMemorySource::new(kind, title, path, content, true),
        Err(e) => ProjectMemorySource::new(kind, title, path, "", true)
            .with_warning(format!("failed_to_read_memory_source: {e}")),
    }
}
