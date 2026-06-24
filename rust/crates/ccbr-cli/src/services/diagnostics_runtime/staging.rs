//! Mirrors Python `lib/cli/services/diagnostics_runtime/staging.py`.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

use crate::context::CliContext;
use crate::services::diagnostics_runtime::models::DiagnosticBundleEntry;
use crate::services::diagnostics_runtime::sources::archive_path_for_source;

const _TAIL_BYTE_LIMIT: usize = 64 * 1024;
const _TAIL_LINE_LIMIT: usize = 200;
const _TAIL_SUFFIXES: &[&str] = &[".log", ".jsonl", ".txt", ".yaml", ".yml"];

/// Stage a single source file into the bundle staging tree.
pub fn stage_file(
    context: &CliContext,
    stage_root: &Path,
    category: &str,
    source: &Path,
) -> DiagnosticBundleEntry {
    let archive_path = archive_path_for_source(context, source);
    let (exists, error) = source_exists(source);
    if let Some(err) = error {
        return DiagnosticBundleEntry {
            category: category.into(),
            source_path: source.to_string_lossy().to_string(),
            archive_path,
            status: "error".into(),
            error: Some(err),
            ..Default::default()
        };
    }
    if !exists {
        return DiagnosticBundleEntry {
            category: category.into(),
            source_path: source.to_string_lossy().to_string(),
            archive_path,
            status: "missing".into(),
            ..Default::default()
        };
    }

    let target = stage_root.join(&archive_path);
    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if is_tail_suffix(source) {
        stage_tailed_text(category, source, &archive_path, &target)
    } else {
        stage_bytes(category, source, &archive_path, &target)
    }
}

/// Tail a text file to the last 64 KiB and/or last 200 lines.
pub fn tail_text_payload(path: &Path) -> anyhow::Result<(String, bool)> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let size = file.metadata()?.len() as usize;
    let mut truncated = size > _TAIL_BYTE_LIMIT;
    let start = if truncated {
        size.saturating_sub(_TAIL_BYTE_LIMIT)
    } else {
        0
    };
    file.seek(SeekFrom::Start(start as u64))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let text = String::from_utf8_lossy(&data).into_owned();
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > _TAIL_LINE_LIMIT {
        lines = lines.split_off(lines.len() - _TAIL_LINE_LIMIT);
        truncated = true;
    }
    let payload = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    Ok((payload, truncated))
}

/// Write a JSON payload to `path` with a trailing newline.
pub fn write_json(path: &Path, payload: &Value) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(payload)?;
    text.push('\n');
    fs::write(path, text)?;
    Ok(())
}

/// Create a gzipped tarball from the staging tree.
pub fn create_tarball(
    stage_root: &Path,
    output_path: &Path,
    bundle_id: &str,
) -> anyhow::Result<()> {
    let file = fs::File::create(output_path)?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut archive = tar::Builder::new(enc);
    archive.append_dir_all(bundle_id, stage_root)?;
    let enc = archive.into_inner()?;
    let _file = enc.finish()?;
    Ok(())
}

fn source_exists(source: &Path) -> (bool, Option<String>) {
    match fs::symlink_metadata(source) {
        Ok(meta) => (meta.is_file(), None),
        Err(e) => (false, Some(e.to_string())),
    }
}

fn is_tail_suffix(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    _TAIL_SUFFIXES
        .iter()
        .any(|s| s.strip_prefix('.').unwrap_or(s).eq_ignore_ascii_case(ext))
}

fn stage_tailed_text(
    category: &str,
    source: &Path,
    archive_path: &str,
    target: &Path,
) -> DiagnosticBundleEntry {
    match tail_text_payload(source) {
        Ok((payload, truncated)) => {
            let byte_count = payload.len();
            if let Err(e) = fs::write(target, payload) {
                return DiagnosticBundleEntry {
                    category: category.into(),
                    source_path: source.to_string_lossy().to_string(),
                    archive_path: archive_path.into(),
                    status: "error".into(),
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
            DiagnosticBundleEntry {
                category: category.into(),
                source_path: source.to_string_lossy().to_string(),
                archive_path: archive_path.into(),
                status: "included".into(),
                truncated,
                byte_count,
                error: None,
            }
        }
        Err(e) => DiagnosticBundleEntry {
            category: category.into(),
            source_path: source.to_string_lossy().to_string(),
            archive_path: archive_path.into(),
            status: "error".into(),
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}

fn stage_bytes(
    category: &str,
    source: &Path,
    archive_path: &str,
    target: &Path,
) -> DiagnosticBundleEntry {
    match fs::read(source) {
        Ok(data) => {
            let byte_count = data.len();
            if let Err(e) = fs::write(target, data) {
                return DiagnosticBundleEntry {
                    category: category.into(),
                    source_path: source.to_string_lossy().to_string(),
                    archive_path: archive_path.into(),
                    status: "error".into(),
                    error: Some(e.to_string()),
                    ..Default::default()
                };
            }
            DiagnosticBundleEntry {
                category: category.into(),
                source_path: source.to_string_lossy().to_string(),
                archive_path: archive_path.into(),
                status: "included".into(),
                truncated: false,
                byte_count,
                error: None,
            }
        }
        Err(e) => DiagnosticBundleEntry {
            category: category.into(),
            source_path: source.to_string_lossy().to_string(),
            archive_path: archive_path.into(),
            status: "error".into(),
            error: Some(e.to_string()),
            ..Default::default()
        },
    }
}
