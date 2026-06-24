use std::env;

/// Return whether CCB debug logging is enabled.
pub fn debug_enabled() -> bool {
    matches!(
        env::var("CCB_DEBUG").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Emit a debug message to stderr when `CCB_DEBUG` is enabled.
pub fn debug(message: &str) {
    if !debug_enabled() {
        return;
    }
    eprintln!("[DEBUG] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_enabled_variants() {
        let old = env::var("CCB_DEBUG").ok();
        for value in ["1", "true", "yes"] {
            env::set_var("CCB_DEBUG", value);
            assert!(debug_enabled(), "expected enabled for {value}");
        }
        env::set_var("CCB_DEBUG", "0");
        assert!(!debug_enabled());
        env::remove_var("CCB_DEBUG");
        assert!(!debug_enabled());
        if let Some(v) = old {
            env::set_var("CCB_DEBUG", v);
        }
    }

    #[test]
    fn test_debug_does_not_panic_when_disabled() {
        env::remove_var("CCB_DEBUG");
        debug("this should be silent");
    }
}
