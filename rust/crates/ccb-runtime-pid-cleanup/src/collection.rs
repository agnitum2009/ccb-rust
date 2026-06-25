//! Candidate collection for runtime PID cleanup.
//!
//! Mirrors Python `runtime_pid_cleanup.collection`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use camino::Utf8Path;

use crate::procfs::read_pid_file;
use crate::utils::{coerce_pid, resolved_runtime_roots, RuntimeRef};

/// Collect PID candidates associated with an agent directory.
///
/// Mirrors Python `runtime_pid_cleanup.collection.collect_pid_candidates`.
pub fn collect_pid_candidates(
    agent_dir: &Path,
    runtime: Option<&dyn RuntimeRef>,
    fallback_to_agent_dir: bool,
) -> HashMap<u32, Vec<PathBuf>> {
    let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    if let Some(runtime) = runtime {
        let runtime_pid = runtime.runtime_pid().or_else(|| runtime.pid());
        if let Some(pid) = runtime_pid {
            candidates
                .entry(pid)
                .or_default()
                .push(agent_dir.join("runtime.json"));
        }
    }
    for root in resolved_runtime_roots(agent_dir, runtime, fallback_to_agent_dir) {
        let mut pid_paths: Vec<PathBuf> = walk_pid_files(&root);
        pid_paths.sort();
        for pid_path in pid_paths {
            if let Some(pid) = read_pid_file(&pid_path) {
                candidates.entry(pid).or_default().push(pid_path);
            }
        }
    }
    let helper_path = agent_dir.join("helper.json");
    if let Some(leader_pid) = load_helper_manifest_best_effort(&helper_path) {
        candidates.entry(leader_pid).or_default().push(helper_path);
    }
    candidates
}

/// Collect process candidates by scanning `/proc` for project markers.
///
/// Mirrors Python `runtime_pid_cleanup.collection.collect_project_process_candidates`.
pub fn collect_project_process_candidates(
    project_root: &Path,
    proc_root: &Path,
    read_proc_cmdline_fn: impl Fn(u32) -> String,
    current_pid: Option<u32>,
) -> HashMap<u32, Vec<PathBuf>> {
    let current_pid = current_pid.unwrap_or_else(std::process::id);
    let layout = match build_layout(project_root) {
        Some(layout) => layout,
        None => return HashMap::new(),
    };
    let ccb_root = layout.ccb_dir();
    let markers = project_runtime_markers(project_root, &ccb_root, &layout);
    if markers.is_empty() {
        return HashMap::new();
    }

    let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let entries = match std::fs::read_dir(proc_root) {
        Ok(entries) => entries,
        Err(_) => return candidates,
    };
    for entry in entries.flatten() {
        let pid = match coerce_pid(entry.file_name().to_string_lossy()) {
            Some(pid) => pid,
            None => continue,
        };
        if pid == current_pid {
            continue;
        }
        let cmdline = read_proc_cmdline_fn(pid).trim().to_string();
        let mut matched_markers: Vec<PathBuf> = markers
            .iter()
            .filter(|marker| cmdline.contains(&marker.to_string_lossy().to_string()))
            .cloned()
            .collect();
        if let Some(control_marker) = control_plane_marker(project_root, &cmdline, &layout) {
            if !matched_markers.contains(&control_marker) {
                matched_markers.push(control_marker);
            }
        }
        if matched_markers.is_empty() {
            continue;
        }
        candidates.entry(pid).or_default().extend(matched_markers);
    }
    candidates
}

