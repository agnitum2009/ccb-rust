// NOTE: These tests use global environment variables (HOME, AGENT_ROLES_STORE) and
// must be run sequentially to avoid race conditions. Run with:
//   cargo test -p ccbr-agents --test rolepack_tests -- --test-threads=1

use std::path::{Path, PathBuf};

use ccbr_agents::rolepacks::{
    add_role_source, agent_roles_installed_root, confirm_project_role_lock_refresh,
    default_agent_roles_source, discover_path_roles, discover_source_roles,
    discover_system_source_roles, find_project_role_lock_updates, find_source_role,
    find_system_source_role, installed_role_ids, installed_role_metadata, load_installed_role,
    load_locked_installed_role, load_project_agent_role, load_role_sources,
    project_role_lock_entry, project_role_lock_path, project_role_lock_warning,
    project_role_memory_sources, project_role_skill_sources, remove_role_source,
    resolve_project_agent_role, role_catalog_status, system_role_sources, tree_digest,
    write_project_role_lock,
};
use tempfile::TempDir;

fn write_role(root: &Path, id: &str, version: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("role.toml"),
        format!(
            r#"schema = "rolepack/v1"
id = "{id}"
name = "{name}"
version = "{version}"
description = "A test role"

[identity]
default_agent_name = "{agent}"

[compatibility]
providers = ["codex"]

[memory]
files = ["memory.md"]

[skills]
codex = ["skills/codex"]
"#,
            name = id.split('.').next_back().unwrap(),
            agent = id.split('.').next_back().unwrap(),
        ),
    )
    .unwrap();
    std::fs::write(root.join("memory.md"), "# role memory").unwrap();
    let skills = root.join("skills").join("codex");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("skill.md"), "# skill").unwrap();
}

fn with_isolated_env<F>(test: F)
where
    F: FnOnce(),
{
    let dir = TempDir::new().unwrap();
    let home = dir.path().to_path_buf();
    let roles_store = home.join(".roles");
    std::fs::create_dir_all(&roles_store).unwrap();
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("AGENT_ROLES_STORE", &roles_store);
        std::env::remove_var("CCBR_SYSTEM_ROLES_HOME");
        std::env::remove_var("CCBR_ROLES_HOME");
        std::env::remove_var("AGENT_ROLES_SPEC_HOME");
        std::env::remove_var("CCBR_AGENT_ROLES_SPEC_HOME");
    }
    test();
}

#[test]
fn test_load_installed_role_direct() {
    with_isolated_env(|| {
        let root = agent_roles_installed_root().join("agentroles.test");
        write_role(&root, "agentroles.test", "1.0.0");

        let role = load_installed_role("agentroles.test").unwrap().unwrap();
        assert_eq!(role.id, "agentroles.test");
        assert_eq!(role.version, "1.0.0");
    });
}

#[test]
fn test_load_installed_role_via_current_symlink() {
    with_isolated_env(|| {
        let installed = agent_roles_installed_root().join("agentroles.test");
        let current_target = installed.join("versions").join("1.0.0").join("abc123");
        write_role(&current_target, "agentroles.test", "1.0.0");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&current_target, installed.join("current")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&current_target, installed.join("current")).unwrap();

        let role = load_installed_role("agentroles.test").unwrap().unwrap();
        assert_eq!(role.id, "agentroles.test");
    });
}

#[test]
fn test_installed_role_metadata_and_ids() {
    with_isolated_env(|| {
        let root = agent_roles_installed_root().join("agentroles.test");
        write_role(&root, "agentroles.test", "1.0.0");
        std::fs::write(
            root.join("install.json"),
            r#"{"schema":"agent-roles-install/v1","version":"1.0.0","digest":"sha256:abc"}"#,
        )
        .unwrap();

        let meta = installed_role_metadata("agentroles.test").unwrap().unwrap();
        assert_eq!(meta.get("version").unwrap().as_str().unwrap(), "1.0.0");

        let ids = installed_role_ids().unwrap();
        assert!(ids.contains(&"agentroles.test".to_string()));
    });
}

#[test]
fn test_load_locked_installed_role() {
    with_isolated_env(|| {
        let installed = agent_roles_installed_root().join("agentroles.test");
        let candidate = installed.join("versions").join("1.0.0").join("abc123");
        write_role(&candidate, "agentroles.test", "1.0.0");

        let role = load_locked_installed_role("agentroles.test", "1.0.0", "sha256:abc123")
            .unwrap()
            .unwrap();
        assert_eq!(role.id, "agentroles.test");
        assert_eq!(role.version, "1.0.0");
    });
}

