use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use serde_json::{json, Value};

use crate::app::CcbdApp;
use crate::handlers::str_field;
use crate::provider_launcher::default_session_path;

const DEFAULT_TAIL: usize = 50;
const MAX_TAIL: usize = 500;

/// Return recent log lines for an agent's session file.
pub fn handle_logs(app: &mut CcbdApp, payload: &Value) -> Result<Value, String> {
    let agent_name = str_field(payload, "agent_name")
        .filter(|s| !s.is_empty())
        .ok_or("logs requires agent_name")?;

    let tail = payload
        .get("tail")
        .and_then(|v| v.as_u64())
        .map(|n| (n as usize).clamp(1, MAX_TAIL))
        .unwrap_or(DEFAULT_TAIL);

    let entry = app
        .registry
        .get(&agent_name)
        .ok_or_else(|| format!("unknown agent: {}", agent_name))?;

    let provider = entry.provider.clone();
    let workspace_path = entry
        .workspace_path
        .clone()
        .unwrap_or_else(|| app.project_root.to_string_lossy().to_string());

    let session_ref = default_session_path(
        &provider,
        &agent_name,
        &app.project_root,
        Path::new(&workspace_path),
    )
    .map(|p| p.to_string_lossy().to_string());

    let entries = match session_ref.as_ref() {
        Some(path) if Path::new(path).exists() => {
            let lines =
                tail_file_lines(path, tail).map_err(|e| format!("read logs failed: {e}"))?;
            vec![json!({
                "source": "session",
                "path": path,
                "lines": lines,
            })]
        }
        _ => Vec::new(),
    };

    Ok(json!({
        "logs_status": "ok",
        "project_id": app.project_id(),
        "agent_name": agent_name,
        "provider": provider,
        "runtime_ref": entry.pane_id.clone().unwrap_or_default(),
        "session_ref": session_ref.unwrap_or_default(),
        "log_count": entries.iter().map(|e| e.get("lines").and_then(|l| l.as_array()).map(|a| a.len()).unwrap_or(0)).sum::<usize>(),
        "entries": entries,
    }))
}

fn tail_file_lines(path: &str, limit: usize) -> Result<Vec<String>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let metadata = reader.get_ref().metadata()?;
    let file_size = metadata.len();

    if file_size == 0 {
        return Ok(Vec::new());
    }

    // Estimate bytes to read: average 128 bytes per line, clamped to file size.
    let estimated_bytes = (limit * 128).min(file_size as usize) as u64;
    let start = file_size.saturating_sub(estimated_bytes);
    reader.seek(SeekFrom::Start(start))?;

    let mut lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    // Drop the first (likely partial) line unless we started at the beginning.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }

    // Keep only the last `limit` lines.
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }

    Ok(lines)
}
