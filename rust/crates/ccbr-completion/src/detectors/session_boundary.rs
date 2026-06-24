use crate::detectors::base::{BaseDetector, CompletionDetector};
use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::first_non_empty;

/// Detector that treats a turn boundary as completion.
pub struct SessionBoundaryDetector {
    base: BaseDetector,
}

impl Default for SessionBoundaryDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBoundaryDetector {
    pub fn new() -> Self {
        Self {
            base: BaseDetector::new(),
        }
    }
}

impl CompletionDetector for SessionBoundaryDetector {
    fn bind(&mut self, request_ctx: CompletionRequestContext, baseline: CompletionCursor) {
        self.base.bind(request_ctx, baseline);
    }

    fn ingest(&mut self, item: &CompletionItem) {
        self.base.require_bound();
        self.base.consume_common_item(item);

        match item.kind {
            CompletionItemKind::AssistantChunk | CompletionItemKind::AssistantFinal => {
                if let Some(text) =
                    first_non_empty(&item.payload, &["text", "reply", "last_agent_message"])
                {
                    self.base.record_reply(item, &text, false, None);
                }
                self.base.set_pending();
            }
            CompletionItemKind::TurnBoundary => {
                let reply =
                    first_non_empty(&item.payload, &["last_agent_message", "reply", "text"])
                        .unwrap_or_default();
                if !reply.is_empty() {
                    self.base.record_reply(item, &reply, true, None);
                }
                self.base.set_terminal(
                    CompletionStatus::Completed,
                    &first_non_empty(&item.payload, &["reason", "completion_reason"])
                        .unwrap_or_else(|| "turn_duration".into()),
                    CompletionConfidence::Observed,
                    &item.timestamp,
                    &reply,
                    None,
                );
            }
            CompletionItemKind::Error => {
                self.base.set_terminal(
                    CompletionStatus::Failed,
                    &first_non_empty(&item.payload, &["reason", "error"])
                        .unwrap_or_else(|| "api_error".into()),
                    CompletionConfidence::Observed,
                    &item.timestamp,
                    "",
                    Some(BaseDetector::terminal_diagnostics_from_item(item)),
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
                    Some(BaseDetector::terminal_diagnostics_from_item(item)),
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
