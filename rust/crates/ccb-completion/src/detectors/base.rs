use serde_json::Map;

use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::{fingerprint_text, first_non_empty};

/// Trait implemented by all completion detectors.
pub trait CompletionDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor);
    fn ingest(&mut self, item: &CompletionItem);
    fn decision(&self) -> CompletionDecision;
    fn state(&self) -> CompletionState;

    /// Called periodically when no new items are available.
    fn tick(&mut self, _now: &str, _cursor: Option<&CompletionCursor>) {}

    /// Called once when the request timeout is reached.
    fn finalize_timeout(&mut self, _now: &str, _cursor: Option<&CompletionCursor>) {}
}

/// Marker trait for detectors that support tick/finalize timeout.
///
/// Mirrors Python `TickableCompletionDetector`. Because the base trait already
/// provides default no-op implementations, every `CompletionDetector` is also
/// tickable.
pub trait TickableCompletionDetector: CompletionDetector {}

impl<T: CompletionDetector> TickableCompletionDetector for T {}

/// Alias matching the Python name `BaseCompletionDetector`.
pub type BaseCompletionDetector = BaseDetector;

/// Shared state and helper methods used by concrete detectors.
pub struct BaseDetector {
    request_ctx: Option<CompletionRequestContext>,
    state: CompletionState,
    decision: CompletionDecision,
}

impl Default for BaseDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseDetector {
    pub fn new() -> Self {
        Self {
            request_ctx: None,
            state: CompletionState::default(),
            decision: CompletionDecision::pending(None),
        }
    }

