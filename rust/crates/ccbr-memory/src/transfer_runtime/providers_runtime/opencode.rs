//! Mirrors Python `lib/memory/transfer_runtime/providers_runtime/opencode.py`.

use crate::deduper::ConversationDeduper;
use crate::formatter::ContextFormatter;
use crate::transfer::{context_from_pairs, load_session_data};
use crate::types::{MemoryError, Result, TransferContext};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Extract transfer context from an OpenCode session by capturing the current
/// session state and reading its messages.
pub fn extract_from_opencode(
    work_dir: &Path,
    _source_session_files: &HashMap<String, String>,
    deduper: &ConversationDeduper,
    formatter: &ContextFormatter,
    max_tokens: u32,
    fallback_pairs: usize,
    last_n: usize,
) -> Result<TransferContext> {
    let (_session_file, data) = load_session_data(work_dir, "opencode");

    let project_id = data
        .get("opencode_project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let (session_id, session_path) = capture_opencode_session(work_dir, &data)?;

    let pairs = read_opencode_session_pairs(&session_path)?;
    let effective_last_n = if last_n > 0 { last_n } else { fallback_pairs };

    let mut ctx = context_from_pairs(
        deduper,
        formatter,
        max_tokens,
        &pairs,
        "opencode",
        &session_id,
        Some(&session_path),
        effective_last_n,
        None,
    );

    if let Some(obj) = ctx.metadata.as_object_mut() {
        obj.insert(
            "project_id".to_string(),
            serde_json::Value::String(project_id),
        );
    }

    Ok(ctx)
}

fn capture_opencode_session(
    work_dir: &Path,
    data: &serde_json::Map<String, serde_json::Value>,
) -> Result<(String, PathBuf)> {
    if let (Some(id), Some(path)) = (
        data.get("opencode_session_id").and_then(|v| v.as_str()),
        data.get("opencode_session_path").and_then(|v| v.as_str()),
    ) {
        if !path.is_empty() {
            return Ok((id.to_string(), PathBuf::from(path)));
        }
    }

    // Fallback: scan for a session file.
    let candidates = opencode_session_candidates(work_dir);
    let (id, path) = candidates
        .into_iter()
        .next()
        .ok_or_else(|| MemoryError::SessionNotFound("No OpenCode session found".to_string()))?;

    if !path.exists() {
        return Err(MemoryError::SessionNotFound(
            "No OpenCode session found".to_string(),
        ));
    }

    Ok((id, path))
}

fn opencode_session_candidates(work_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();

    // .opencode/sessions/<session_id>/messages.json
    let sessions_dir = work_dir.join(".opencode").join("sessions");
    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            let session_dir = entry.path();
            let messages = session_dir.join("messages.json");
            if messages.is_file() {
                let id = session_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("session")
                    .to_string();
                out.push((id, messages));
            }
        }
    }

    // Flat session.json in work_dir.
    if let Ok(entries) = std::fs::read_dir(work_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("session")
                    .to_string();
                out.push((id, path));
            }
        }
    }

    out
}

fn read_opencode_session_pairs(path: &Path) -> Result<Vec<(String, String)>> {
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| MemoryError::SessionParse(format!("invalid opencode session JSON: {e}")))?;

    let messages = value
        .get("messages")
        .and_then(|v| v.as_array())
        .or_else(|| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut pairs = Vec::new();
    let mut current_user: Option<String> = None;
    for msg in messages {
        if let Some(obj) = msg.as_object() {
            let role = obj
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let content = obj
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if role == "user" {
                current_user = Some(content);
            } else if role == "assistant" && current_user.is_some() {
                pairs.push((current_user.take().unwrap(), content));
            }
        }
    }
    Ok(pairs)
}
