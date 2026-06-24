//! Mirrors Python `lib/cli/services/doctor_runtime/system.py`.

use serde_json::Value;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const ROOT_RUNTIME_WARNING: &str =
    "Running CCBR as root in a non-root-owned project can create root-owned .ccbr files.";

/// Build a runtime identity summary for the doctor command.
///
/// Mirrors Python `runtime_identity_summary`.
pub fn runtime_identity_summary(
    project_root: &Path,
    ccbr_dir: Option<&Path>,
    installation: Option<&Value>,
) -> Value {
    let uid = effective_uid();
    let user_name = user_name(uid);
    runtime_identity_summary_with(
        project_root,
        ccbr_dir,
        installation,
        uid,
        &user_name,
        path_owner,
    )
}

/// Type alias for a function that resolves path ownership.
pub type PathOwnerFn = dyn Fn(&Path) -> Option<Value> + Sync;

/// Injected version of `runtime_identity_summary` for testing.
pub fn runtime_identity_summary_with(
    project_root: &Path,
    ccbr_dir: Option<&Path>,
    installation: Option<&Value>,
    uid: i64,
    user_name: &str,
    path_owner_fn: impl Fn(&Path) -> Option<Value>,
) -> Value {
    let installation = installation.unwrap_or(&Value::Null);
    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());
    let install_path_text = installation
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let project_owner = path_owner_fn(project_root);
    let ccbr_owner = ccbr_dir.and_then(&path_owner_fn);
    let install_owner = if install_path_text.is_empty() {
        None
    } else {
        path_owner_fn(Path::new(install_path_text))
    };

    let install_root_owned = install_root_owned(installation, install_owner.as_ref());
    let root_runtime = uid == 0;

    let mut warnings = Vec::new();
    if root_runtime {
        if let Some(owner) = project_owner.as_ref() {
            let owner_uid = owner.get("uid").and_then(|v| v.as_i64());
            if owner_uid != Some(0) {
                warnings.push(ROOT_RUNTIME_WARNING.to_string());
            }
        }
    }

    serde_json::json!({
        "user_id": uid,
        "user_name": user_name,
        "home": home,
        "root_runtime": root_runtime,
        "install_root_owned": install_root_owned,
        "install_user_id": installation.get("install_user_id").cloned().unwrap_or(Value::Null),
        "install_user_name": installation.get("install_user_name").cloned().unwrap_or(Value::Null),
        "sudo_user": sudo_user(installation),
        "project_owner": owner_display(project_owner.as_ref()),
        "ccbr_dir_owner": owner_display(ccbr_owner.as_ref()),
        "install_owner": owner_display(install_owner.as_ref()),
        "warnings": warnings,
    })
}

/// Return the effective UID, falling back to the real UID.
pub fn effective_uid() -> i64 {
    // SAFETY: geteuid/getuid are async-signal-safe and read-only.
    let uid = unsafe {
        if libc::geteuid() != u32::MAX {
            libc::geteuid()
        } else {
            libc::getuid()
        }
    };
    i64::from(uid)
}

