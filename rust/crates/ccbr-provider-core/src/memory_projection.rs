use std::path::{Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use ccbr_storage::atomic::atomic_write_text;
use ccbr_storage::paths::PathLayout;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{ProviderCoreError, Result};

const DEFAULT_PROJECT_MEMORY: &str = r"# CCB Project Memory

This project uses CCB for visible multi-agent collaboration.

## Collaboration

- You are one agent in a CCB-managed project team.
- Use CCB `ask` for project-level collaboration with configured agents.
- Delegate with the goal, scope/files, assumptions, expected output, and verification needs.
- Reply concisely with findings, changes, verification, blockers, and risks when relevant.
";

#[derive(Debug, Clone)]
struct MemorySource {
    title: String,
    path: PathBuf,
    content: String,
    exists: bool,
    warning: String,
}

/// Result of a memory projection operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProjectionResult {
    pub status: String,
    pub reason: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub source_count: i64,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub error_detail: String,
}

/// Build a normalized memory projection result.
#[allow(clippy::too_many_arguments)]
pub fn memory_projection_result(
    status: &str,
    reason: &str,
    path: &Path,
    sha256: Option<&str>,
    source_count: Option<i64>,
    warnings: Option<&[String]>,
    error_detail: Option<&str>,
) -> MemoryProjectionResult {
    MemoryProjectionResult {
        status: status.to_string(),
        reason: reason.to_string(),
        path: path.to_string_lossy().to_string(),
        sha256: sha256.unwrap_or("").to_string(),
        source_count: source_count.unwrap_or(0),
        warnings: warnings
            .unwrap_or(&[])
            .iter()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect(),
        error_detail: error_detail.unwrap_or("").to_string(),
    }
}

/// Record a memory-projection agent event, deduplicating against the marker.
pub fn record_memory_projection_event(
    result: &MemoryProjectionResult,
    provider: &str,
    event_path: Option<&Path>,
    marker_path: Option<&Path>,
    agent_name: Option<&str>,
) -> Result<()> {
    let event_path = match event_path {
        Some(p) => p,
        None => return Ok(()),
    };
    let marker_path = match marker_path {
        Some(p) => p,
        None => return Ok(()),
    };
    let provider_name = provider.trim();
    if provider_name.is_empty() {
        return Ok(());
    }
    let agent_name = match agent_name {
        Some(a) if !a.trim().is_empty() => a,
        _ => return Ok(()),
    };

    let status = result.status.clone();
    let reason = result.reason.clone();
    let signature = json!({
        "status": status.clone(),
        "reason": reason.clone(),
        "path": result.path.clone(),
        "sha256": result.sha256.clone(),
        "warnings": result.warnings.clone(),
    });

    if same_memory_projection_signature(marker_path, &signature) {
        return Ok(());
    }

    let event = json!({
        "record_type": "agent_event",
        "event_type": format!("{}_memory_projection_{}", provider_name, status),
        "provider": provider_name,
        "agent_name": agent_name,
        "status": status,
        "reason": reason,
        "projection_path": result.path,
        "sha256": result.sha256,
        "source_count": result.source_count,
        "warnings": result.warnings,
        "error_detail": result.error_detail,
        "created_at": now_utc_rfc3339(),
    });

    write_projection_event_and_marker(&event, &signature, event_path, marker_path)
}

/// Append an event to a JSONL file and write its signature marker.
pub fn write_projection_event_and_marker(
    event: &serde_json::Value,
    signature: &serde_json::Value,
    event_path: &Path,
    marker_path: &Path,
) -> Result<()> {
    let event_utf8 = Utf8Path::from_path(event_path).ok_or_else(|| {
        ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "event path is not valid utf-8",
        ))
    })?;
    let marker_utf8 = Utf8Path::from_path(marker_path).ok_or_else(|| {
        ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "marker path is not valid utf-8",
        ))
    })?;

    let store = ccbr_storage::jsonl::JsonlStore::new();
    store.append(event_utf8, event)?;
    ccbr_storage::atomic::atomic_write_json(marker_utf8, signature)?;
    Ok(())
}