/// Collect authority PIDs from CCBD JSON state files.
///
/// Mirrors Python `runtime_pid_cleanup.collection.collect_project_authority_pid_candidates`.
pub fn collect_project_authority_pid_candidates(project_root: &Path) -> HashMap<u32, Vec<PathBuf>> {
    let layout = match build_layout(project_root) {
        Some(layout) => layout,
        None => return HashMap::new(),
    };
    let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
    let paths_and_keys: [(PathBuf, &[&str]); 3] = [
        (
            layout.ccbd_lease_path().into(),
            &["ccbd_pid", "keeper_pid"],
        ),
        (layout.ccbd_keeper_path().into(), &["keeper_pid"]),
        (
            layout.ccbd_lifecycle_path().into(),
            &["owner_pid", "keeper_pid"],
        ),
    ];
    for (path, keys) in paths_and_keys {
        let Some(payload) = load_json_object(&path) else {
            continue;
        };
        for key in keys {
            if let Some(value) = payload.get(*key) {
                if let Some(pid) = coerce_pid(value.to_string()) {
                    candidates.entry(pid).or_default().push(path.clone());
                }
            }
        }
    }
    candidates
}

fn walk_pid_files(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() && path.extension().and_then(|e| e.to_str()) == Some("pid")
            {
                result.push(path);
            }
        }
    }
    result
}

fn load_helper_manifest_best_effort(path: &Path) -> Option<u32> {
    let utf8_path = Utf8Path::from_path(path)?;
    let manifest = ccb_providers::runtime::load_helper_manifest(utf8_path)?;
    u32::try_from(manifest.leader_pid).ok()
}

fn project_runtime_markers(
    _project_root: &Path,
    ccb_root: &Utf8Path,
    layout: &ccb_storage::paths::PathLayout,
) -> Vec<PathBuf> {
    let mut markers: Vec<PathBuf> = vec![ccb_root.into()];
    if matches!(
        layout.runtime_state_placement().root_kind,
        ccb_storage::path_helpers::RootKind::Relocated
    ) {
        for path in [
            PathBuf::from(layout.runtime_state_root().as_str()).join("agents"),
            PathBuf::from(layout.runtime_state_root().as_str()).join("ccbd"),
        ] {
            if !markers.contains(&path) {
                markers.push(path);
            }
        }
    }
    markers
}

fn build_layout(project_root: &Path) -> Option<ccb_storage::paths::PathLayout> {
    let utf8_path = Utf8Path::from_path(project_root)?;
    Some(ccb_storage::paths::PathLayout::new(utf8_path))
}

fn load_json_object(path: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let data = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value.as_object().cloned()
}

fn control_plane_marker(
    project_root: &Path,
    cmdline: &str,
    layout: &ccb_storage::paths::PathLayout,
) -> Option<PathBuf> {
    if cmdline.is_empty() {
        return None;
    }
    if !cmdline.contains("ccbd/main.py") && !cmdline.contains("ccbd/keeper_main.py") {
        return None;
    }
    if !cmdline_has_project_arg(cmdline, project_root) {
        return None;
    }
    Some(layout.ccbd_dir().into())
}

fn cmdline_has_project_arg(cmdline: &str, project_root: &Path) -> bool {
    let expected = resolved_project_text(project_root);
    let tokens: Vec<&str> = cmdline.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        if *token == "--project"
            && index + 1 < tokens.len()
            && resolved_project_text(Path::new(tokens[index + 1])) == expected
        {
            return true;
        }
        if let Some(value) = token.strip_prefix("--project=") {
            if resolved_project_text(Path::new(value)) == expected {
                return true;
            }
        }
    }
    false
}

fn resolved_project_text(value: &Path) -> String {
    let expanded = expand_user_path(value);
    let resolved = if let Ok(p) = expanded.canonicalize() {
        p
    } else if let Ok(p) = std::path::absolute(&expanded) {
        p
    } else {
        expanded.to_path_buf()
    };
    resolved.to_string_lossy().to_string()
}

