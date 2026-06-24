//! Decision constructors for reply-delivery completion.

use ccbr_completion::models::{CompletionConfidence, CompletionDecision, CompletionStatus};
use serde_json::Map;

/// Build a successful reply-delivery completion decision.
pub fn reply_delivery_completed_decision(
    finished_at: &str,
    provider_turn_ref: Option<&str>,
) -> CompletionDecision {
    CompletionDecision {
        terminal: true,
        status: CompletionStatus::Completed,
        reason: Some("reply_delivery_completed".into()),
        confidence: Some(CompletionConfidence::Exact),
        reply: String::new(),
        anchor_seen: true,
        reply_started: true,
        reply_stable: true,
        provider_turn_ref: provider_turn_ref.map(String::from),
        source_cursor: None,
        finished_at: Some(finished_at.into()),
        diagnostics: Map::new(),
    }
}

/// Build a failed reply-delivery completion decision.
pub fn reply_delivery_failed_decision(
    finished_at: &str,
    reason: &str,
    diagnostics: Option<Map<String, serde_json::Value>>,
) -> CompletionDecision {
    CompletionDecision {
        terminal: true,
        status: CompletionStatus::Failed,
        reason: Some(reason.into()),
        confidence: Some(CompletionConfidence::Degraded),
        reply: String::new(),
        anchor_seen: true,
        reply_started: true,
        reply_stable: false,
        provider_turn_ref: None,
        source_cursor: None,
        finished_at: Some(finished_at.into()),
        diagnostics: diagnostics.unwrap_or_default(),
    }
}