/// Check whether `payload` matches the signature stored at `marker_path`.
pub fn same_memory_projection_signature(marker_path: &Path, payload: &serde_json::Value) -> bool {
    let existing = match std::fs::read_to_string(marker_path) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let existing: serde_json::Value = match serde_json::from_str(&existing) {
        Ok(serde_json::Value::Object(m)) => serde_json::Value::Object(m),
        _ => return false,
    };

    if existing == *payload {
        return true;
    }

    let payload_obj = match payload.as_object() {
        Some(o) => o,
        None => return false,
    };

    let status = payload_obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let reason = payload_obj
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if status == "skipped" && reason == "unchanged" {
        let sha256 = payload_obj
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        return !sha256.is_empty()
            && field_eq(&existing, payload, "path")
            && field_eq(&existing, payload, "sha256")
            && field_eq(&existing, payload, "warnings");
    }

    if status == "skipped" {
        return field_eq(&existing, payload, "reason")
            && field_eq(&existing, payload, "path")
            && field_eq(&existing, payload, "sha256")
            && field_eq(&existing, payload, "warnings");
    }

    false
}

fn field_eq(left: &serde_json::Value, right: &serde_json::Value, key: &str) -> bool {
    left.get(key) == right.get(key)
}

/// Compute the SHA-256 hex digest of an existing text file.
pub fn text_file_sha256(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => sha256_text(&text),
        Err(_) => String::new(),
    }
}

fn to_utf8_path(path: &Path) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).map_err(|path| {
        ProviderCoreError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path is not valid utf-8: {}", path.display()),
        ))
    })
}

fn utf8_path_buf(path: &Path) -> Utf8PathBuf {
    to_utf8_path(path).unwrap_or_else(|_| Utf8PathBuf::from(path.to_string_lossy().as_ref()))
}

fn ensure_project_memory(root: &PathLayout) -> String {
    let path = root.project_memory_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, DEFAULT_PROJECT_MEMORY);
        return String::new();
    }
    String::new()
}

fn read_source(title: &str, path: &Path, _include_missing: bool) -> MemorySource {
    if !path.is_file() {
        return MemorySource {
            title: title.to_string(),
            path: path.to_path_buf(),
            content: String::new(),
            exists: false,
            warning: String::new(),
        };
    }
    match std::fs::read_to_string(path) {
        Ok(content) => MemorySource {
            title: title.to_string(),
            path: path.to_path_buf(),
            content,
            exists: true,
            warning: String::new(),
        },
        Err(e) => MemorySource {
            title: title.to_string(),
            path: path.to_path_buf(),
            content: String::new(),
            exists: true,
            warning: format!("failed_to_read_memory_source: {e}"),
        },
    }
}

fn render_source_section(source: &MemorySource) -> Vec<String> {
    let content = source.content.trim_end();
    let mut lines = vec![
        format!("## {}", source.title),
        format!("source: {}", source.path.display()),
    ];
    if !source.warning.is_empty() {
        lines.push(format!("warning: {}", source.warning));
    }
    lines.push(String::new());
    if !content.is_empty() {
        lines.push(content.to_string());
        lines.push(String::new());
    }
    lines
}

fn render_memory_bundle(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
    sources: &[MemorySource],
    workspace_path: Option<&Path>,
) -> String {
    let mut lines: Vec<String> = vec![
        "# CCB Managed Agent Memory".to_string(),
        String::new(),
        "<!-- ccbr-memory-bundle schema_version=1".to_string(),
        "generated_by: ccb".to_string(),
        "do_not_edit: true".to_string(),
        format!("agent: {agent_name}"),
        format!("provider: {provider}"),
        format!("project_root: {}", resolve_path(project_root).display()),
    ];
    if let Some(ws) = workspace_path {
        lines.push(format!("workspace_path: {}", resolve_path(ws).display()));
    }
    lines.extend([
        "-->".to_string(),
        String::new(),
        "## CCB Runtime Coordination Rules".to_string(),
        String::new(),
        "- CCB `ask` is submit-only: submit once, then stop. Do not wait, poll, or run `pend`/`watch`/`ping` unless diagnostics were requested.".to_string(),
        "- Prefer `/ask <agent> <message>` when available.".to_string(),
        String::new(),
    ]);
    for source in sources {
        if !source.exists && source.warning.is_empty() {
            continue;
        }
        if source.content.trim().is_empty() && source.warning.is_empty() {
            continue;
        }
        lines.extend(render_source_section(source));
    }
    format!("{}\n", lines.join("\n").trim_end())
}

