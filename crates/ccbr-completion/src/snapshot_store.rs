use serde_json::Map;

use crate::error::Result;
use crate::models::CompletionSnapshot;

/// Persistent store for completion snapshots.
pub struct CompletionSnapshotStore {
    layout: ccbr_storage::paths::PathLayout,
    store: ccbr_storage::json::JsonStore,
}

impl CompletionSnapshotStore {
    pub fn new(
        layout: ccbr_storage::paths::PathLayout,
        store: Option<ccbr_storage::json::JsonStore>,
    ) -> Self {
        Self {
            layout,
            store: store.unwrap_or_default(),
        }
    }

    pub fn load(&self, job_id: &str) -> Result<Option<CompletionSnapshot>> {
        let path = self.layout.snapshot_path(job_id);
        if !path.exists() {
            return Ok(None);
        }
        let value: serde_json::Value = self.store.load(&path)?;
        let record = value.as_object().ok_or_else(|| {
            crate::error::CompletionError::Validation(
                "snapshot file must contain a JSON object".into(),
            )
        })?;
        Ok(Some(completion_snapshot_from_record(record)?))
    }

    pub fn save(&self, snapshot: &CompletionSnapshot) -> Result<()> {
        let path = self.layout.snapshot_path(&snapshot.job_id);
        self.store.save(&path, &snapshot.to_record())?;
        Ok(())
    }
}

fn validate_record(record: &Map<String, serde_json::Value>, expected_type: &str) -> Result<()> {
    if record
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        != Some(crate::models::SCHEMA_VERSION)
    {
        return Err(crate::error::CompletionError::Validation(format!(
            "schema_version must be {}",
            crate::models::SCHEMA_VERSION
        )));
    }
    if record.get("record_type").and_then(|v| v.as_str()) != Some(expected_type) {
        return Err(crate::error::CompletionError::Validation(format!(
            "record_type must be {expected_type:?}"
        )));
    }
    Ok(())
}

fn completion_snapshot_from_record(
    record: &Map<String, serde_json::Value>,
) -> Result<CompletionSnapshot> {
    validate_record(record, "completion_snapshot")?;
    CompletionSnapshot::from_record(record)
}