#[test]
fn test_load_locked_installed_role_by_digest() {
    with_isolated_env(|| {
        let installed = agent_roles_installed_root().join("agentroles.test");
        let version_root = installed.join("versions").join("1.0.0");
        write_role(&version_root, "agentroles.test", "1.0.0");
        let digest = format!("sha256:{}", tree_digest(&version_root));

        let role = load_locked_installed_role("agentroles.test", "1.0.0", &digest)
            .unwrap()
            .unwrap();
        assert_eq!(role.id, "agentroles.test");
    });
}

#[test]
fn test_project_role_lock_read_write() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let ccbr = project.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr).unwrap();
        std::fs::write(
            ccbr.join("ccbr.config"),
            r#"default_agents = ["agent1"]

[windows]
main = "agent1:codex"

[agents.agent1]
provider = "codex"
target = "."
role = "agentroles.test"
"#,
        )
        .unwrap();

        let installed = agent_roles_installed_root().join("agentroles.test");
        write_role(&installed, "agentroles.test", "1.0.0");

        let manifest = load_installed_role("agentroles.test").unwrap().unwrap();
        write_project_role_lock(project.path(), &manifest).unwrap();

        let lock_path = project_role_lock_path(project.path());
        assert!(lock_path.exists());

        let entry = project_role_lock_entry(project.path(), "agentroles.test")
            .unwrap()
            .unwrap();
        assert_eq!(entry.get("version").unwrap().as_str().unwrap(), "1.0.0");
        assert_eq!(
            entry.get("default_agent_name").unwrap().as_str().unwrap(),
            "test"
        );
    });
}

#[test]
fn test_resolve_project_agent_role() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let ccbr = project.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr).unwrap();
        std::fs::write(
            ccbr.join("ccbr.config"),
            r#"default_agents = ["agent1"]

[windows]
main = "agent1:codex"

[agents.agent1]
provider = "codex"
target = "."
role = "agentroles.test"
"#,
        )
        .unwrap();

        let installed = agent_roles_installed_root().join("agentroles.test");
        write_role(&installed, "agentroles.test", "1.0.0");

        let resolved = resolve_project_agent_role(project.path(), "agent1")
            .unwrap()
            .unwrap();
        assert_eq!(resolved.role_id, "agentroles.test");
        assert!(resolved.role.is_some());
        assert!(resolved.warning.is_empty());

        let role = load_project_agent_role(project.path(), "agent1")
            .unwrap()
            .unwrap();
        assert_eq!(role.id, "agentroles.test");
    });
}

#[test]
fn test_project_role_lock_warning_on_mismatch() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let ccbr = project.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr).unwrap();
        std::fs::write(
            ccbr.join("ccbr.config"),
            r#"default_agents = ["agent1"]

[windows]
main = "agent1:codex"

[agents.agent1]
provider = "codex"
target = "."
role = "agentroles.test"
"#,
        )
        .unwrap();

        let installed = agent_roles_installed_root().join("agentroles.test");
        write_role(&installed, "agentroles.test", "1.0.0");

        std::fs::write(
            ccbr.join("role-lock.json"),
            r#"{"schema":"rolepack-lock/v1","roles":{"agentroles.test":{"version":"1.0.0","digest":"sha256:bad","source":"installed","default_agent_name":"test"}}}"#,
        )
        .unwrap();

        let resolved = resolve_project_agent_role(project.path(), "agent1")
            .unwrap()
            .unwrap();
        assert!(resolved.role.is_none());
        assert!(resolved.warning.contains("role_lock_mismatch"));

        let role = load_installed_role("agentroles.test").unwrap().unwrap();
        let warning = project_role_lock_warning(project.path(), &role).unwrap();
        assert!(warning.contains("role_lock_mismatch"));
    });
}

#[test]
fn test_project_role_memory_and_skill_sources() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let ccbr = project.path().join(".ccbr");
        std::fs::create_dir_all(&ccbr).unwrap();
        std::fs::write(
            ccbr.join("ccbr.config"),
            r#"default_agents = ["agent1"]

[windows]
main = "agent1:codex"

[agents.agent1]
provider = "codex"
target = "."
role = "agentroles.test"
"#,
        )
        .unwrap();

        let installed = agent_roles_installed_root().join("agentroles.test");
        write_role(&installed, "agentroles.test", "1.0.0");

        let memory = project_role_memory_sources(project.path(), "agent1").unwrap();
        assert_eq!(memory.len(), 1);
        assert_eq!(memory[0].content, "# role memory");

        let skills = project_role_skill_sources(project.path(), "agent1", "codex").unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].0, "codex");
    });
}

