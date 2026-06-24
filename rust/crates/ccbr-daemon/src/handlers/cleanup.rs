use std::collections::HashSet;
use std::path::Path;

use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::handlers::bool_field;

const STALE_FILE_AGE_S: u64 = 24 * 60 * 60;

/// Run a lightweight cleanup pass over terminal jobs and stale temp files.
pub fn handle_cleanup(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let dry_run = bool_field(payload, "dry_run", true);
    let agent_name = payload
        .get("agent_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let mut errors: Vec<String> = Vec::new();

    // Identify orphaned terminal jobs.
    let terminal_job_ids: Vec<String> = app
        .dispatcher
        .job_store
        .iter()
        .filter(|job| {
            if !job.status.is_terminal() {
                return false;
            }
            if let Some(ref target) = agent_name {
                return job.agent_name == *target;
            }
            true
        })
        .map(|job| job.job_id.clone())
        .collect();

    let mut orphaned_jobs_removed = 0usize;
    if !terminal_job_ids.is_empty() && !dry_run {
        let keep: HashSet<String> = app
            .dispatcher
            .job_store
            .iter()
            .filter(|job| !terminal_job_ids.contains(&job.job_id))
            .map(|job| job.job_id.clone())
            .collect();
        app.dispatcher
            .job_store
            .retain(|job| keep.contains(&job.job_id));
        app.dispatcher.state.rebuild(&app.dispatcher.job_store);
        orphaned_jobs_removed = terminal_job_ids.len();
    }

    // Collect stale temp files under `.ccbr/tmp`.
    let tmp_dir = app.project_root.join(".ccbr").join("tmp");
    let mut stale_files_removed = 0usize;
    if tmp_dir.exists() {
        let now = std::time::SystemTime::now();
        let entries =
            std::fs::read_dir(&tmp_dir).map_err(|e| format!("failed to read tmp dir: {e}"))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let is_stale = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .map(|duration| duration.as_secs() >= STALE_FILE_AGE_S)
                .unwrap_or(false);

            if is_stale {
                if dry_run {
                    stale_files_removed += 1;
                } else if let Err(e) = remove_path(&path) {
                    errors.push(format!("{}: {e}", path.display()));
                } else {
                    stale_files_removed += 1;
                }
            }
        }
    }

    Ok(json!({
        "cleanup_status": "ok",
        "dry_run": dry_run,
        "orphaned_jobs_removed": if dry_run { terminal_job_ids.len() } else { orphaned_jobs_removed },
        "stale_files_removed": stale_files_removed,
        "errors": errors,
    }))
}

fn remove_path(path: &Path) -> Result<(), std::io::Error> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}
