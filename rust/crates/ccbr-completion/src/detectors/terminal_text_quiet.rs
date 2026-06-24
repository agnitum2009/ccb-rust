use crate::detectors::base::{BaseDetector, CompletionDetector};
use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::first_non_empty;

/// Detector for quiet terminal text streams that rely on done markers.
pub struct TerminalTextQuietDetector {
    base: BaseDetector,
}

impl Default for TerminalTextQuietDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalTextQuietDetector {
    pub fn new() -> Self {
        Self {
            base: BaseDetector::new(),
        }
    }
}

impl CompletionDetector for TerminalTextQuietDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        self.base.bind(request_ctx, baseline);
    }

    fn ingest(&mut self, item: &CompletionItem) {
        self.base.require_bound();
        self.base.consume_common_item(item);

        match item.kind {
            CompletionItemKind::AssistantChunk
            | CompletionItemKind::AssistantFinal
            | CompletionItemKind::Result => {
                let reply = first_non_empty(
                    &item.payload,
                    &["reply", "result_text", "final_answer", "text"],
                )
                .unwrap_or_default();
                if !reply.is_empty() {
                    self.base.record_reply(item, &reply, false, None);
                }
                if item
                    .payload
                    .get("done_marker")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    || item
                        .payload
                        .get("ccbr_done")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                {
                    self.base.set_terminal(
                        CompletionStatus::Completed,
                        "terminal_done_marker",
                        CompletionConfidence::Degraded,
                        &item.timestamp,
                        &reply,
                        None,
                    );
                    return;
                }
                self.base.set_pending();
            }
            CompletionItemKind::CancelInfo => {
                self.base.set_terminal(
                    CompletionStatus::Cancelled,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "cancel_info".into()),
                    CompletionConfidence::Degraded,
                    &item.timestamp,
                    "",
                    None,
                );
            }
            CompletionItemKind::Error => {
                self.base.set_terminal(
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason", "error"])
                        .unwrap_or_else(|| "transport_error".into()),
                    CompletionConfidence::Degraded,
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

    fn finalize_timeout(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base.require_bound();
        if self.base.decision().terminal {
            return;
        }
        self.base.sync_cursor(cursor);
        if self.base.state().reply_started {
            self.base.set_terminal(
                CompletionStatus::Completed,
                "terminal_quiet",
                CompletionConfidence::Degraded,
                now,
                "",
                None,
            );
            return;
        }
        self.base.base_finalize_timeout(now, cursor);
    }

    fn decision(&self) -> CompletionDecision {
        self.base.decision()
    }

    fn state(&self) -> CompletionState {
        self.base.state()
    }

    fn tick(&mut self, now: &str, cursor: Option<&CompletionCursor>) {
        self.base.base_tick(now, cursor);
    }
}
