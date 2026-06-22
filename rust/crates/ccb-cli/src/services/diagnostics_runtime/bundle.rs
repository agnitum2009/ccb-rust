//! Mirrors Python `lib/cli/services/diagnostics_runtime/bundle.py`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

use ccb_storage::paths::PathLayout;

use crate::context::CliContext;
use crate::services::diagnostics_runtime::models::{
    DiagnosticBundleEntry, DiagnosticBundleSummary,
};
use crate::services::diagnostics_runtime::sources::project_root_sources;
use crate::services::diagnostics_runtime::staging::{create_tarball, stage_file, write_json};

/// Export a diagnostic bundle for the current project.
pub fn export_diagnostic_bundle(
    context: &CliContext,
    command: &Value,
) -> anyhow::Result<DiagnosticBundleSummary> {
    export_diagnostic_bundle_with_storage(context, command, |paths| {
        ccb_storage_classification::classification::summarize_storage(paths)
            .map_err(|e| e.to_string())
    })
}

/// Export a diagnostic bundle with an injectable storage-summary function.
///
/// The public `export_diagnostic_bundle` calls this with the real storage
/// classifier; tests can inject a failing implementation.
pub fn export_diagnostic_bundle_with_storage<F>(
    context: &CliContext,
    command: &Value,
    storage_fn: F,
) -> anyhow::Result<DiagnosticBundleSummary>
where
    F: FnOnce(&PathLayout) -> Result<serde_json::Map<String, Value>, String>,
{
    let generated_at = utc_now();
    let bundle_id = bundle_identifier(&context.project.project_id, &generated_at);
    let output_path = resolve_output_path(context, command, &bundle_id)?;
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (doctor_payload, doctor_error) = doctor_payload(context);
    let (storage_payload, storage_error) = storage_payload(context, storage_fn);
    let mut entries: Vec<DiagnosticBundleEntry> = Vec::new();

    let support_dir = context.paths.ccbd_support_dir();
    std::fs::create_dir_all(support_dir.as_std_path())?;
    let tmpdir = tempfile::tempdir_in(support_dir.as_std_path())
        .with_context(|| "create bundle staging directory")?;
    let stage_root = tmpdir.path().join(&bundle_id);
    std::fs::create_dir_all(&stage_root)?;

    write_generated_payloads(
        &stage_root,
        context,
        &bundle_id,
        &generated_at,
        &doctor_payload,
        doctor_error.as_deref(),
        &storage_payload,
        storage_error.as_deref(),
    )?;

    for (category, source) in project_root_sources(context, Some(&storage_payload)) {
        entries.push(stage_file(context, &stage_root, &category, &source));
    }

    let manifest = bundle_manifest(
        context,
        &generated_at,
        &bundle_id,
        doctor_error.as_deref(),
        storage_error.as_deref(),
        &entries,
    );
    write_json(&stage_root.join("manifest.json"), &manifest)?;
    create_tarball(&stage_root, &output_path, &bundle_id)?;

    Ok(bundle_summary(
        context,
        &output_path,
        &bundle_id,
        doctor_error.as_deref(),
        &entries,
    ))
}

fn doctor_payload(context: &CliContext) -> (Value, Option<String>) {
    let payload = serde_json::json!({
        "project": context.project.project_root.to_string_lossy().to_string(),
        "project_id": context.project.project_id,
    });
    (payload, None)
}

fn storage_payload<F>(context: &CliContext, storage_fn: F) -> (Value, Option<String>)
where
    F: FnOnce(&PathLayout) -> Result<serde_json::Map<String, Value>, String>,
{
    match storage_fn(&context.paths) {
        Ok(map) => (Value::Object(map), None),
        Err(err) => {
            let payload = serde_json::json!({
                "schema_version": 1,
                "project": context.project.project_root.to_string_lossy().to_string(),
                "project_id": context.project.project_id,
                "error": err,
                "entries": [],
            });
            (payload, Some(err))
        }
    }
}

