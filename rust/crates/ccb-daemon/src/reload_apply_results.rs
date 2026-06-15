//! Mirrors Python `lib/ccbd/reload_apply_results.py`.
//! 1:1 file alignment stub.

use serde_json::Value;
use std::collections::HashMap;

/// Generate a stage result
pub fn stage_result(
    status: &str,
    stage: &str,
    old_graph: &dyn GraphVersion,
    target_graph: &dyn GraphVersion,
    plan: &HashMap<String, Value>,
    namespace_patch: Option<&NamespacePatch>,
    runtime_mount: Option<&RuntimeMount>,
    publish_transaction: Option<&PublishTransaction>,
    diagnostics: HashMap<String, String>,
) -> AdditiveReloadApplyResult {
    AdditiveReloadApplyResult {
        status: status.to_string(),
        stage: stage.to_string(),
        plan_class: plan
            .get("plan_class")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        old_graph_version: old_graph.version(),
        target_graph_version: target_graph.version(),
        published_graph_version: None,
        old_config_signature: graph_signature(old_graph),
        new_config_signature: graph_signature(target_graph),
        plan: plan.clone(),
        namespace_patch: namespace_patch.and_then(|p| p.to_record()),
        runtime_mount: runtime_mount.and_then(|m| m.to_record()),
        publish_transaction: publish_transaction.and_then(|t| t.to_record()),
        diagnostics,
    }
}

/// Generate a noop result
pub fn noop_result(
    old_graph: &dyn GraphVersion,
    plan: &HashMap<String, Value>,
) -> AdditiveReloadApplyResult {
    let mut diagnostics = HashMap::new();
    diagnostics.insert("reason".to_string(), "no_change".to_string());
    diagnostics.insert(
        "message".to_string(),
        "config identity and presentation fields are unchanged".to_string(),
    );

    for (key, value) in not_published_diagnostics() {
        diagnostics.insert(key, value);
    }

    AdditiveReloadApplyResult {
        status: "noop".to_string(),
        stage: "no_op".to_string(),
        plan_class: plan
            .get("plan_class")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        old_graph_version: old_graph.version(),
        target_graph_version: None,
        published_graph_version: None,
        old_config_signature: graph_signature(old_graph),
        new_config_signature: plan
            .get("new_config_signature")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        plan: plan.clone(),
        namespace_patch: None,
        runtime_mount: None,
        publish_transaction: None,
        diagnostics,
    }
}

/// Extract namespace residue from patch
pub fn namespace_residue(namespace_patch: &NamespacePatch) -> HashMap<String, String> {
    let mut residue = HashMap::new();
    if let Some(record) = namespace_patch.to_record() {
        residue.insert(
            "partial".to_string(),
            record
                .get("partial")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .to_string(),
        );
    }
    residue
}

/// Extract runtime residue from mount
pub fn runtime_residue(runtime_mount: &RuntimeMount) -> HashMap<String, String> {
    let mut residue = HashMap::new();
    if let Some(record) = runtime_mount.to_record() {
        residue.insert(
            "partial".to_string(),
            record
                .get("partial")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .to_string(),
        );
    }
    residue
}

/// Extract status from a value
pub fn status_of(value: &dyn StatusProvider) -> String {
    value.status().trim().to_string()
}

/// Extract reason from a value
pub fn reason_of(value: &dyn RecordProvider, fallback: String) -> String {
    if let Some(record) = value.to_record() {
        if let Some(diag) = record.get("diagnostics").and_then(|v| v.as_object()) {
            if let Some(reason) = diag.get("reason").and_then(|v| v.as_str()) {
                return reason.to_string();
            }
        }
    }
    fallback
}

/// Extract message from a value
pub fn message_of(value: &dyn RecordProvider) -> Option<String> {
    if let Some(record) = value.to_record() {
        if let Some(diag) = record.get("diagnostics").and_then(|v| v.as_object()) {
            if let Some(message) = diag.get("message").and_then(|v| v.as_str()) {
                let trimmed = message.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Generate not published diagnostics
pub fn not_published_diagnostics() -> HashMap<String, String> {
    let mut diagnostics = HashMap::new();
    diagnostics.insert("graph_published".to_string(), "false".to_string());
    diagnostics.insert(
        "lease_or_lifecycle_written".to_string(),
        "false".to_string(),
    );
    diagnostics.insert("config_watch_started".to_string(), "false".to_string());
    diagnostics.insert(
        "unload_or_replace_executed".to_string(),
        "false".to_string(),
    );
    diagnostics
}

/// Generate graph signature
pub fn graph_signature(graph: &dyn GraphVersion) -> String {
    graph.version().unwrap_or("unknown".to_string())
}

pub trait GraphVersion {
    fn version(&self) -> Option<String>;
}
pub trait StatusProvider {
    fn status(&self) -> String;
}
pub trait RecordProvider {
    fn to_record(&self) -> Option<Value>;
}

#[derive(Debug, Clone)]
pub struct AdditiveReloadApplyResult {
    pub status: String,
    pub stage: String,
    pub plan_class: String,
    pub old_graph_version: Option<String>,
    pub target_graph_version: Option<String>,
    pub published_graph_version: Option<String>,
    pub old_config_signature: String,
    pub new_config_signature: String,
    pub plan: HashMap<String, Value>,
    pub namespace_patch: Option<Value>,
    pub runtime_mount: Option<Value>,
    pub publish_transaction: Option<Value>,
    pub diagnostics: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct NamespacePatch {
    pub status: String,
}
impl StatusProvider for NamespacePatch {
    fn status(&self) -> String {
        self.status.clone()
    }
}
impl RecordProvider for NamespacePatch {
    fn to_record(&self) -> Option<Value> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeMount {
    pub status: String,
}
impl StatusProvider for RuntimeMount {
    fn status(&self) -> String {
        self.status.clone()
    }
}
impl RecordProvider for RuntimeMount {
    fn to_record(&self) -> Option<Value> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct PublishTransaction {
    pub status: String,
}
impl StatusProvider for PublishTransaction {
    fn status(&self) -> String {
        self.status.clone()
    }
}
impl RecordProvider for PublishTransaction {
    fn to_record(&self) -> Option<Value> {
        None
    }
}
