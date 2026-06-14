//! Mirrors Python `lib/provider_hooks/settings_runtime/` module
//!
//! This module consolidates Python files:
//! - `settings_runtime/claude.py`
//! - `settings_runtime/command.py`
//! - `settings_runtime/common.py`
//! - `settings_runtime/gemini.py`
//! - `settings_runtime/install.py`
//!
//! In the Rust implementation, most functionality is consolidated into `settings.rs`.

// Re-export all items from settings.rs for Python compatibility
pub use crate::settings::{
    build_activity_hook_command, build_hook_command, claude_event_has_command,
    claude_hook_home_layout, gemini_event_has_command, install_claude_activity_hooks,
    install_claude_hooks, install_gemini_hooks, install_workspace_activity_hooks,
    install_workspace_completion_hooks, load_json, save_json, trust_claude_workspace,
    trust_gemini_workspace, workspace_key,
};

// Specific implementation for `settings_runtime/command.py`
// Command building functions are already in settings.rs
pub mod command {
    //! Mirrors Python `lib/provider_hooks/settings_runtime/command.py`

    // Re-export command building functions from settings.rs
    pub use crate::settings::{build_activity_hook_command, build_hook_command};
}

// Specific implementation for `settings_runtime/common.py`
// Common utility functions are already in settings.rs
pub mod common {
    //! Mirrors Python `lib/provider_hooks/settings_runtime/common.py`

    // Re-export common utility functions from settings.rs
    pub use crate::settings::{load_json, save_json, workspace_key};
}

// Specific implementation for `settings_runtime/claude.py`
// Claude-specific hook functions are already in settings.rs
pub mod claude {
    //! Mirrors Python `lib/provider_hooks/settings_runtime/claude.py`

    use camino::Utf8Path;
    use serde_json::Map;

    // Re-export Claude-specific functions from settings.rs
    pub use crate::settings::{
        claude_event_has_command, claude_hook_home_layout, install_claude_activity_hooks,
        install_claude_hooks, trust_claude_workspace,
    };

    /// Re-export for Python compatibility
    /// In Python this loads JSON from settings path
    pub fn load_settings(settings_path: &Utf8Path) -> Result<Map<String, serde_json::Value>, crate::HookError> {
        crate::settings::load_json(settings_path)
    }
}

// Specific implementation for `settings_runtime/gemini.py`
// Gemini-specific hook functions are already in settings.rs
pub mod gemini {
    //! Mirrors Python `lib/provider_hooks/settings_runtime/gemini.py`

    // Re-export Gemini-specific functions from settings.rs
    pub use crate::settings::{gemini_event_has_command, install_gemini_hooks, trust_gemini_workspace};
}

// Specific implementation for `settings_runtime/install.py`
// Hook installation functions are already in settings.rs
pub mod install {
    //! Mirrors Python `lib/provider_hooks/settings_runtime/install.py`

    // Re-export installation functions from settings.rs
    pub use crate::settings::{install_workspace_activity_hooks, install_workspace_completion_hooks};
}
