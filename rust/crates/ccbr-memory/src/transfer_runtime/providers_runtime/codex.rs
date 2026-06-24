//! Mirrors Python `lib/memory/transfer_runtime/providers_runtime/codex.py`.

use crate::deduper::ConversationDeduper;
use crate::formatter::ContextFormatter;
use crate::transfer::{context_from_pairs, load_session_data};
use crate::types::{MemoryError, Result, TransferContext};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Extract transfer context from a Codex session, falling back to the latest
/// `.jsonl` log in `work_dir` when the bound session file is missing or invalid.
pub fn extract_from_codex(
    work_dir: &Path,
    _source_session_files: &HashMap<String, String>,
    deduper: &ConversationDeduper,
    formatter: &ContextFormatter,
    max_tokens: u32,
    fallback_pairs: usize,
    last_n: usize,
) -> Result<TransferContext> {
    let (_session_file, data) = load_session_data(work_dir, "codex");

    let resolved_path = data
        .get("codex_session_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| latest_jsonl_log(work_dir));

    let resolved_path = match resolved_path {
        Some(p) => p,
        None => {
            return Err(MemoryError::SessionNotFound(
                "No Codex session found".to_string(),
            ))
        }
    };

    let pairs = read_codex_log_pairs(&resolved_path)?;
    let session_id = resolved_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("latest")
        .to_string();

    let effective_last_n = if last_n > 0 { last_n } else { fallback_pairs };

    Ok(context_from_pairs(
        deduper,
        formatter,
        max_tokens,
        &pairs,
        "codex",
        &session_id,
        Some(&resolved_path),
        effective_last_n,
        None,
    ))
}

fn latest_jsonl_log(work_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(work_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        candidates.push((modified, path));
                    } else {
                        candidates.push((std::time::SystemTime::UNIX_EPOCH, path));
                    }
                }
            }
        }
    }
    candidates.sort_by_key(|b| std::cmp::Reverse(b.0));
    candidates.into_iter().next().map(|(_, p)| p)
}

fn read_codex_log_pairs(path: &Path) -> Result<Vec<(String, String)>> {
    let raw = std::fs::read_to_string(path)?;
    let mut messages: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| MemoryError::SessionParse(format!("invalid codex log line: {e}")))?;
        if let Some(obj) = value.as_object() {
            if let (Some(role), Some(content)) = (
                obj.get("role").and_then(|v| v.as_str()),
                obj.get("content").and_then(|v| v.as_str()),
            ) {
                messages.push((role.to_string(), content.to_string()));
                continue;
            }
            if let (Some(user), Some(assistant)) = (
                obj.get("user").and_then(|v| v.as_str()),
                obj.get("assistant").and_then(|v| v.as_str()),
            ) {
                messages.push(("user".to_string(), user.to_string()));
                messages.push(("assistant".to_string(), assistant.to_string()));
            }
        }
    }

    let mut pairs = Vec::new();
    let mut current_user: Option<String> = None;
    for (role, content) in messages {
        if role == "user" {
            current_user = Some(content);
        } else if role == "assistant" && current_user.is_some() {
            pairs.push((current_user.take().unwrap(), content));
        }
    }
    Ok(pairs)
}
