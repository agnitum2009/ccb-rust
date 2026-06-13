pub mod detectors;
pub mod error;
pub mod models;
pub mod orchestration;
pub mod profiles;
pub mod registry;
pub mod selectors;
pub mod snapshot_store;
pub mod sources;
pub mod tracker;
pub mod utils;

pub use error::{CompletionError, Result};
pub use models::reply_candidates_from_item;
pub use models::{
    CompletionConfidence, CompletionCursor, CompletionDecision, CompletionFamily, CompletionItem,
    CompletionItemKind, CompletionProfile, CompletionRequestContext, CompletionSnapshot,
    CompletionSourceKind, CompletionState, CompletionStatus, JobRecord, ReplyCandidate,
    ReplyCandidateKind, SelectorFamily, TargetKind,
};
pub use orchestration::CompletionOrchestrator;
pub use profiles::{build_completion_profile, CompletionManifest, CompletionManifestResolver};
pub use registry::CompletionRegistry;
pub use selectors::{
    FinalMessageSelector, ReplySelector, SessionReplySelector, StructuredResultSelector,
};
pub use snapshot_store::CompletionSnapshotStore;
pub use sources::CompletionSource;
pub use tracker::{CompletionTrackerService, CompletionTrackerView};
pub use utils::{fingerprint_text, first_non_empty, parse_timestamp, seconds_between, utc_now_iso};

// Re-export detector concrete types for callers that need to construct them directly.
pub use detectors::{
    AnchoredSessionStabilityDetector, BaseDetector, CompletionDetector, ProtocolTurnDetector,
    SessionBoundaryDetector, StructuredResultDetector, TerminalTextQuietDetector,
};
