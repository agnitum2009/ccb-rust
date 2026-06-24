//! Mirrors Python `lib/ccbd/reload_transaction_models.py`.

use serde::{Deserialize, Serialize};

/// Result of publishing a reload transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadPublishTransactionResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_graph_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_config_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_patch: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_mount: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<serde_json::Value>,
    pub diagnostics: serde_json::Value,
}