    pub fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        self.request_ctx = Some(request_ctx);
        self.state = CompletionState {
            latest_cursor: Some(baseline),
            ..CompletionState::default()
        };
        self.decision = CompletionDecision::pending(self.state.latest_cursor.clone());
    }

    pub fn decision(&self) -> CompletionDecision {
        self.decision.clone()
    }

    pub fn state(&self) -> CompletionState {
        self.state.clone()
    }

    pub fn state_mut(&mut self) -> &mut CompletionState {
        &mut self.state
    }

    pub fn require_bound(&self) -> &CompletionRequestContext {
        self.request_ctx
            .as_ref()
            .expect("detector must be bound before use")
    }

    pub fn sync_cursor(&mut self, cursor: Option<&CompletionCursor>) {
        if let Some(c) = cursor {
            self.state.latest_cursor = Some(c.clone());
        }
    }

    pub fn base_tick(&mut self, _now: &str, cursor: Option<&CompletionCursor>) {
        self.sync_cursor(cursor);
    }

    pub fn base_finalize_timeout(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        if self.decision.terminal {
            return;
        }
        self.sync_cursor(cursor);
        self.set_terminal(
            CompletionStatus::Incomplete,
            "timeout",
            CompletionConfidence::Degraded,
            now,
            "",
            None,
        );
    }

    pub fn consume_common_item(&mut self, item: &CompletionItem) {
        self.sync_cursor(Some(&item.cursor));

        match item.kind {
            CompletionItemKind::AnchorSeen => {
                self.state.anchor_seen = true;
            }
            CompletionItemKind::ToolCall => {
                self.state.tool_active = true;
            }
            CompletionItemKind::ToolResult => {
                self.state.tool_active = false;
            }
            CompletionItemKind::SessionRotate => {
                self.state.anchor_seen = false;
                self.state.reply_started = false;
                self.state.reply_stable = false;
                self.state.tool_active = false;
                self.state.last_reply_hash = None;
                self.state.last_reply_at = None;
                self.state.stable_since = None;
            }
            _ => {}
        }

        if first_non_empty(&item.payload, &["subagent_id", "subagent_name"]).is_some() {
            self.state.subagent_activity_seen = true;
        }

        if let Some(turn_ref) = first_non_empty(
            &item.payload,
            &[
                "turn_id",
                "provider_turn_ref",
                "message_id",
                "provider_session_id",
                "session_id",
            ],
        ) {
            self.state.provider_turn_ref = Some(turn_ref);
        }
    }

    pub fn record_reply(
        &mut self,
        item: &CompletionItem,
        text: &str,
        stable: bool,
        fingerprint: Option<&str>,
    ) {
        let message = text.trim();
        if message.is_empty() {
            return;
        }
        self.state.reply_started = true;
        self.state.last_reply_hash = Some(fingerprint_text(&[fingerprint.unwrap_or(message)]));
        self.state.last_reply_at = Some(item.timestamp.clone());
        self.state.reply_stable = stable;
    }

    pub fn terminal_diagnostics_from_item(item: &CompletionItem) -> Map<String, serde_json::Value> {
        let mut payload = item.payload.clone();
        for key in &[
            "reply",
            "content",
            "final_answer",
            "result_text",
            "last_agent_message",
        ] {
            payload.remove(*key);
        }
        payload
    }

    pub fn set_terminal(
        &mut self,
        status: CompletionStatus,
        reason: &str,
        confidence: CompletionConfidence,
        finished_at: &str,
        reply: &str,
        diagnostics: Option<Map<String, serde_json::Value>>,
    ) {
        self.state.terminal = true;
        self.decision = CompletionDecision {
            terminal: true,
            status,
            reason: Some(reason.to_string()),
            confidence: Some(confidence),
            reply: reply.to_string(),
            anchor_seen: self.state.anchor_seen,
            reply_started: self.state.reply_started,
            reply_stable: self.state.reply_stable,
            provider_turn_ref: self.state.provider_turn_ref.clone(),
            source_cursor: self.state.latest_cursor.clone(),
            finished_at: Some(finished_at.to_string()),
            diagnostics: diagnostics.unwrap_or_default(),
        };
    }

    pub fn set_pending(&mut self) {
        self.decision = self.pending_decision();
    }

    fn pending_decision(&self) -> CompletionDecision {
        CompletionDecision {
            terminal: false,
            status: CompletionStatus::Incomplete,
            reason: None,
            confidence: None,
            reply: String::new(),
            anchor_seen: self.state.anchor_seen,
            reply_started: self.state.reply_started,
            reply_stable: self.state.reply_stable,
            provider_turn_ref: self.state.provider_turn_ref.clone(),
            source_cursor: self.state.latest_cursor.clone(),
            finished_at: None,
            diagnostics: Map::new(),
        }
    }

    pub fn terminal_status_from_abort(item: &CompletionItem) -> CompletionStatus {
        let raw_status = item
            .payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        match raw_status.as_str() {
            "completed" | "cancelled" | "failed" | "incomplete" => {
                return CompletionStatus::from_record_str(&raw_status);
            }
            _ => {}
        }
        let reason = item
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if reason.contains("cancel") || reason.contains("abort") {
            CompletionStatus::Cancelled
        } else {
            CompletionStatus::Failed
        }
    }
}

impl CompletionDetector for BaseDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        BaseDetector::bind(self, request_ctx, baseline);
    }

    fn ingest(&mut self, item: &CompletionItem) {
        self.consume_common_item(item);
    }

    fn decision(&self) -> CompletionDecision {
        BaseDetector::decision(self)
    }

    fn state(&self) -> CompletionState {
        BaseDetector::state(self)
    }

    fn tick(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base_tick(now, cursor);
    }

    fn finalize_timeout(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base_finalize_timeout(now, cursor);
    }
}

impl CompletionStatus {
    pub fn from_record_str(value: &str) -> Self {
        match value {
            "completed" => CompletionStatus::Completed,
            "cancelled" => CompletionStatus::Cancelled,
            "failed" => CompletionStatus::Failed,
            _ => CompletionStatus::Incomplete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_completion_detector_alias_and_tickable_trait() {
        // Compile-only check that the Python-named alias and marker trait exist.
        fn assert_tickable<T: TickableCompletionDetector>() {}
        assert_tickable::<BaseCompletionDetector>();
        let _: BaseCompletionDetector = BaseDetector::new();
    }
}
