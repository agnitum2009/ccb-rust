//! Mirrors Python `lib/ccbd/services/project_namespace_runtime/records.py`.
//! 1:1 file alignment stub.

/// Normalize layout signature to canonical form
pub fn normalized_layout_signature(signature: Option<&str>) -> Option<String> {
    signature
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
