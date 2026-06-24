//! Mirrors Python `lib/cli/services/config_validate.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigValidationSummary {
    pub project_root: String,
    pub project_id: String,
    pub source: Option<String>,
    pub source_kind: String,
    pub used_builtin_default: bool,
    pub default_agents: Vec<String>,
    pub agent_names: Vec<String>,
    pub cmd_enabled: bool,
    pub layout_spec: String,
    #[serde(default)]
    pub style_warnings: Vec<String>,
}

// TODO: align `validate_config_context` with Python once config loader is ready.