/// Resolve a user name for a UID.
pub fn user_name(uid: i64) -> String {
    // SAFETY: getpwuid returns a pointer to static/thread-local storage.
    let pw = unsafe { libc::getpwuid(uid as u32) };
    if !pw.is_null() {
        // SAFETY: pw_name is a valid C string when getpwuid succeeds.
        let name = unsafe {
            let cstr = std::ffi::CStr::from_ptr((*pw).pw_name);
            cstr.to_string_lossy().to_string()
        };
        if !name.is_empty() {
            return name;
        }
    }
    std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

/// Return the owning UID/name of a path, if available.
pub fn path_owner(path: &Path) -> Option<Value> {
    let metadata = std::fs::metadata(path).ok()?;
    let uid = i64::from(metadata.uid());
    Some(serde_json::json!({
        "uid": uid,
        "name": user_name(uid),
    }))
}

fn owner_display(owner: Option<&Value>) -> String {
    let owner = match owner {
        Some(v) => v,
        None => return String::new(),
    };
    let uid = owner.get("uid").and_then(|v| v.as_i64()).unwrap_or(-1);
    let name = owner
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    format!("{uid}:{name}")
}

fn sudo_user(installation: &Value) -> String {
    if let Some(user) = installation.get("sudo_user").and_then(|v| v.as_str()) {
        return user.to_string();
    }
    std::env::var("SUDO_USER").unwrap_or_default()
}

fn install_root_owned(installation: &Value, install_owner: Option<&Value>) -> Option<bool> {
    if let Some(root_install) = installation.get("root_install").and_then(|v| v.as_bool()) {
        return Some(root_install);
    }
    if let Some(install_user_id) = installation.get("install_user_id").and_then(|v| v.as_i64()) {
        return Some(install_user_id == 0);
    }
    if let Some(owner) = install_owner {
        let owner_uid = owner.get("uid").and_then(|v| v.as_i64());
        return owner_uid.map(|uid| uid == 0);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_identity_summary_root_warning_when_project_not_root_owned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path().join("project");
        let ccbr_dir = project_root.join(".ccbr");
        let install_dir = tmp.path().join("install");
        std::fs::create_dir_all(&ccbr_dir).unwrap();
        std::fs::create_dir_all(&install_dir).unwrap();

        let fake_path_owner = |path: &Path| -> Option<Value> {
            if path == project_root || path == ccbr_dir {
                Some(serde_json::json!({"uid": 1000, "name": "demo"}))
            } else if path == install_dir {
                Some(serde_json::json!({"uid": 0, "name": "root"}))
            } else {
                None
            }
        };

        let installation = serde_json::json!({
            "path": install_dir,
            "root_install": true,
            "install_user_id": "0",
            "install_user_name": "root",
            "sudo_user": "demo",
        });

        let summary = runtime_identity_summary_with(
            &project_root,
            Some(&ccbr_dir),
            Some(&installation),
            0,
            "root",
            fake_path_owner,
        );

        assert_eq!(summary["user_id"], 0);
        assert_eq!(summary["user_name"], "root");
        assert_eq!(summary["home"], std::env::var("HOME").unwrap_or_default());
        assert_eq!(summary["root_runtime"], true);
        assert_eq!(summary["install_root_owned"], true);
        assert_eq!(summary["install_user_id"], "0");
        assert_eq!(summary["install_user_name"], "root");
        assert_eq!(summary["sudo_user"], "demo");
        assert_eq!(summary["project_owner"], "1000:demo");
        assert_eq!(summary["ccbr_dir_owner"], "1000:demo");
        assert_eq!(summary["install_owner"], "0:root");
        let warnings = summary["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], ROOT_RUNTIME_WARNING);
    }

    #[test]
    fn test_runtime_identity_summary_no_warning_for_non_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path().join("project");
        let ccbr_dir = project_root.join(".ccbr");
        std::fs::create_dir_all(&ccbr_dir).unwrap();

        let summary = runtime_identity_summary(&project_root, Some(&ccbr_dir), None);

        // root_runtime reflects the actual UID of the test process; this test
        // only verifies that no warning is emitted when project ownership matches.
        assert!(summary["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_install_root_owned_prefers_root_install_flag() {
        let installation = serde_json::json!({"root_install": true});
        assert_eq!(install_root_owned(&installation, None), Some(true));

        let installation = serde_json::json!({"root_install": false});
        assert_eq!(install_root_owned(&installation, None), Some(false));
    }

    #[test]
    fn test_install_root_owned_falls_back_to_install_user_id() {
        let installation = serde_json::json!({"install_user_id": 0});
        assert_eq!(install_root_owned(&installation, None), Some(true));

        let installation = serde_json::json!({"install_user_id": 1000});
        assert_eq!(install_root_owned(&installation, None), Some(false));
    }

    #[test]
    fn test_install_root_owned_falls_back_to_install_owner() {
        let installation = serde_json::json!({});
        let owner = serde_json::json!({"uid": 0});
        assert_eq!(install_root_owned(&installation, Some(&owner)), Some(true));

        let owner = serde_json::json!({"uid": 1000});
        assert_eq!(install_root_owned(&installation, Some(&owner)), Some(false));
    }

    #[test]
    fn test_owner_display_formats_uid_and_name() {
        let owner = serde_json::json!({"uid": 1000, "name": "demo"});
        assert_eq!(owner_display(Some(&owner)), "1000:demo");
        assert_eq!(owner_display(None), "");
    }
}