#[test]
fn test_system_role_sources_and_discovery() {
    with_isolated_env(|| {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let system_roles = home.join(".ccbr").join("roles").join("agentroles.test");
        write_role(&system_roles, "agentroles.test", "1.0.0");

        let sources = system_role_sources();
        assert!(sources.iter().any(|s| s.name == "systemroles"));

        let roles = discover_system_source_roles().unwrap();
        assert!(roles.iter().any(|r| r.role_id == "agentroles.test"));

        let found = find_system_source_role("agentroles.test").unwrap().unwrap();
        assert_eq!(found.role_id, "agentroles.test");
    });
}

#[test]
fn test_role_source_registry_add_remove_and_discovery() {
    with_isolated_env(|| {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let source_dir = home.join("custom-source");
        let role_dir = source_dir.join("agentroles.custom");
        write_role(&role_dir, "agentroles.custom", "1.0.0");

        let added = add_role_source("custom", &source_dir).unwrap();
        assert_eq!(
            added.get("source_status").unwrap().as_str().unwrap(),
            "added"
        );

        let sources = load_role_sources(true, false);
        assert!(sources.iter().any(|s| s.name == "custom"));

        let roles = discover_source_roles(false, false).unwrap();
        assert!(roles.iter().any(|r| r.role_id == "agentroles.custom"));

        let found = find_source_role("agentroles.custom", false, false)
            .unwrap()
            .unwrap();
        assert_eq!(found.role_id, "agentroles.custom");

        let removed = remove_role_source("custom").unwrap();
        assert_eq!(
            removed.get("source_status").unwrap().as_str().unwrap(),
            "removed"
        );
        let sources_after = load_role_sources(false, false);
        assert!(!sources_after.iter().any(|s| s.name == "custom"));
    });
}

#[test]
fn test_discover_path_roles() {
    let dir = TempDir::new().unwrap();
    let role_dir = dir.path().join("agentroles.path");
    write_role(&role_dir, "agentroles.path", "1.0.0");

    let roles = discover_path_roles(dir.path()).unwrap();
    assert!(roles.iter().any(|r| r.role_id == "agentroles.path"));
}

#[test]
fn test_role_catalog_status() {
    with_isolated_env(|| {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let system_roles = home.join(".ccbr").join("roles").join("agentroles.test");
        write_role(&system_roles, "agentroles.test", "1.0.0");

        let installed = agent_roles_installed_root().join("agentroles.test");
        write_role(&installed, "agentroles.test", "1.0.0");
        let digest = format!("sha256:{}", tree_digest(&installed));
        std::fs::write(
            installed.join("install.json"),
            format!(
                r#"{{"schema":"agent-roles-install/v1","version":"1.0.0","digest":"{digest}"}}"#
            ),
        )
        .unwrap();

        let rows = role_catalog_status(false).unwrap();
        let row = rows
            .iter()
            .find(|r| r.get("role_id").unwrap().as_str().unwrap() == "agentroles.test")
            .unwrap();
        assert_eq!(row.get("status").unwrap().as_str().unwrap(), "current");
    });
}

#[test]
fn test_default_agent_roles_source_env() {
    with_isolated_env(|| {
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let spec_home = home.join("custom-spec");
        std::fs::create_dir_all(spec_home.join("roles")).unwrap();
        unsafe {
            std::env::set_var("AGENT_ROLES_SPEC_HOME", &spec_home);
        }
        let found = default_agent_roles_source(false).unwrap();
        assert_eq!(
            found.canonicalize().unwrap(),
            spec_home.canonicalize().unwrap()
        );
    });
}

fn install_role_version(role_id: &str, version: &str) -> (std::path::PathBuf, String) {
    let installed = agent_roles_installed_root().join(role_id);
    let target = installed
        .join("versions")
        .join(version)
        .join(format!("{role_id}-{version}"));
    std::fs::create_dir_all(&target).unwrap();
    write_role(&target, role_id, version);
    let digest = format!("sha256:{}", tree_digest(&target));
    std::fs::write(
        installed.join("install.json"),
        format!(
            r#"{{"schema":"agent-roles-install/v1","id":"{role_id}","version":"{version}","digest":"{digest}"}}"#
        ),
    )
    .unwrap();
    (target, digest)
}

fn set_current_symlink(installed: &std::path::Path, target: &std::path::Path) {
    let current = installed.join("current");
    if current.exists() || current.is_symlink() {
        if current.is_symlink() || current.is_file() {
            std::fs::remove_file(&current).unwrap();
        } else {
            std::fs::remove_dir_all(&current).unwrap();
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &current).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, &current).unwrap();
}