fn expand_user_path(raw: &Path) -> PathBuf {
    let raw_str = raw.to_string_lossy();
    if let Some(rest) = raw_str.strip_prefix('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home + rest);
        }
    }
    raw.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestRuntime {
        runtime_pid: Option<u32>,
        pid: Option<u32>,
        runtime_root: Option<String>,
    }

    impl RuntimeRef for TestRuntime {
        fn runtime_pid(&self) -> Option<u32> {
            self.runtime_pid
        }

        fn pid(&self) -> Option<u32> {
            self.pid
        }

        fn runtime_root(&self) -> Option<&str> {
            self.runtime_root.as_deref()
        }
    }

    #[test]
    fn collect_pid_candidates_from_runtime_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        fs::create_dir_all(&agent_dir).unwrap();
        let runtime = TestRuntime {
            runtime_pid: Some(9999),
            pid: None,
            runtime_root: None,
        };
        let candidates = collect_pid_candidates(&agent_dir, Some(&runtime), false);
        assert_eq!(
            candidates.get(&9999),
            Some(&vec![agent_dir.join("runtime.json")])
        );
    }

    #[test]
    fn collect_pid_candidates_from_pid_files() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        let runtime_dir = agent_dir.join("provider-runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        let pid_path = runtime_dir.join("bridge.pid");
        fs::write(&pid_path, "4242").unwrap();

        let candidates = collect_pid_candidates(&agent_dir, None, true);
        assert_eq!(candidates.get(&4242), Some(&vec![pid_path]));
    }

    #[test]
    fn collect_pid_candidates_from_helper_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        fs::create_dir_all(&agent_dir).unwrap();
        let helper_path = agent_dir.join("helper.json");
        fs::write(
            &helper_path,
            serde_json::json!({
                "schema_version": 1,
                "record_type": "provider_helper_manifest",
                "agent_name": "codex",
                "runtime_generation": 1,
                "helper_kind": "codex_bridge",
                "leader_pid": 7777,
                "state": "running",
            })
            .to_string(),
        )
        .unwrap();

        let candidates = collect_pid_candidates(&agent_dir, None, false);
        assert_eq!(candidates.get(&7777), Some(&vec![helper_path]));
    }

    #[test]
    fn collect_project_process_candidates_skips_current_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(project_root.join(".ccb")).unwrap();

        let current_pid = std::process::id();
        let cmdline_map: HashMap<u32, String> = [(current_pid, "/project/.ccb marker".into())]
            .into_iter()
            .collect();
        let proc_root = tmp.path().join("proc");
        fs::create_dir(&proc_root).unwrap();

        let candidates = collect_project_process_candidates(
            &project_root,
            &proc_root,
            |pid| cmdline_map.get(&pid).cloned().unwrap_or_default(),
            Some(current_pid),
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn collect_project_authority_pid_candidates_reads_lease() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(project_root.join(".ccb/ccbd")).unwrap();
        let lease_path = project_root.join(".ccb/ccbd/lease.json");
        fs::write(
            &lease_path,
            serde_json::json!({"ccbd_pid": 1111, "keeper_pid": 2222}).to_string(),
        )
        .unwrap();

        let candidates = collect_project_authority_pid_candidates(&project_root);
        assert_eq!(candidates.get(&1111), Some(&vec![lease_path.clone()]));
        assert_eq!(candidates.get(&2222), Some(&vec![lease_path]));
    }

    #[test]
    fn control_plane_marker_matches_project_arg() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(project_root.join(".ccb")).unwrap();
        let layout = build_layout(&project_root).unwrap();
        let cmdline = format!(
            "python ccbd/main.py --project {} --daemon",
            project_root.to_string_lossy()
        );
        let marker = control_plane_marker(&project_root, &cmdline, &layout);
        assert_eq!(marker, Some(layout.ccbd_dir().into()));
    }

    #[test]
    fn control_plane_marker_rejects_wrong_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(project_root.join(".ccb")).unwrap();
        let layout = build_layout(&project_root).unwrap();
        let cmdline = "python ccbd/main.py --project /other/project --daemon".to_string();
        let marker = control_plane_marker(&project_root, &cmdline, &layout);
        assert_eq!(marker, None);
    }

    #[test]
    fn cmdline_has_project_arg_equals_form() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let expected = resolved_project_text(&project_root);
        let cmdline = format!(
            "python ccbd/main.py --project={}",
            project_root.to_string_lossy()
        );
        assert!(cmdline_has_project_arg(&cmdline, &project_root));
        assert_eq!(
            resolved_project_text(Path::new(&cmdline.split('=').nth(1).unwrap())),
            expected
        );
    }
}
