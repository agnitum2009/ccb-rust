use ccb_provider_core::source_home::current_provider_source_home;

fn with_env<F>(vars: &[(&str, Option<&std::path::Path>)], test: F)
where
    F: FnOnce(),
{
    let mut saved: Vec<(String, Option<String>)> = Vec::new();
    unsafe {
        for (name, value) in vars {
            saved.push((name.to_string(), std::env::var(name).ok()));
            match value {
                Some(path) => std::env::set_var(name, path),
                None => std::env::remove_var(name),
            }
        }
    }
    test();
    unsafe {
        for (name, value) in saved {
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[test]
fn test_current_provider_source_home_uses_home_when_not_managed() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir(&home).unwrap();
    with_env(
        &[
            ("HOME", Some(&home)),
            ("CCB_SOURCE_HOME", None),
            ("USERPROFILE", None),
        ],
        || {
            assert_eq!(current_provider_source_home(), home);
        },
    );
}

#[test]
#[cfg(unix)]
fn test_current_provider_source_home_falls_back_from_managed_home_to_passwd_home() {
    let tmp = tempfile::tempdir().unwrap();
    let managed_home = tmp
        .path()
        .join("repo")
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-state")
        .join("claude")
        .join("home");
    std::fs::create_dir_all(&managed_home).unwrap();
    with_env(
        &[
            ("HOME", Some(&managed_home)),
            ("CCB_SOURCE_HOME", None),
            ("USERPROFILE", None),
        ],
        || {
            let result = current_provider_source_home();
            assert_ne!(result, managed_home);
            assert!(result.is_absolute());
        },
    );
}

#[test]
fn test_current_provider_source_home_honors_explicit_override() {
    let tmp = tempfile::tempdir().unwrap();
    let override_home = tmp.path().join("override-home");
    std::fs::create_dir(&override_home).unwrap();
    let managed_home = tmp
        .path()
        .join("repo")
        .join(".ccb")
        .join("agents")
        .join("agent1")
        .join("provider-state")
        .join("claude")
        .join("home");
    std::fs::create_dir_all(&managed_home).unwrap();
    with_env(
        &[
            ("HOME", Some(&managed_home)),
            ("CCB_SOURCE_HOME", Some(&override_home)),
            ("USERPROFILE", None),
        ],
        || {
            assert_eq!(current_provider_source_home(), override_home);
        },
    );
}