/// Build a deterministic bundle identifier from the project id and timestamp.
pub fn bundle_identifier(project_id: &str, generated_at: &str) -> String {
    let safe_time = generated_at
        .replace([':', '-', '.'], "")
        .replace('T', "t")
        .replace('Z', "z");
    format!(
        "ccb-support-{}-{}",
        safe_time,
        &project_id[..project_id.len().min(12)]
    )
}

/// Resolve the bundle output path from the command or the default support location.
pub fn resolve_output_path(
    context: &CliContext,
    command: &Value,
    bundle_id: &str,
) -> anyhow::Result<PathBuf> {
    if let Some(raw) = command.get("output_path").and_then(Value::as_str) {
        let expanded = expand_user_path(raw);
        let candidate = PathBuf::from(expanded);
        let resolved = if candidate.is_absolute() {
            candidate
        } else {
            std::path::absolute(context.cwd.join(candidate))?
        };
        Ok(resolved)
    } else {
        Ok(context
            .paths
            .support_bundle_path(bundle_id)
            .map_err(|e| anyhow::anyhow!("invalid bundle id: {}", e))?
            .into_std_path_buf())
    }
}

#[allow(clippy::too_many_arguments)]
fn write_generated_payloads(
    stage_root: &Path,
    context: &CliContext,
    bundle_id: &str,
    generated_at: &str,
    doctor_payload: &Value,
    doctor_error: Option<&str>,
    storage_payload: &Value,
    storage_error: Option<&str>,
) -> anyhow::Result<()> {
    write_json(
        &stage_root.join("generated").join("doctor.json"),
        doctor_payload,
    )?;
    write_json(
        &stage_root.join("generated").join("storage-summary.json"),
        storage_payload,
    )?;
    write_json(
        &stage_root.join("generated").join("bundle-metadata.json"),
        &serde_json::json!({
            "generated_at": generated_at,
            "project_root": context.project.project_root.to_string_lossy().to_string(),
            "project_id": context.project.project_id,
            "bundle_id": bundle_id,
            "doctor_error": doctor_error,
            "storage_error": storage_error,
        }),
    )?;
    Ok(())
}

fn bundle_manifest(
    context: &CliContext,
    generated_at: &str,
    bundle_id: &str,
    doctor_error: Option<&str>,
    storage_error: Option<&str>,
    entries: &[DiagnosticBundleEntry],
) -> Value {
    let entries_json: Vec<Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "category": entry.category,
                "source_path": entry.source_path,
                "archive_path": entry.archive_path,
                "status": entry.status,
                "truncated": entry.truncated,
                "byte_count": entry.byte_count,
                "error": entry.error,
            })
        })
        .collect();
    serde_json::json!({
        "schema_version": 1,
        "record_type": "ccbd_diagnostic_bundle",
        "generated_at": generated_at,
        "project_root": context.project.project_root.to_string_lossy().to_string(),
        "project_id": context.project.project_id,
        "bundle_id": bundle_id,
        "doctor_error": doctor_error,
        "storage_error": storage_error,
        "entries": entries_json,
    })
}

fn bundle_summary(
    context: &CliContext,
    output_path: &Path,
    bundle_id: &str,
    doctor_error: Option<&str>,
    entries: &[DiagnosticBundleEntry],
) -> DiagnosticBundleSummary {
    let included_count = entries.iter().filter(|e| e.status == "included").count();
    let missing_count = entries.iter().filter(|e| e.status == "missing").count();
    let truncated_count = entries.iter().filter(|e| e.truncated).count();
    DiagnosticBundleSummary {
        project_root: context.project.project_root.to_string_lossy().to_string(),
        project_id: context.project.project_id.clone(),
        bundle_id: bundle_id.into(),
        bundle_path: output_path.to_string_lossy().to_string(),
        file_count: entries.len(),
        included_count,
        missing_count,
        truncated_count,
        doctor_error: doctor_error.map(|s| s.into()),
    }
}

fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return home + rest;
        }
    }
    raw.to_string()
}
