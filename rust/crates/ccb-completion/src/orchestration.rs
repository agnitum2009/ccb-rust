use std::time::{Duration, Instant};

use crate::detectors::CompletionDetector;
use crate::models::{CompletionDecision, CompletionRequestContext};
use crate::selectors::ReplySelector;
use crate::sources::CompletionSource;
use crate::utils::utc_now_iso;

/// Orchestrates polling a completion source until a terminal decision is reached.
pub struct CompletionOrchestrator {
    now_factory: Box<dyn Fn() -> String + Send + Sync>,
}

impl Default for CompletionOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionOrchestrator {
    pub fn new() -> Self {
        Self {
            now_factory: Box::new(utc_now_iso),
        }
    }

    pub fn with_now_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.now_factory = Box::new(factory);
        self
    }

    pub fn run(
        &self,
        request_ctx: &CompletionRequestContext,
        source: &mut dyn CompletionSource,
        detector: &mut dyn CompletionDetector,
        selector: &mut dyn ReplySelector,
    ) -> CompletionDecision {
        let baseline = source.capture_baseline();
        detector.bind(
            CompletionRequestContext::new(
                request_ctx.req_id.clone(),
                request_ctx.agent_name.clone(),
                request_ctx.provider.clone(),
                request_ctx.timeout_s,
            )
            .expect("request context already validated"),
            baseline.clone(),
        );
        let mut cursor = baseline;
        let deadline = Instant::now() + Duration::from_secs_f64(request_ctx.timeout_s);

        loop {
            let remaining = (deadline - Instant::now()).as_secs_f64();
            if remaining <= 0.0 {
                break;
            }
            let timeout = request_ctx.poll_interval_s.min(remaining);
            let item = source.poll(&cursor, timeout);
            match item {
                None => {
                    let now = (self.now_factory)();
                    detector.tick(&now, Some(&cursor));
                    let decision = detector.decision();
                    if decision.terminal {
                        return self.finalize(selector, decision);
                    }
                }
                Some(item) => {
                    cursor = item.cursor.clone();
                    for candidate in crate::models::reply_candidates_from_item(&item) {
                        selector.ingest_candidate(candidate);
                    }
                    detector.ingest(&item);
                    let decision = detector.decision();
                    if decision.terminal {
                        return self.finalize(selector, decision);
                    }
                }
            }
        }

        let now = (self.now_factory)();
        detector.finalize_timeout(&now, Some(&cursor));
        let decision = detector.decision();
        self.finalize(selector, decision)
    }

    fn finalize(
        &self,
        selector: &dyn ReplySelector,
        decision: CompletionDecision,
    ) -> CompletionDecision {
        if !decision.terminal {
            return decision;
        }
        let reply = selector.select(&decision);
        if !reply.is_empty() && decision.reply.is_empty() {
            decision.with_reply(reply)
        } else {
            decision
        }
    }
}
