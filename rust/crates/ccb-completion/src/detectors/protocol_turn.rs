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
            let reason = self.classify_empty_boundary(item);
            self.base.set_terminal(
                CompletionStatus::Incomplete,
                &reason,
                CompletionConfidence::Exact,
                &item.timestamp,
                "",
                Some(self.empty_boundary_diagnostics(item, &reason)),
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

    fn classify_empty_boundary(&self, item: &CompletionItem) -> String {
        if self.api_error_seen(item) {
            return "api_empty_after_error".into();
        }
        if !self.base.state().anchor_seen {
            return "delivery_late_empty".into();
        }
        "model_empty_output".into()
    }

    fn api_error_seen(&self, item: &CompletionItem) -> bool {
        for key in &["api_error_seen", "error_seen"] {
            match item.payload.get(*key) {
                Some(serde_json::Value::Bool(true)) => return true,
                Some(serde_json::Value::String(value)) => {
                    let trimmed = value.trim().to_lowercase();
                    if !trimmed.is_empty() && trimmed != "false" && trimmed != "0" {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn empty_boundary_diagnostics(
        &self,
        item: &CompletionItem,
        reason: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut diagnostics = BaseDetector::terminal_diagnostics_from_item(item);
        let diagnosis = self.empty_boundary_diagnosis(reason);
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
            .entry("empty_reply_reason")
            .or_insert(serde_json::Value::String(reason.into()));
        diagnostics
            .entry("error_type")
            .or_insert(serde_json::Value::String("empty_provider_reply".into()));
        diagnostics
            .entry("message")
            .or_insert(serde_json::Value::String(diagnosis.clone().into()));
        diagnostics
            .entry("diagnosis")
            .or_insert(serde_json::Value::String(diagnosis.into()));
        diagnostics
    }

    fn empty_boundary_diagnosis(&self, reason: &str) -> String {
        match reason {
            "api_empty_after_error" => "Provider reported an API error during the turn and then completed without assistant reply text; inspect the protocol session log and authentication/API output.".into(),
            "delivery_late_empty" => "Provider turn boundary arrived before the request anchor was observed; the prompt may not have been delivered or the reader was bound to stale history.".into(),
            _ => "Provider protocol reported task_complete without assistant reply text; inspect the protocol session log, pane state, and authentication/API output.".into(),
        }
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
