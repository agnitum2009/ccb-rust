use std::env;

pub fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(val) => {
            let v = val.trim().to_lowercase();
            match v.as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => default,
            }
        }
        Err(_) => default,
    }
}

pub fn env_int(name: &str, default: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

pub fn env_float(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_bool_default() {
        assert!(!env_bool("CCB_NONEXISTENT_VAR_XYZ", false));
        assert!(env_bool("CCB_NONEXISTENT_VAR_XYZ", true));
    }

    #[test]
    fn test_env_int_default() {
        assert_eq!(env_int("CCB_NONEXISTENT_VAR_XYZ", 42), 42);
    }

    #[test]
    fn test_env_float_default() {
        assert!((env_float("CCB_NONEXISTENT_VAR_XYZ", 1.5) - 1.5).abs() < f64::EPSILON);
    }
}
