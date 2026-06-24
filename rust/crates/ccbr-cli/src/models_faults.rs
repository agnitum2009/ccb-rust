//! Mirrors Python `lib/cli/models_faults.py`.
//!
//! Fault-injection CLI command models. 1:1 alignment with Python dataclasses.

use serde::{Deserialize, Serialize};

/// Parsed `fault-list` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFaultListCommand {
    pub project: Option<String>,
    #[serde(default = "default_fault_list_kind")]
    pub kind: String,
}

fn default_fault_list_kind() -> String {
    "fault-list".into()
}

impl ParsedFaultListCommand {
    pub fn new(project: Option<String>) -> Self {
        Self {
            project,
            kind: "fault-list".into(),
        }
    }
}

/// Parsed `fault-arm` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFaultArmCommand {
    pub project: Option<String>,
    pub agent_name: String,
    pub task_id: String,
    pub reason: String,
    pub count: i64,
    pub error_message: String,
    #[serde(default = "default_fault_arm_kind")]
    pub kind: String,
}

fn default_fault_arm_kind() -> String {
    "fault-arm".into()
}

impl ParsedFaultArmCommand {
    pub fn new(
        project: Option<String>,
        agent_name: String,
        task_id: String,
        reason: String,
        count: i64,
        error_message: String,
    ) -> Self {
        Self {
            project,
            agent_name,
            task_id,
            reason,
            count,
            error_message,
            kind: "fault-arm".into(),
        }
    }
}

/// Parsed `fault-clear` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFaultClearCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default = "default_fault_clear_kind")]
    pub kind: String,
}

fn default_fault_clear_kind() -> String {
    "fault-clear".into()
}

impl ParsedFaultClearCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            kind: "fault-clear".into(),
        }
    }
}
