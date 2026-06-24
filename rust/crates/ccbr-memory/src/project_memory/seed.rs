use crate::project_memory::hashing::sha256_text;
use crate::types::{ProjectMemoryEnsureResult, Result};
use ccbr_storage::atomic::{atomic_write_json, atomic_write_text};
use ccbr_storage::paths::PathLayout;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub const TEMPLATE_VERSION: i64 = 5;

pub const DEFAULT_PROJECT_MEMORY: &str = r"# CCBR Project Memory

This project uses CCBR for visible multi-agent collaboration.

## Collaboration

- You are one agent in a CCBR-managed project team.
- Use CCBR `ask` for project-level collaboration with configured agents.
- Delegate with the goal, scope/files, assumptions, expected output, and verification needs.
- Reply concisely with findings, changes, verification, blockers, and risks when relevant.
";

const SEED_SCHEMA_VERSION: i64 = 1;
const SEED_RECORD_TYPE: &str = "ccbr_project_memory_seed";

/// Path to the project memory markdown file.
pub fn project_memory_path(project_root_or_layout: &PathLayout) -> PathBuf {
    project_root_or_layout
        .project_memory_path()
        .into_std_path_buf()
}

/// Path to the memory seed metadata file.
pub fn seed_metadata_path(project_root_or_layout: &PathLayout) -> PathBuf {
    project_root_or_layout
        .memory_seed_path()
        .into_std_path_buf()
}

/// Ensure the project memory file exists and is up-to-date.
pub fn ensure_project_memory(
    layout: &PathLayout,
    now: Option<&str>,
) -> Result<ProjectMemoryEnsureResult> {
    let path = layout.project_memory_path();
    let seed_path = layout.memory_seed_path();
    let template = DEFAULT_PROJECT_MEMORY;
    let template_hash = sha256_text(template);

    let mut created = false;
    let mut warning = String::new();

    match atomic_create_text(&path, template) {
        Ok(true) => created = true,
        Ok(false) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => {
            return Ok(ProjectMemoryEnsureResult {
                path: path.into_std_path_buf(),
                seed_path: seed_path.into_std_path_buf(),
                created: false,
                seed_written: false,
                sha256: String::new(),
                warning: format!("failed_to_create_project_memory: {e}"),
            })
        }
    }

    if created {
        let (seed_written, seed_warning) =
            write_seed_metadata(&seed_path, &path, &template_hash, now);
        return Ok(ProjectMemoryEnsureResult {
            path: path.into_std_path_buf(),
            seed_path: seed_path.into_std_path_buf(),
            created: true,
            seed_written,
            sha256: template_hash,
            warning: seed_warning,
        });
    }

    let current_hash = file_sha256(&path);
    let seed_record = read_seed_metadata(layout);
    if should_upgrade_project_memory(&seed_record, &current_hash, &template_hash) {
        if let Err(e) = atomic_write_text(&path, template) {
            return Ok(ProjectMemoryEnsureResult {
                path: path.into_std_path_buf(),
                seed_path: seed_path.into_std_path_buf(),
                created: false,
                seed_written: false,
                sha256: current_hash,
                warning: format!("failed_to_upgrade_project_memory_seed: {e}"),
            });
        }
        let (seed_written, seed_warning) =
            write_seed_metadata(&seed_path, &path, &template_hash, now);
        return Ok(ProjectMemoryEnsureResult {
            path: path.into_std_path_buf(),
            seed_path: seed_path.into_std_path_buf(),
            created: false,
            seed_written,
            sha256: template_hash,
            warning: seed_warning,
        });
    }

    let mut seed_written = false;
    if current_hash == template_hash && !seed_path.is_file() {
        let (written, seed_warning) = write_seed_metadata(&seed_path, &path, &template_hash, now);
        seed_written = written;
        warning = seed_warning;
    }

    Ok(ProjectMemoryEnsureResult {
        path: path.into_std_path_buf(),
        seed_path: seed_path.into_std_path_buf(),
        created: false,
        seed_written,
        sha256: current_hash,
        warning,
    })
}

/// Read seed metadata if it exists and is valid.
pub fn read_seed_metadata(layout: &PathLayout) -> serde_json::Map<String, serde_json::Value> {
    let path = layout.memory_seed_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return serde_json::Map::new(),
    };
    let value: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return serde_json::Map::new(),
    };
    value.as_object().cloned().unwrap_or_default()
}

fn atomic_create_text(path: &camino::Utf8Path, text: &str) -> std::io::Result<bool> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?;
    std::fs::create_dir_all(parent)?;

    let target = std::path::Path::new(path.as_str());
    let file = OpenOptions::new().write(true).create_new(true).open(target);

    match file {
        Ok(mut f) => {
            f.write_all(text.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

fn write_seed_metadata(
    seed_path: &camino::Utf8Path,
    memory_path: &camino::Utf8Path,
    memory_hash: &str,
    now: Option<&str>,
) -> (bool, String) {
    if let Some(parent) = seed_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return (false, format!("failed_to_create_seed_dir: {e}"));
        }
    }
    let timestamp = now
        .map(|s| s.to_string())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let payload = serde_json::json!({
        "schema_version": SEED_SCHEMA_VERSION,
        "record_type": SEED_RECORD_TYPE,
        "template_version": TEMPLATE_VERSION,
        "memory_path": memory_path.as_str(),
        "sha256": memory_hash,
        "created_at": timestamp,
    });
    match atomic_write_json(seed_path, &payload) {
        Ok(()) => (true, String::new()),
        Err(e) => (false, format!("failed_to_write_project_memory_seed: {e}")),
    }
}

fn file_sha256(path: &camino::Utf8Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => sha256_text(&text),
        Err(_) => String::new(),
    }
}

fn should_upgrade_project_memory(
    seed_record: &serde_json::Map<String, serde_json::Value>,
    current_hash: &str,
    template_hash: &str,
) -> bool {
    if current_hash.is_empty() || current_hash == template_hash {
        return false;
    }
    if legacy_generated_template_version(current_hash).is_some() {
        return true;
    }
    if seed_record.is_empty()
        || seed_record.get("record_type").and_then(|v| v.as_str()) != Some(SEED_RECORD_TYPE)
    {
        return false;
    }
    if seed_record
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        != current_hash
    {
        return false;
    }
    let seed_version = seed_record
        .get("template_version")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    seed_version < TEMPLATE_VERSION
}

const LEGACY_V4_TEMPLATE_HASH: &str =
    "60b058f792c7e927ff816109c9b9717f176bf7f694306b325118e5fd843486cb";

fn legacy_generated_template_version(current_hash: &str) -> Option<i64> {
    if current_hash == LEGACY_V4_TEMPLATE_HASH {
        return Some(4);
    }
    None
}
