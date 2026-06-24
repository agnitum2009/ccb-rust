//! Mirrors Python `lib/terminal_runtime/backend_selection.py`.

use std::collections::HashMap;

use crate::backend::TmuxBackend;
use crate::layouts::LayoutResult;
use crate::registry::UserSession;

/// Select and cache a tmux backend based on terminal detection.
pub struct TerminalBackendSelection {
    cached: Option<TmuxBackend>,
    detect_fn: Box<dyn Fn() -> Option<String>>,
    factory: Box<dyn Fn() -> TmuxBackend>,
}

impl Default for TerminalBackendSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalBackendSelection {
    pub fn new() -> Self {
        Self::with_deps(crate::detect::detect_terminal, || {
            TmuxBackend::new(None, None)
        })
    }

    pub fn with_deps<D, F>(detect_fn: D, factory: F) -> Self
    where
        D: Fn() -> Option<String> + 'static,
        F: Fn() -> TmuxBackend + 'static,
    {
        Self {
            cached: None,
            detect_fn: Box::new(detect_fn),
            factory: Box::new(factory),
        }
    }

    /// Return a cached tmux backend only when terminal detection reports tmux.
    pub fn get_backend(&mut self) -> Option<&TmuxBackend> {
        if self.cached.is_none() {
            let terminal = (self.detect_fn)()?;
            if terminal == "tmux" {
                self.cached = Some((self.factory)());
            }
        }
        self.cached.as_ref()
    }

    /// Resolve a backend from session data using its tmux socket settings.
    pub fn get_backend_for_session(&self, session: &UserSession) -> TmuxBackend {
        TmuxBackend::new(
            session.tmux_socket_name.clone(),
            session.tmux_socket_path.clone(),
        )
    }

    /// Extract the pane id from session data, preferring `pane_id` over `tmux_session`.
    pub fn get_pane_id_from_session(&self, session: &UserSession) -> Option<String> {
        session
            .pane_id
            .clone()
            .or_else(|| session.tmux_session.clone())
            .filter(|s| !s.is_empty())
    }
}

type LayoutFn =
    Box<dyn Fn(&[String], &str, &TmuxBackend, &str, bool) -> anyhow::Result<LayoutResult>>;

/// High-level layout service that delegates to the runtime auto-layout function.
pub struct TerminalLayoutService {
    backend_factory: Box<dyn Fn() -> TmuxBackend>,
    session_name_fn: Box<dyn Fn(&str) -> String>,
    env: Option<HashMap<String, String>>,
    layout_fn: LayoutFn,
}

impl TerminalLayoutService {
    pub fn new<F, S>(
        backend_factory: F,
        session_name_fn: S,
        env: Option<HashMap<String, String>>,
    ) -> Self
    where
        F: Fn() -> TmuxBackend + 'static,
        S: Fn(&str) -> String + 'static,
    {
        let layout_fn: LayoutFn = Box::new(|providers, cwd, backend, session_name, inside_tmux| {
            crate::layouts::create_tmux_auto_layout(
                providers,
                cwd,
                backend,
                None,
                None,
                50,
                false,
                "",
                Some(session_name),
                inside_tmux,
            )
        });
        Self {
            backend_factory: Box::new(backend_factory),
            session_name_fn: Box::new(session_name_fn),
            env,
            layout_fn,
        }
    }

    pub fn with_layout_fn(mut self, layout_fn: LayoutFn) -> Self {
        self.layout_fn = layout_fn;
        self
    }

    pub fn create_auto_layout(
        &self,
        providers: Vec<String>,
        cwd: &str,
    ) -> anyhow::Result<LayoutResult> {
        let backend = (self.backend_factory)();
        let session_name = (self.session_name_fn)(cwd);
        let inside_tmux = self
            .env
            .as_ref()
            .and_then(|e| e.get("TMUX"))
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        (self.layout_fn)(&providers, cwd, &backend, &session_name, inside_tmux)
    }
}
