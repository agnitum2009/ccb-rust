//! Mirrors Python `lib/cli/services/diagnostics_runtime/models.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticBundleEntry {
    pub category: String,
    pub source_path: String,
    pub archive_path: String,
    pub status: String,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub byte_count: usize,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticBundleSummary {
    pub project_root: String,
    pub project_id: String,
    pub bundle_id: String,
    pub bundle_path: String,
    pub file_count: usize,
    pub included_count: usize,
    pub missing_count: usize,
    pub truncated_count: usize,
    #[serde(default)]
    pub doctor_error: Option<String>,
}
