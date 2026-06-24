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
}
