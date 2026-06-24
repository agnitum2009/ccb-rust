//! Mirrors Python `lib/ccbrd/reload_runtime_mount_models.py`.
//! 1:1 file alignment stub.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditiveRuntimeMountResult {
    pub status: String,
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<HashMap<String, serde_json::Value>>,
}

impl AdditiveRuntimeMountResult {
    pub fn blocked(reason: &str, message: &str) -> Self {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("reason".to_string(), serde_json::json!(reason));
        diagnostics.insert("message".to_string(), serde_json::json!(message));

        Self {
            status: "blocked".to_string(),
            stage: "mount".to_string(),
            diagnostics: Some(diagnostics),
        }
    }

    pub fn success() -> Self {
        Self {
            status: "success".to_string(),
            stage: "mount".to_string(),
            diagnostics: None,
        }
    }
}
