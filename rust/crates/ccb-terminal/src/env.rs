use std::path::Path;

/// Read a float environment variable with a default.
pub fn env_float(name: &str, default: f64) -> f64 {
    match std::env::var(name) {
        Ok(raw) => raw
            .trim()
            .parse::<f64>()
            .map(|v| v.max(0.0))
            .unwrap_or(default),
        Err(_) => default,
    }
}

/// Read an integer environment variable with a default.
pub fn env_int(name: &str, default: i64) -> i64 {
    match std::env::var(name) {
        Ok(raw) => {
            let raw = raw.trim();
            if raw.is_empty() {
                default
            } else {
                raw.parse::<i64>().unwrap_or(default)
            }
        }
        Err(_) => default,
    }
}

/// Sanitize a filename by replacing unsafe characters with underscores.
pub fn sanitize_filename(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    let sanitized: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    sanitized.trim_matches('_').to_string()
}

/// Detect Windows.
pub fn is_windows() -> bool {
    std::env::consts::OS == "windows"
}

/// Detect WSL.
pub fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|s| s.to_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

/// Return default shell and primary flag.
pub fn default_shell() -> (String, String) {
    if is_wsl() {
        return ("bash".to_string(), "-c".to_string());
    }
    if is_windows() {
        for shell in ["pwsh", "powershell"] {
            if which(shell) {
                return (shell.to_string(), "-Command".to_string());
            }
        }
        return ("powershell".to_string(), "-Command".to_string());
    }
    ("bash".to_string(), "-c".to_string())
}

fn which(name: &str) -> bool {
    std::env::var("PATH")
        .ok()
        .map(|path| {
            path.split(':')
                .any(|dir| Path::new(dir).join(name).exists())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_float() {
        std::env::set_var("TEST_CCB_FLOAT", "1.5");
        assert_eq!(env_float("TEST_CCB_FLOAT", 0.0), 1.5);
        std::env::set_var("TEST_CCB_FLOAT", "-1");
        assert_eq!(env_float("TEST_CCB_FLOAT", 0.0), 0.0);
        std::env::remove_var("TEST_CCB_FLOAT");
        assert_eq!(env_float("TEST_CCB_FLOAT_MISSING", 2.0), 2.0);
    }

    #[test]
    fn test_env_int() {
        std::env::set_var("TEST_CCB_INT", "42");
        assert_eq!(env_int("TEST_CCB_INT", 0), 42);
        std::env::set_var("TEST_CCB_INT", "");
        assert_eq!(env_int("TEST_CCB_INT", 0), 0);
        std::env::remove_var("TEST_CCB_INT");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello world!"), "hello_world");
        assert_eq!(sanitize_filename(""), "");
        assert_eq!(sanitize_filename("file.name_v1"), "file.name_v1");
    }
}
