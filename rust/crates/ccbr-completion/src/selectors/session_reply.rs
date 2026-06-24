use crate::models::{CompletionDecision, ReplyCandidate};
use crate::selectors::base::{BaseReplySelector, ReplySelector};

pub struct SessionReplySelector {
    base: BaseReplySelector,
}

impl Default for SessionReplySelector {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionReplySelector {
    pub fn new() -> Self {
        Self {
            base: BaseReplySelector::new(),
        }
    }
}

impl ReplySelector for SessionReplySelector {
    fn ingest_candidate(&mut self, candidate: ReplyCandidate) {
        self.base.ingest_candidate(candidate);
    }

    fn select(&self, decision: &CompletionDecision) -> String {
        self.base.select(decision)
    }

    fn preview(&self) -> String {
        self.base.preview()
    }

    fn reset(&mut self) {
        self.base.reset();
    }
}
