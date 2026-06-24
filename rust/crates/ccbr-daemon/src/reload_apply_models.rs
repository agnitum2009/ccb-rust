//! Mirrors Python `lib/ccbd/reload_apply_models.py`.
//! 1:1 file alignment stub.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of an additive reload apply operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditiveReloadApplyResult {
    pub status: String,
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_patch: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_mount: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_transaction: Option<serde_json::Value>,
    #[serde(default)]
    pub diagnostics: HashMap<String, serde_json::Value>,
}

impl AdditiveReloadApplyResult {
    pub fn to_record(&self) -> HashMap<String, serde_json::Value> {
        let mut record = HashMap::new();
        record.insert("status".to_string(), serde_json::json!(self.status.clone()));
        record.insert("stage".to_string(), serde_json::json!(self.stage.clone()));
        if let Some(plan_class) = &self.plan_class {
            record.insert("plan_class".to_string(), serde_json::json!(plan_class));
        }
        if let Some(old_graph_version) = &self.old_graph_version {
            record.insert(
                "old_graph_version".to_string(),
                serde_json::json!(old_graph_version),
            );
        }
        if let Some(target_graph_version) = &self.target_graph_version {
            record.insert(
                "target_graph_version".to_string(),
                serde_json::json!(target_graph_version),
            );
        }
        if let Some(published_graph_version) = &self.published_graph_version {
            record.insert(
                "published_graph_version".to_string(),
                serde_json::json!(published_graph_version),
            );
        }
        if let Some(old_config_signature) = &self.old_config_signature {
            record.insert(
                "old_config_signature".to_string(),
                serde_json::json!(old_config_signature),
            );
        }
        if let Some(new_config_signature) = &self.new_config_signature {
            record.insert(
                "new_config_signature".to_string(),
                serde_json::json!(new_config_signature),
            );
        }
        if let Some(plan) = &self.plan {
            record.insert("plan".to_string(), serde_json::json!(plan));
        }
        if let Some(namespace_patch) = &self.namespace_patch {
            record.insert(
                "namespace_patch".to_string(),
                serde_json::json!(namespace_patch),
            );
        }
        if let Some(runtime_mount) = &self.runtime_mount {
            record.insert(
                "runtime_mount".to_string(),
                serde_json::json!(runtime_mount),
            );
        }
        if let Some(publish_transaction) = &self.publish_transaction {
            record.insert(
                "publish_transaction".to_string(),
                serde_json::json!(publish_transaction),
            );
        }
        record.insert(
            "diagnostics".to_string(),
            serde_json::json!(self.diagnostics.clone()),
        );
        record
    }
}

/// In-memory representation of the published service graph.
#[derive(Debug, Clone)]
pub struct ServiceGraph {
    pub version: Option<String>,
    pub config: ccbr_agents::models::ProjectConfig,
    pub config_identity: serde_json::Value,
    pub config_signature: String,
}
