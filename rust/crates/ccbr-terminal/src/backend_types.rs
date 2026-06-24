//! Mirrors Python lib/terminal_runtime/backend_types.py
// TODO: translate from Python

/// Abstract terminal backend trait
pub trait TerminalBackend: Send + Sync {
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn is_alive(&self, pane_id: &str) -> Result<bool, Box<dyn std::error::Error>>;
    fn kill_pane(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn activate(&self, pane_id: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error>>;
}

// Re-export from backend module
pub use crate::backend::TerminalBackend as _TerminalBackend;
