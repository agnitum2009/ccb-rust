use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Session handle representing a loaded provider session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHandle {
    pub provider: String,
    pub session_id: Option<String>,
    pub session_path: Option<PathBuf>,
    pub state: SessionState,
}

/// State of a provider session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SessionState {
    #[default]
    Fresh,
    Resumed,
    Lost,
    Failed,
}

/// Trait for loading and managing provider sessions.
pub trait SessionManager: Send + Sync {
    fn provider(&self) -> &str;

    fn load_session(
        &self,
        path: &std::path::Path,
        session_key: Option<&str>,
    ) -> Option<SessionHandle>;

    fn save_session(&self, handle: &SessionHandle, path: &std::path::Path) -> std::io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default() {
        assert_eq!(SessionState::default(), SessionState::Fresh);
    }

    #[test]
    fn test_session_handle_serde() {
        let handle = SessionHandle {
            provider: "claude".into(),
            session_id: Some("sess-123".into()),
            session_path: None,
            state: SessionState::Resumed,
        };
        let json = serde_json::to_string(&handle).unwrap();
        let deserialized: SessionHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider, "claude");
        assert_eq!(deserialized.state, SessionState::Resumed);
    }
}
