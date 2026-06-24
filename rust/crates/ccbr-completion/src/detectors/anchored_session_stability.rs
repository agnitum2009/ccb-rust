use crate::detectors::base::{BaseDetector, CompletionDetector};
use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::{fingerprint_text, first_non_empty, seconds_between};

/// Detector that waits for the session reply to become stable.
pub struct AnchoredSessionStabilityDetector {
    base: BaseDetector,
    settle_window_s: f64,
}

impl Default for AnchoredSessionStabilityDetector {
    fn default() -> Self {
        Self::new(2.0)
    }
}

impl AnchoredSessionStabilityDetector {
    pub fn new(settle_window_s: f64) -> Self {
        Self {
            base: BaseDetector::new(),
            settle_window_s,
        }
    }
}

impl CompletionDetector for AnchoredSessionStabilityDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        self.base.bind(request_ctx, baseline);
    }

    fn ingest(&mut self, item: &CompletionItem) {
        self.base.require_bound();
        self.base.consume_common_item(item);

        match item.kind {
            CompletionItemKind::CancelInfo => {
                self.base.set_terminal(
                    CompletionStatus::Cancelled,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "cancel_info".into()),
                    CompletionConfidence::Observed,
                    &item.timestamp,
                    "",
                    None,
                );
            }
            CompletionItemKind::SessionSnapshot | CompletionItemKind::SessionMutation => {
                if let Some(raw) = item.payload.get("tool_call_count") {
                    let tool_active = if let Some(n) = raw.as_u64() {
                        n > 0
                    } else if let Some(b) = raw.as_bool() {
                        b
                    } else if let Some(s) = raw.as_str() {
                        s.parse::<u64>().map(|n| n > 0).unwrap_or(true)
                    } else {
                        true
                    };
                    self.base.state_mut().tool_active = tool_active;
                }

                if let Some(reply) = first_non_empty(&item.payload, &["reply", "content", "text"]) {
                    let fingerprint = fingerprint_text(&[
                        &first_non_empty(&item.payload, &["message_id"]).unwrap_or_default(),
                        &reply,
                        &item
                            .payload
                            .get("message_count")
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                        item.payload
                            .get("last_updated")
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    ]);
                    if self.base.state().last_reply_hash.as_deref() != Some(&fingerprint) {
                        self.base
                            .record_reply(item, &reply, false, Some(&fingerprint));
                        self.base.state_mut().stable_since = Some(item.timestamp.clone());
                    }
                    self.base.set_pending();
                }
            }
            CompletionItemKind::Error => {
                self.base.set_terminal(
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason", "error"])
                        .unwrap_or_else(|| "session_corrupt".into()),
                    CompletionConfidence::Observed,
                    &item.timestamp,
                    "",
                    None,
                );
            }
            CompletionItemKind::PaneDead => {
                self.base.set_terminal(
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "pane_dead".into()),
                    CompletionConfidence::Degraded,
                    &item.timestamp,
                    "",
                    None,
                );
            }
            _ => {
                self.base.set_pending();
            }
        }
    }

    fn tick(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base.base_tick(now, cursor);
        if self.base.decision().terminal {
            return;
        }
        if !self.base.state().reply_started || self.base.state().stable_since.is_none() {
            return;
        }
        if self.base.state().tool_active {
            self.base.set_pending();
            return;
        }
        let state = self.base.state();
        let stable_since = state.stable_since.as_deref().unwrap();
        match seconds_between(stable_since, now) {
            Ok(elapsed) if elapsed >= self.settle_window_s => {
                self.base.state_mut().reply_stable = true;
                self.base.set_terminal(
                    CompletionStatus::Completed,
                    "session_reply_stable",
                    CompletionConfidence::Observed,
                    now,
                    "",
                    None,
                );
            }
            _ => {
                self.base.set_pending();
            }
        }
    }

    fn finalize_timeout(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base.require_bound();
        if self.base.decision().terminal {
            return;
        }
        self.base.sync_cursor(cursor);
        self.base.base_finalize_timeout(now, cursor);
    }

    fn decision(&self) -> CompletionDecision {
        self.base.decision()
    }

    fn state(&self) -> CompletionState {
        self.base.state()
    }
}
