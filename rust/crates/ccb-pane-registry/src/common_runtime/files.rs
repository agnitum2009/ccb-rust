use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use camino::Utf8PathBuf;
use ccb_project::runtime_paths::project_registry_dir;
use serde_json::{Map, Value};

pub const REGISTRY_PREFIX: &str = "ccb-session-";
pub const REGISTRY_SUFFIX: &str = ".json";
pub const REGISTRY_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;

fn to_utf8_path(path: &Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .unwrap_or_else(|p| Utf8PathBuf::from(p.to_string_lossy().as_ref()))
}

/// Return the registry directory for a work directory.
pub fn registry_dir(work_dir: Option<&Path>) -> PathBuf {
    let utf8 = match work_dir {
        Some(dir) => to_utf8_path(dir),
        None => to_utf8_path(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
    };
    project_registry_dir(&utf8).as_std_path().to_path_buf()
}

/// Return the registry file path for a session.
pub fn registry_path_for_session(session_id: &str, work_dir: Option<&Path>) -> PathBuf {
    registry_dir(work_dir).join(format!("{REGISTRY_PREFIX}{session_id}{REGISTRY_SUFFIX}"))
}

/// Iterate over registry file paths.
pub fn iter_registry_files(work_dir: Option<&Path>) -> Vec<PathBuf> {
    let dir = registry_dir(work_dir);
    if !dir.exists() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    name.starts_with(REGISTRY_PREFIX) && name.ends_with(REGISTRY_SUFFIX)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Coerce an `updated_at` value to seconds, falling back to file mtime.
pub fn coerce_updated_at(value: Option<&Value>, fallback_path: Option<&Path>) -> i64 {
    if let Some(v) = value {
        if let Some(i) = v.as_i64() {
            return i;
        }
        if let Some(f) = v.as_f64() {
            return f as i64;
        }
        if let Some(s) = v.as_str() {
            if let Ok(i) = s.trim().parse::<i64>() {
                return i;
            }
        }
    }
    if let Some(path) = fallback_path {
        if let Ok(meta) = path.metadata() {
            if let Ok(modified) = meta.modified() {
                return modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
            }
        }
    }
    0
}

/// Check if a registry timestamp is stale.
pub fn is_stale(updated_at: i64, now: Option<i64>) -> bool {
    if updated_at <= 0 {
        return true;
    }
    let now_ts = now.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    });
    (now_ts - updated_at) > REGISTRY_TTL_SECONDS
}

/// Load a registry file as JSON.
pub fn load_registry_file(path: &Path) -> Option<Map<String, Value>> {
    let data = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value.as_object().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_registry_path_for_session() {
        let path = registry_path_for_session("abc123", Some(Path::new("/tmp/proj")));
        let s = path.to_string_lossy();
        assert!(s.contains("ccb-session-abc123.json"));
        assert!(s.contains("registry"));
    }

    #[test]
    fn test_coerce_updated_at_numeric() {
        assert_eq!(coerce_updated_at(Some(&Value::from(42)), None), 42);
        assert_eq!(coerce_updated_at(Some(&Value::from(42.9)), None), 42);
    }

    #[test]
    fn test_coerce_updated_at_string_digit() {
        assert_eq!(
            coerce_updated_at(Some(&Value::String("123".into())), None),
            123
        );
        assert_eq!(
            coerce_updated_at(Some(&Value::String("  123  ".into())), None),
            123
        );
    }

    #[test]
    fn test_coerce_updated_at_fallback_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("x.json");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(b"{}").unwrap();
        drop(f);
        let mtime = coerce_updated_at(None, Some(&path));
        assert!(mtime > 0);
    }

    #[test]
    fn test_is_stale() {
        let now = 1_000_000i64;
        assert!(is_stale(0, Some(now)));
        assert!(is_stale(-1, Some(now)));
        assert!(!is_stale(now, Some(now)));
        assert!(is_stale(now - REGISTRY_TTL_SECONDS - 1, Some(now)));
    }

    #[test]
    fn test_load_registry_file_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.json");
        fs::write(&path, r#"{"key": "value"}"#).unwrap();
        let data = load_registry_file(&path).unwrap();
        assert_eq!(data["key"], "value");
    }

    #[test]
    fn test_load_registry_file_missing() {
        let path = Path::new("/nonexistent/registry.json");
        assert!(load_registry_file(path).is_none());
    }

    #[test]
    fn test_iter_registry_files_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(registry_dir(Some(tmp.path()))).unwrap();
        let p1 = registry_path_for_session("z", Some(tmp.path()));
        let p2 = registry_path_for_session("a", Some(tmp.path()));
        fs::write(&p1, "{}").unwrap();
        fs::write(&p2, "{}").unwrap();
        let files = iter_registry_files(Some(tmp.path()));
        assert_eq!(files.len(), 2);
        assert!(files[0].to_string_lossy().contains("ccb-session-a.json"));
    }
}
