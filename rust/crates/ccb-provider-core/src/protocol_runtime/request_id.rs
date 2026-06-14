use std::sync::atomic::{AtomicU64, Ordering};

static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique request ID.
///
/// Format mirrors the Python implementation:
/// `YYYYMMDD-HHMMSS-fff-PID-COUNTER`.
pub fn make_req_id() -> String {
    let now = chrono::Local::now();
    let ms = now.timestamp_subsec_millis();
    let count = REQ_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!(
        "{}-{ms:03}-{}-{count}",
        now.format("%Y%m%d-%H%M%S"),
        std::process::id()
    )
}

/// Resolve a request anchor for a job.
///
/// If `job_id` is empty, the optional `fallback_factory` is tried. If no
/// non-empty anchor can be produced, this function panics (matching Python's
/// `ValueError`).
pub fn request_anchor_for_job(
    job_id: Option<&str>,
    fallback_factory: Option<&dyn Fn() -> Option<String>>,
) -> String {
    if let Some(anchor) = job_id.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return anchor.to_string();
    }
    if let Some(factory) = fallback_factory {
        if let Some(fallback) = factory()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return fallback;
        }
    }
    panic!("request anchor cannot be empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_req_id_format() {
        let id = make_req_id();
        let re = regex::Regex::new(r"^\d{8}-\d{6}-\d{3}-\d+-\d+$").unwrap();
        assert!(
            re.is_match(&id),
            "request id {id} did not match expected pattern"
        );
    }

    #[test]
    fn test_make_req_id_increments_counter() {
        let a = make_req_id();
        let b = make_req_id();
        // Timestamps may match, but the trailing counters must differ.
        assert_ne!(a, b);
    }

    #[test]
    fn test_request_anchor_for_job_uses_job_id() {
        assert_eq!(
            request_anchor_for_job(Some("job-123"), None),
            "job-123".to_string()
        );
    }

    #[test]
    fn test_request_anchor_for_job_uses_fallback() {
        assert_eq!(
            request_anchor_for_job(None, Some(&|| Some("fallback-1".to_string()))),
            "fallback-1".to_string()
        );
    }

    #[test]
    fn test_request_anchor_for_job_falls_back_from_whitespace() {
        assert_eq!(
            request_anchor_for_job(Some("   "), Some(&|| Some("fallback-2".to_string()))),
            "fallback-2".to_string()
        );
    }

    #[test]
    #[should_panic(expected = "request anchor cannot be empty")]
    fn test_request_anchor_for_job_empty_panics() {
        request_anchor_for_job(None, None);
    }
}
