use crate::detectors::{
    AnchoredSessionStabilityDetector, CompletionDetector, ProtocolTurnDetector,
    SessionBoundaryDetector, StructuredResultDetector, TerminalTextQuietDetector,
};
use crate::error::Result;
use crate::models::{CompletionFamily, CompletionProfile};
use crate::profiles::{build_completion_profile, CompletionManifest};
use crate::selectors::{
    FinalMessageSelector, ReplySelector, SessionReplySelector, StructuredResultSelector,
};
use ccb_agents::models::AgentSpec;

/// Registry that creates completion profiles, detectors, and selectors.
#[derive(Debug, Default)]
pub struct CompletionRegistry;

impl CompletionRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn build_profile(
        &self,
        agent_spec: &AgentSpec,
        _runtime_ref: Option<&str>,
        manifest: &CompletionManifest,
    ) -> Result<CompletionProfile> {
        build_completion_profile(agent_spec, manifest)
    }

    pub fn build_detector(
        &self,
        profile: &CompletionProfile,
    ) -> Box<dyn CompletionDetector + Send> {
        match profile.completion_family {
            CompletionFamily::ProtocolTurn => Box::new(ProtocolTurnDetector::new()),
            CompletionFamily::StructuredResult => Box::new(StructuredResultDetector::new()),
            CompletionFamily::SessionBoundary => Box::new(SessionBoundaryDetector::new()),
            CompletionFamily::AnchoredSessionStability => {
                Box::new(AnchoredSessionStabilityDetector::new(2.0))
            }
            CompletionFamily::TerminalTextQuiet => Box::new(TerminalTextQuietDetector::new()),
        }
    }

    pub fn build_selector(&self, profile: &CompletionProfile) -> Box<dyn ReplySelector + Send> {
        match profile.selector_family {
            crate::models::SelectorFamily::FinalMessage => Box::new(FinalMessageSelector::new()),
            crate::models::SelectorFamily::StructuredResult => {
                Box::new(StructuredResultSelector::new())
            }
            crate::models::SelectorFamily::SessionReply => Box::new(SessionReplySelector::new()),
        }
    }
}
