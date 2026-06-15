//! Mirrors Python `lib/ccbd/services/dispatcher_runtime/artifact_maintenance.py`.
//! Periodic text artifact sweep.

pub const TEXT_ARTIFACT_SWEEP_INTERVAL_S: f64 = 300.0;

/// Sweep expired text artifacts if the sweep interval has elapsed.
/// Returns the number of artifacts removed.
pub fn sweep_text_artifacts_if_due(
    layout: &ccb_storage::paths::PathLayout,
    last_sweep_at: &mut Option<f64>,
    now: f64,
) -> usize {
    if let Some(last) = last_sweep_at {
        if now - *last < TEXT_ARTIFACT_SWEEP_INTERVAL_S {
            return 0;
        }
    }
    let completion_dir = layout.ccbd_dir().join("completion");
    let mut removed = 0;
    if completion_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&completion_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let age = now
                            - modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64();
                        if age > TEXT_ARTIFACT_SWEEP_INTERVAL_S * 6.0 {
                            let _ = std::fs::remove_file(&path);
                            removed += 1;
                        }
                    }
                }
            }
        }
    }
    *last_sweep_at = Some(now);
    removed
}