fn write_locked_project(
    project: &std::path::Path,
    role_id: &str,
    locked_version: &str,
    locked_digest: &str,
) {
    let ccbr = project.join(".ccbr");
    std::fs::create_dir_all(&ccbr).unwrap();
    std::fs::write(
        ccbr.join("ccbr.config"),
        format!(
            r#"version = 2
entry_window = "main"

[windows]
main = "locked:codex"

[agents.locked]
provider = "codex"
role = "{role_id}"
"#
        ),
    )
    .unwrap();
    std::fs::write(
        ccbr.join("role-lock.json"),
        format!(
            r#"{{"schema":"rolepack-lock/v1","roles":{{"{role_id}":{{"version":"{locked_version}","digest":"{locked_digest}","source":"installed","default_agent_name":"locked"}}}}}}"#
        ),
    )
    .unwrap();
}

#[test]
fn test_find_project_role_lock_updates_detects_installed_current_drift() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let (_v1_target, v1_digest) = install_role_version("test.locked", "1.0.0");
        let (v2_target, v2_digest) = install_role_version("test.locked", "2.0.0");
        let installed = agent_roles_installed_root().join("test.locked");
        set_current_symlink(&installed, &v2_target);
        write_locked_project(project.path(), "test.locked", "1.0.0", &v1_digest);

        let updates = find_project_role_lock_updates(project.path()).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].role_id, "test.locked");
        assert_eq!(updates[0].locked_version, "1.0.0");
        assert_eq!(updates[0].locked_digest, v1_digest);
        assert_eq!(updates[0].current_version, "2.0.0");
        assert_eq!(updates[0].current_digest, v2_digest);
    });
}

#[test]
fn test_confirm_project_role_lock_refresh_updates_lock_when_accepted() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let (_v1_target, v1_digest) = install_role_version("test.locked", "1.0.0");
        let (v2_target, v2_digest) = install_role_version("test.locked", "2.0.0");
        let installed = agent_roles_installed_root().join("test.locked");
        set_current_symlink(&installed, &v2_target);
        write_locked_project(project.path(), "test.locked", "1.0.0", &v1_digest);

        let mut stdout = Vec::new();
        let mut stdin = "y\n".as_bytes();
        let updates =
            confirm_project_role_lock_refresh(project.path(), &mut stdout, &mut stdin, true)
                .unwrap();
        assert_eq!(updates.len(), 1);

        let output = String::from_utf8(stdout).unwrap();
        assert!(output.contains("Role Pack updates are available"));
        assert!(output.contains("role_lock_refreshed: test.locked version=2.0.0"));

        let lock_text =
            std::fs::read_to_string(project.path().join(".ccbr").join("role-lock.json")).unwrap();
        let lock: serde_json::Value = serde_json::from_str(&lock_text).unwrap();
        let entry = lock.get("roles").unwrap().get("test.locked").unwrap();
        assert_eq!(entry.get("version").unwrap().as_str().unwrap(), "2.0.0");
        assert_eq!(entry.get("digest").unwrap().as_str().unwrap(), v2_digest);
    });
}

#[test]
fn test_confirm_project_role_lock_refresh_warns_without_mutating_noninteractive() {
    with_isolated_env(|| {
        let project = TempDir::new().unwrap();
        let (_v1_target, v1_digest) = install_role_version("test.locked", "1.0.0");
        let (v2_target, _v2_digest) = install_role_version("test.locked", "2.0.0");
        let installed = agent_roles_installed_root().join("test.locked");
        set_current_symlink(&installed, &v2_target);
        write_locked_project(project.path(), "test.locked", "1.0.0", &v1_digest);

        let mut stdout = Vec::new();
        let mut stdin = "y\n".as_bytes();
        let updates =
            confirm_project_role_lock_refresh(project.path(), &mut stdout, &mut stdin, false)
                .unwrap();
        assert_eq!(updates.len(), 1);

        let output = String::from_utf8(stdout).unwrap();
        assert!(output.contains("role_lock_update_available: test.locked"));
        assert!(output.contains("role_lock_refresh: skipped_noninteractive"));

        let lock_text =
            std::fs::read_to_string(project.path().join(".ccbr").join("role-lock.json")).unwrap();
        let lock: serde_json::Value = serde_json::from_str(&lock_text).unwrap();
        let entry = lock.get("roles").unwrap().get("test.locked").unwrap();
        assert_eq!(entry.get("version").unwrap().as_str().unwrap(), "1.0.0");
        assert_eq!(entry.get("digest").unwrap().as_str().unwrap(), v1_digest);
    });
}
