//! Mirrors Python `lib/cli/models_start.py`.
//!
//! Operational CLI command models (start, kill, clear, restart, etc.).
//! 1:1 alignment with Python dataclasses.

use serde::{Deserialize, Serialize};

/// Parsed `start` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedStartCommand {
    pub project: Option<String>,
    pub agent_names: Vec<String>,
    pub restore: bool,
    pub auto_permission: bool,
    #[serde(default)]
    pub reset_context: bool,
    #[serde(default = "default_start_kind")]
    pub kind: String,
}

fn default_start_kind() -> String {
    "start".into()
}

impl ParsedStartCommand {
    pub fn new(
        project: Option<String>,
        agent_names: Vec<String>,
        restore: bool,
        auto_permission: bool,
    ) -> Self {
        Self {
            project,
            agent_names,
            restore,
            auto_permission,
            reset_context: false,
            kind: "start".into(),
        }
    }
}

/// Parsed `kill` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedKillCommand {
    pub project: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_kill_kind")]
    pub kind: String,
}

fn default_kill_kind() -> String {
    "kill".into()
}

impl ParsedKillCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            force: false,
            kind: "kill".into(),
        }
    }
}

/// Parsed `clear` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedClearCommand {
    pub project: Option<String>,
    #[serde(default)]
    pub agent_names: Vec<String>,
    #[serde(default = "default_clear_kind")]
    pub kind: String,
}

fn default_clear_kind() -> String {
    "clear".into()
}

impl ParsedClearCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            agent_names: Vec::new(),
            kind: "clear".into(),
        }
    }
}

/// Parsed `restart` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedRestartCommand {
    pub project: Option<String>,
    pub agent_name: String,
    #[serde(default = "default_restart_kind")]
    pub kind: String,
}

fn default_restart_kind() -> String {
    "restart".into()
}

impl ParsedRestartCommand {
    pub fn new(project: Option<String>, agent_name: String) -> Self {
        Self {
            project,
            agent_name,
            kind: "restart".into(),
        }
    }
}

/// Parsed `maintenance` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedMaintenanceCommand {
    pub project: Option<String>,
    #[serde(default = "default_maintenance_action")]
    pub action: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_maintenance_kind")]
    pub kind: String,
}

fn default_maintenance_action() -> String {
    "status".into()
}
fn default_maintenance_kind() -> String {
    "maintenance".into()
}

impl ParsedMaintenanceCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            action: "status".into(),
            args: Vec::new(),
            kind: "maintenance".into(),
        }
    }
}

/// Parsed `mobile` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedMobileCommand {
    pub project: Option<String>,
    pub action: String,
    #[serde(default)]
    pub listen: Option<String>,
    #[serde(default)]
    pub public_url: Option<String>,
    #[serde(default)]
    pub route_provider: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default = "default_mobile_kind")]
    pub kind: String,
}

fn default_mobile_kind() -> String {
    "mobile".into()
}

impl ParsedMobileCommand {
    pub fn new(project: Option<String>, action: String) -> Self {
        Self {
            project,
            action,
            listen: None,
            public_url: None,
            route_provider: None,
            device_id: None,
            kind: "mobile".into(),
        }
    }
}

/// Parsed `cleanup` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedCleanupCommand {
    pub project: Option<String>,
    #[serde(default = "default_cleanup_kind")]
    pub kind: String,
}

fn default_cleanup_kind() -> String {
    "cleanup".into()
}

impl ParsedCleanupCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            kind: "cleanup".into(),
        }
    }
}

/// Parsed `ps` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPsCommand {
    pub project: Option<String>,
    #[serde(default)]
    pub alive_only: bool,
    #[serde(default = "default_ps_kind")]
    pub kind: String,
}

fn default_ps_kind() -> String {
    "ps".into()
}

impl ParsedPsCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            alive_only: false,
            kind: "ps".into(),
        }
    }
}

/// Parsed `config-validate` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedConfigValidateCommand {
    pub project: Option<String>,
    #[serde(default = "default_config_validate_kind")]
    pub kind: String,
}

fn default_config_validate_kind() -> String {
    "config-validate".into()
}

impl ParsedConfigValidateCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            kind: "config-validate".into(),
        }
    }
}

/// Parsed `reload` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedReloadCommand {
    pub project: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_reload_kind")]
    pub kind: String,
}

fn default_reload_kind() -> String {
    "reload".into()
}

impl ParsedReloadCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            dry_run: false,
            kind: "reload".into(),
        }
    }
}

/// Parsed `doctor` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedDoctorCommand {
    pub project: Option<String>,
    #[serde(default)]
    pub bundle: bool,
    pub output_path: Option<String>,
    #[serde(default)]
    pub storage: bool,
    #[serde(default)]
    pub json_output: bool,
    #[serde(default = "default_doctor_kind")]
    pub kind: String,
}

fn default_doctor_kind() -> String {
    "doctor".into()
}

impl ParsedDoctorCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            bundle: false,
            output_path: None,
            storage: false,
            json_output: false,
            kind: "doctor".into(),
        }
    }
}

/// Parsed `logs` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedLogsCommand {
    pub project: Option<String>,
    pub agent_name: String,
    #[serde(default = "default_logs_kind")]
    pub kind: String,
}

fn default_logs_kind() -> String {
    "logs".into()
}

impl ParsedLogsCommand {
    pub fn new(project: Option<String>, agent_name: String) -> Self {
        Self {
            project,
            agent_name,
            kind: "logs".into(),
        }
    }
}

/// Parsed `ping` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPingCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default = "default_ping_kind")]
    pub kind: String,
}

fn default_ping_kind() -> String {
    "ping".into()
}

impl ParsedPingCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            kind: "ping".into(),
        }
    }
}
