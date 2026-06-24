//! Mirrors Python `lib/ccbrd/services/health_assessment/models.py`.

use std::path::Path;

/// A binding that can load a provider session for an agent.
///
/// Mirrors the callable attributes on Python's provider session binding
/// objects.
pub trait SessionBinding: std::fmt::Debug {
    fn provider(&self) -> &str;
    fn session_id_attr(&self) -> &str;
    fn session_path_attr(&self) -> &str;
    fn load_session(
        &self,
        root: &Path,
        instance: Option<&str>,
    ) -> Option<ccbr_provider_core::session_binding::Session>;
    fn clone_box(&self) -> Box<dyn SessionBinding>;
}

/// Assessment of a provider's tmux pane, mirroring Python
/// `ProviderPaneAssessment`.
#[derive(Debug)]
pub struct ProviderPaneAssessment {
    pub binding: Option<Box<dyn SessionBinding>>,
    pub session: Option<ccbr_provider_core::session_binding::Session>,
    pub terminal: Option<String>,
    pub pane_state: Option<String>,
    pub health: String,
}
