//! Mirrors Python `lib/cli/services/fault.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultListSummary {
    pub project_id: String,
    pub rule_count: usize,
    #[serde(default)]
    pub rules: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultArmSummary {
    pub project_id: String,
    pub rule_id: String,
    pub agent_name: String,
    pub task_id: String,
    pub reason: String,
    pub remaining_count: usize,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaultClearSummary {
    pub project_id: String,
    pub target: String,
    pub cleared_count: usize,
    #[serde(default)]
    pub cleared_rule_ids: Vec<String>,
}

// TODO: align `list_fault_rules` / `arm_fault_rule` / `clear_fault_rules` once fault injection ready.
