//! Mirrors Python `lib/ccbd/services/health_assessment/tmux.py`.

use std::collections::HashMap;
use std::sync::Arc;

use ccb_provider_core::session_binding::{Session, SessionBackend};

use super::tmux_runtime::backend::TmuxBackend;
use super::tmux_runtime::namespace::{PaneRecord, TmuxNamespaceBackend};

/// Extract the tmux backend from a session, if any.
///
/// Mirrors Python `session_backend(session)`.
pub fn session_backend(session: &Session) -> Option<SessionBackendAdapter> {
    session.backend.clone().map(SessionBackendAdapter)
}

/// Adapter that exposes a provider-core `SessionBackend` through the health
/// assessment `TmuxBackend` and `TmuxNamespaceBackend` traits.
#[derive(Clone)]
pub struct SessionBackendAdapter(pub Arc<dyn SessionBackend>);

impl TmuxBackend for SessionBackendAdapter {
    fn pane_exists(&self, pane_id: &str) -> bool {
        self.0.pane_exists(pane_id)
    }

    fn is_tmux_pane_alive(&self, pane_id: &str) -> Option<bool> {
        Some(self.0.is_tmux_pane_alive(pane_id))
    }

    fn is_alive(&self, pane_id: &str) -> Option<bool> {
        Some(self.0.is_alive(pane_id))
    }
}

impl TmuxNamespaceBackend for SessionBackendAdapter {
    fn backend_socket_matches(&self, tmux_socket_path: Option<&str>) -> bool {
        let Some(target) = tmux_socket_path else {
            return false;
        };
        self.0
            .socket_path()
            .as_deref()
            .map(|s| s == target)
            .unwrap_or(false)
            || self
                .0
                .socket_name()
                .as_deref()
                .map(|s| s == target)
                .unwrap_or(false)
    }

    fn inspect_project_namespace_pane(&self, pane_id: &str) -> Option<Box<dyn PaneRecord>> {
        let options = self.0.describe_pane(
            pane_id,
            &[
                "@ccb_project_id".to_string(),
                "@ccb_role".to_string(),
                "@ccb_slot".to_string(),
                "@ccb_window".to_string(),
                "@ccb_managed_by".to_string(),
            ],
        )?;
        Some(Box::new(SessionPaneRecord { options }))
    }
}

struct SessionPaneRecord {
    options: HashMap<String, String>,
}

impl PaneRecord for SessionPaneRecord {
    fn window_id(&self) -> Option<&str> {
        None
    }

    fn window_name(&self) -> Option<&str> {
        self.options.get("@ccb_window").map(|s| s.as_str())
    }

    fn ccb_window(&self) -> Option<&str> {
        self.options.get("@ccb_window").map(|s| s.as_str())
    }

    fn matches(
        &self,
        _tmux_session_name: &str,
        project_id: &str,
        role: &str,
        slot_key: Option<&str>,
        window_name: Option<&str>,
        managed_by: &str,
    ) -> bool {
        self.options.get("@ccb_project_id").map(|s| s.as_str()) == Some(project_id)
            && self.options.get("@ccb_role").map(|s| s.as_str()) == Some(role)
            && slot_key
                .is_none_or(|key| self.options.get("@ccb_slot").map(|s| s.as_str()) == Some(key))
            && window_name.is_none_or(|name| {
                self.options.get("@ccb_window").map(|s| s.as_str()) == Some(name)
            })
            && self.options.get("@ccb_managed_by").map(|s| s.as_str()) == Some(managed_by)
    }
}