fn resolve_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Materialize the provider memory file for an agent.
///
/// This is a self-contained Rust implementation that mirrors Python's
/// `materialize_provider_memory_file`. It does not depend on `ccbr-memory`
/// (which depends on this crate), so it replicates the essential source
/// loading and rendering behavior locally.
pub fn materialize_provider_memory_file(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
    target: &Path,
    provider_memory_path: &Path,
    provider_memory_title: &str,
    workspace_path: Option<&Path>,
) -> MemoryProjectionResult {
    let project_root_utf8 = utf8_path_buf(project_root);
    let root = PathLayout::new(project_root_utf8);
    let mut warnings: Vec<String> = Vec::new();

    let warning = ensure_project_memory(&root);
    if !warning.is_empty() {
        warnings.push(warning);
    }

    let sources: Vec<MemorySource> = vec![
        read_source(
            "CCB Shared Project Memory",
            root.project_memory_path().as_ref(),
            true,
        ),
        read_source(provider_memory_title, provider_memory_path, false),
        read_source(
            "Agent Private Memory",
            root.agent_private_memory_path(agent_name).as_ref(),
            true,
        ),
    ];

    warnings.extend(
        sources
            .iter()
            .filter(|s| !s.warning.is_empty())
            .map(|s| s.warning.clone()),
    );

    let rendered =
        render_memory_bundle(project_root, agent_name, provider, &sources, workspace_path);
    let digest = sha256_text(&rendered);

    if text_file_sha256(target) == digest {
        return memory_projection_result(
            "skipped",
            "unchanged",
            target,
            Some(&digest),
            Some(sources.len() as i64),
            Some(&warnings),
            None,
        );
    }

    let target_utf8 = match to_utf8_path(target) {
        Ok(p) => p,
        Err(e) => {
            return memory_projection_result(
                "failed",
                "invalid_target_path",
                target,
                None,
                Some(sources.len() as i64),
                Some(&warnings),
                Some(&e.to_string()),
            )
        }
    };

    if let Err(e) = atomic_write_text(&target_utf8, &rendered) {
        return memory_projection_result(
            "failed",
            "write_error",
            target,
            None,
            Some(sources.len() as i64),
            Some(&warnings),
            Some(&e.to_string()),
        );
    }

    memory_projection_result(
        "ok",
        "written",
        target,
        Some(&digest),
        Some(sources.len() as i64),
        Some(&warnings),
        None,
    )
}

fn sha256_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_utc_rfc3339() -> String {
    chrono::Utc::now()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("+00:00", "Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_projection_result_normalizes() {
        let tmp = std::env::temp_dir();
        let result = memory_projection_result(
            "failed",
            "missing_project_context",
            &tmp.join("AGENTS.md"),
            None,
            None,
            Some(&["warn".to_string(), "".to_string(), "also-warn".to_string()]),
            None,
        );
        assert_eq!(result.status, "failed");
        assert_eq!(result.warnings, vec!["warn", "also-warn"]);
        assert_eq!(result.error_detail, "");
    }

    #[test]
    fn test_materialize_provider_memory_file_writes_and_skips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir(&project_root).unwrap();
        let provider_memory_path = project_root.join("AGENTS.md");
        std::fs::write(&provider_memory_path, "# Agent instructions\n").unwrap();
        let target = project_root.join("runtime-memory.md");

        let result = materialize_provider_memory_file(
            &project_root,
            "claude",
            "claude",
            &target,
            &provider_memory_path,
            "Provider-Native Project Memory",
            None,
        );
        assert_eq!(result.status, "ok");
        assert_eq!(result.reason, "written");
        assert!(target.exists());
        assert!(result.source_count >= 2);
        assert!(!result.sha256.is_empty());

        let second = materialize_provider_memory_file(
            &project_root,
            "claude",
            "claude",
            &target,
            &provider_memory_path,
            "Provider-Native Project Memory",
            None,
        );
        assert_eq!(second.status, "skipped");
        assert_eq!(second.reason, "unchanged");
        assert_eq!(second.sha256, result.sha256);
    }

    #[test]
    fn test_materialize_provider_memory_file_includes_provider_memory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir(&project_root).unwrap();
        let provider_memory_path = project_root.join("GEMINI.md");
        std::fs::write(&provider_memory_path, "## Gemini notes\n").unwrap();
        let target = project_root.join("bundle.md");

        let result = materialize_provider_memory_file(
            &project_root,
            "gemini",
            "gemini",
            &target,
            &provider_memory_path,
            "Provider User Memory",
            Some(&project_root),
        );
        assert_eq!(result.status, "ok");
        let content = std::fs::read_to_string(&target).unwrap();
        assert!(content.contains("## Gemini notes"));
        assert!(content.contains("agent: gemini"));
        assert!(content.contains("workspace_path"));
    }
}
