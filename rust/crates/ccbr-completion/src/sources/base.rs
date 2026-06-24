use crate::models::{CompletionCursor, CompletionItem};

/// Source of completion items consumed by the orchestrator.
pub trait CompletionSource {
    fn capture_baseline(&self) -> CompletionCursor;
    fn poll(&mut self, cursor: &CompletionCursor, timeout_s: f64) -> Option<CompletionItem>;
}
