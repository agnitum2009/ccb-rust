use crate::models::{CompletionDecision, ReplyCandidate};

/// Trait implemented by all reply selectors.
pub trait ReplySelector: Send {
    fn ingest_candidate(&mut self, candidate: ReplyCandidate);
    fn select(&self, decision: &CompletionDecision) -> String;
    fn preview(&self) -> String;
    fn reset(&mut self);
}

/// Selects the best reply candidate using a priority ordering.
pub struct BaseReplySelector {
    candidates: Vec<(usize, ReplyCandidate)>,
    sequence: usize,
}

impl Default for BaseReplySelector {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseReplySelector {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            sequence: 0,
        }
    }

    pub fn ingest_candidate(&mut self, candidate: ReplyCandidate) {
        self.candidates.push((self.sequence, candidate));
        self.sequence += 1;
    }

    pub fn reset(&mut self) {
        self.candidates.clear();
    }

    pub fn select(&self, decision: &CompletionDecision) -> String {
        if !decision.terminal {
            panic!("cannot select reply before terminal decision");
        }
        if !decision.reply.is_empty() {
            return decision.reply.clone();
        }
        self.best_candidate()
            .map(|c| c.text.clone())
            .unwrap_or_default()
    }

    pub fn preview(&self) -> String {
        self.best_candidate()
            .map(|c| c.text.clone())
            .unwrap_or_default()
    }

    fn best_candidate(&self) -> Option<&ReplyCandidate> {
        self.candidates
            .iter()
            .min_by_key(|(seq, c)| (c.priority, -(*seq as i64)))
            .map(|(_, c)| c)
    }
}
