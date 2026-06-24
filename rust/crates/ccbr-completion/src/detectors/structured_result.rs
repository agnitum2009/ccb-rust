use crate::detectors::base::{BaseDetector, CompletionDetector};
use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::first_non_empty;

/// Detector for providers that emit a structured result item.
pub struct StructuredResultDetector {
    base: BaseDetector,
}

impl Default for StructuredResultDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StructuredResultDetector {
    pub fn new() -> Self {
        Self {
            base: BaseDetector::new(),
        }
    }

    fn record_stream_reply(&mut self, item: &CompletionItem) {
        if let Some(text) = first_non_empty(&item.payload, &["reply", "final_answer", "text"]) {
            self.base.record_reply(item, &text, false, None);
        }
    }

    fn complete_from_result(&mut self, item: &CompletionItem) {
        let reply = first_non_empty(
            &item.payload,
            &["reply", "result_text", "final_answer", "text"],
        )
        .unwrap_or_default();
        if !reply.is_empty() {
            self.base.record_reply(item, &reply, true, None);
        }
        self.base.set_terminal(
            CompletionStatus::Completed,
            &first_non_empty(&item.payload, &["reason", "completion_reason"])
                .unwrap_or_else(|| "stream_result".into()),
            CompletionConfidence::Exact,
            &item.timestamp,
            &reply,
            None,
        );
    }

    fn terminal_from_item(
        &mut self,
        item: &CompletionItem,
        status: CompletionStatus,
        reason: &str,
        confidence: CompletionConfidence,
    ) {
        self.base.set_terminal(
            status,
            reason,
            confidence,
            &item.timestamp,
            "",
            Some(BaseDetector::terminal_diagnostics_from_item(item)),
        );
    }
}

impl CompletionDetector for StructuredResultDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        self.base.bind(request_ctx, baseline);
    }

    fn ingest(&mut self, item: &CompletionItem) {
        self.base.require_bound();
        self.base.consume_common_item(item);

        match item.kind {
            CompletionItemKind::AssistantChunk | CompletionItemKind::AssistantFinal => {
                self.record_stream_reply(item);
                self.base.set_pending();
            }
            CompletionItemKind::Result => {
                self.complete_from_result(item);
            }
            CompletionItemKind::CancelInfo => {
                self.terminal_from_item(
                    item,
                    CompletionStatus::Cancelled,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "cancel_info".into()),
                    CompletionConfidence::Exact,
                );
            }
            CompletionItemKind::TurnAborted => {
                self.terminal_from_item(
                    item,
                    BaseDetector::terminal_status_from_abort(item),
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "turn_aborted".into()),
                    CompletionConfidence::Exact,
                );
            }
            CompletionItemKind::Error => {
                self.terminal_from_item(
                    item,
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason", "error"])
                        .unwrap_or_else(|| "transport_error".into()),
                    CompletionConfidence::Exact,
                );
            }
            CompletionItemKind::PaneDead => {
                self.terminal_from_item(
                    item,
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "pane_dead".into()),
                    CompletionConfidence::Degraded,
                );
            }
            _ => {
                self.base.set_pending();
            }
        }
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
