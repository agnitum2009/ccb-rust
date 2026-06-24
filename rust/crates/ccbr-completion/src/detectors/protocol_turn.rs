use crate::detectors::base::{BaseDetector, CompletionDetector};
use crate::models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionItem, CompletionItemKind,
    CompletionRequestContext, CompletionState, CompletionStatus,
};
use crate::utils::first_non_empty;

/// Detector that follows explicit provider turn boundaries.
pub struct ProtocolTurnDetector {
    base: BaseDetector,
}

impl Default for ProtocolTurnDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolTurnDetector {
    pub fn new() -> Self {
        Self {
            base: BaseDetector::new(),
        }
    }

    fn record_transcript_reply(&mut self, item: &CompletionItem) {
        if let Some(text) = first_non_empty(
            &item.payload,
            &[
                "last_agent_message",
                "final_answer",
                "reply",
                "result_text",
                "text",
            ],
        ) {
            self.base.record_reply(item, &text, false, None);
        }
    }

    fn complete_from_boundary(&mut self, item: &CompletionItem) {
        let reply = first_non_empty(
            &item.payload,
            &["last_agent_message", "final_answer", "reply", "text"],
        )
        .unwrap_or_default();
        if !reply.is_empty() {
            self.base.record_reply(item, &reply, true, None);
        } else if !self.base.state().reply_started {
            self.base.set_terminal(
                CompletionStatus::Incomplete,
                "task_complete_empty_reply",
                CompletionConfidence::Exact,
                &item.timestamp,
                "",
                Some(self.empty_boundary_diagnostics(item)),
            );
            return;
        }
        self.base.set_terminal(
            CompletionStatus::Completed,
            &first_non_empty(&item.payload, &["reason", "completion_reason"])
                .unwrap_or_else(|| "task_complete".into()),
            CompletionConfidence::Exact,
            &item.timestamp,
            &reply,
            None,
        );
    }

    fn empty_boundary_diagnostics(
        &self,
        item: &CompletionItem,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut diagnostics = BaseDetector::terminal_diagnostics_from_item(item);
        let diagnosis = "Provider protocol reported task_complete without assistant reply text; inspect the protocol session log, pane state, and authentication/API output.";
        diagnostics
            .entry("provider_terminal_reason")
            .or_insert_with(|| {
                serde_json::Value::String(
                    first_non_empty(&item.payload, &["reason", "completion_reason"])
                        .unwrap_or_else(|| "task_complete".into()),
                )
            });
        diagnostics
            .entry("empty_reply")
            .or_insert(serde_json::Value::Bool(true));
        diagnostics
            .entry("error_type")
            .or_insert(serde_json::Value::String("empty_provider_reply".into()));
        diagnostics
            .entry("message")
            .or_insert(serde_json::Value::String(diagnosis.into()));
        diagnostics
            .entry("diagnosis")
            .or_insert(serde_json::Value::String(diagnosis.into()));
        diagnostics
    }

    fn complete_from_abort(&mut self, item: &CompletionItem) {
        let reply = first_non_empty(&item.payload, &["last_agent_message", "reply", "text"])
            .unwrap_or_default();
        if !reply.is_empty() {
            self.base.record_reply(item, &reply, true, None);
        }
        self.base.set_terminal(
            BaseDetector::terminal_status_from_abort(item),
            &first_non_empty(&item.payload, &["reason"]).unwrap_or_else(|| "turn_aborted".into()),
            CompletionConfidence::Exact,
            &item.timestamp,
            &reply,
            Some(BaseDetector::terminal_diagnostics_from_item(item)),
        );
    }

    fn fail_terminal(&mut self, item: &CompletionItem, reason: &str) {
        self.base.set_terminal(
            CompletionStatus::Failed,
            reason,
            CompletionConfidence::Degraded,
            &item.timestamp,
            "",
            Some(BaseDetector::terminal_diagnostics_from_item(item)),
        );
    }
}

impl CompletionDetector for ProtocolTurnDetector {
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
                self.record_transcript_reply(item);
                self.base.set_pending();
            }
            CompletionItemKind::TurnBoundary => {
                self.complete_from_boundary(item);
            }
            CompletionItemKind::TurnAborted => {
                self.complete_from_abort(item);
            }
            CompletionItemKind::Error => {
                self.fail_terminal(
                    item,
                    &first_non_empty(&item.payload, &["reason", "error"])
                        .unwrap_or_else(|| "transport_error".into()),
                );
            }
            CompletionItemKind::PaneDead => {
                self.fail_terminal(
                    item,
                    &first_non_empty(&item.payload, &["reason"])
                        .unwrap_or_else(|| "pane_dead".into()),
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
