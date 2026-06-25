pub fn compute_backoff(restart_count: u32, base_seconds: u32, max_seconds: u32) -> u32 {
    if restart_count == 0 {
        return 0;
    }
    let shift = restart_count.saturating_sub(1).min(30);
    let backoff = base_seconds.saturating_mul(1u32.wrapping_shl(shift));
    std::cmp::min(backoff, max_seconds)
}

pub fn should_attempt_restart(restart_count: u32, max_retries: u32) -> bool {
    restart_count < max_retries
}

/// Returns true when not enough time has passed since the last restart attempt
/// for the configured backoff window.
pub fn is_in_backoff_window(
    last_restart_at: Option<&str>,
    backoff_seconds: u32,
    now: &chrono::DateTime<chrono::Utc>,
) -> bool {
    if backoff_seconds == 0 {
        return false;
    }
    let Some(last) = last_restart_at else {
        return false;
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last) else {
        return false;
    };
    let elapsed = now.signed_duration_since(dt.with_timezone(&chrono::Utc));
    let remaining = backoff_seconds as i64 - elapsed.num_seconds();
    remaining > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_progression() {
        assert_eq!(compute_backoff(0, 1, 300), 0);
        assert_eq!(compute_backoff(1, 1, 300), 1);
        assert_eq!(compute_backoff(2, 1, 300), 2);
        assert_eq!(compute_backoff(3, 1, 300), 4);
        assert_eq!(compute_backoff(10, 1, 300), 300);
    }

    #[test]
    fn test_is_in_backoff_window() {
        let now = chrono::Utc::now();
        let recent = (now - chrono::Duration::seconds(2)).to_rfc3339();
        let old = (now - chrono::Duration::seconds(10)).to_rfc3339();
        assert!(is_in_backoff_window(Some(&recent), 5, &now));
        assert!(!is_in_backoff_window(Some(&old), 5, &now));
        assert!(!is_in_backoff_window(None, 5, &now));
        assert!(!is_in_backoff_window(Some(&recent), 0, &now));
    }
}
