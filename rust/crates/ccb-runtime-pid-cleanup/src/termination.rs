//! Runtime PID termination orchestration.
//!
//! Mirrors Python `runtime_pid_cleanup.termination`.

use std::collections::HashMap;
use std::path::PathBuf;

use camino::Utf8Path;

use ccb_providers::runtime::terminate_helper_manifest_path;

/// Terminate runtime PIDs discovered for a project.
///
/// Mirrors Python `runtime_pid_cleanup.termination.terminate_runtime_pids`.
#[allow(clippy::too_many_arguments)]
pub fn terminate_runtime_pids(
    project_root: &std::path::Path,
    pid_candidates: &HashMap<u32, Vec<PathBuf>>,
    is_pid_alive_fn: impl Fn(u32) -> bool,
    pid_matches_project_fn: impl Fn(u32, &std::path::Path, &[PathBuf]) -> bool,
    terminate_pid_tree_fn: impl Fn(u32, f64, &dyn Fn(u32) -> bool) -> bool,
    remove_pid_files_fn: impl Fn(&[PathBuf]),
    collect_project_process_candidates_fn: Option<
        impl Fn(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>,
    >,
) {
    let mut merged_candidates: HashMap<u32, Vec<PathBuf>> = pid_candidates
        .iter()
        .map(|(pid, paths)| (*pid, paths.clone()))
        .collect();
    if let Some(collect_fn) = collect_project_process_candidates_fn {
        for (pid, sources) in collect_fn(project_root) {
            merged_candidates.entry(pid).or_default().extend(sources);
        }
    }

    let mut pids: Vec<u32> = merged_candidates.keys().copied().collect();
    pids.sort_unstable();

    for pid in pids {
        let hint_paths = dedupe_paths(&merged_candidates[&pid]);
        let has_helper_manifest = hint_paths
            .iter()
            .any(|path| path.file_name().and_then(|n| n.to_str()) == Some("helper.json"));
        let helper_reaped =
            terminate_helper_groups(&hint_paths, &is_pid_alive_fn, &terminate_pid_tree_fn);
        if !is_pid_alive_fn(pid) {
            if helper_reaped || !has_helper_manifest {
                remove_pid_files_fn(&hint_paths);
            }
            continue;
        }
        if !pid_matches_project_fn(pid, project_root, &hint_paths) {
            continue;
        }
        if terminate_pid_tree_fn(pid, 1.0, &is_pid_alive_fn) {
            remove_pid_files_fn(&hint_paths);
        }
    }
}

#[allow(clippy::type_complexity)]
fn terminate_helper_groups(
    hint_paths: &[PathBuf],
    _is_pid_alive_fn: &dyn Fn(u32) -> bool,
    _terminate_pid_tree_fn: &dyn Fn(u32, f64, &dyn Fn(u32) -> bool) -> bool,
) -> bool {
    let mut reaped = false;
    for path in hint_paths {
        if path.file_name().and_then(|n| n.to_str()) != Some("helper.json") {
            continue;
        }
        let utf8_path = match Utf8Path::from_path(path) {
            Some(p) => p,
            None => continue,
        };
        if terminate_helper_manifest_path(utf8_path) {
            reaped = true;
        }
    }
    reaped
}

fn dedupe_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if !result.contains(path) {
            result.push(path.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;

    #[test]
    fn terminate_runtime_pids_removes_pid_files_for_dead_pids() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let pid_path = tmp.path().join("test.pid");
        fs::write(&pid_path, "42").unwrap();

        let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
        candidates.insert(42, vec![pid_path.clone()]);

        let removed = Rc::new(RefCell::new(Vec::new()));
        let removed_clone = removed.clone();

        terminate_runtime_pids(
            &project_root,
            &candidates,
            |_pid| false,
            |_pid, _root, _hints| true,
            |_pid, _timeout, _alive| false,
            move |paths| {
                removed_clone.borrow_mut().extend(paths.iter().cloned());
            },
            None::<fn(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>>,
        );

        assert!(removed.borrow().contains(&pid_path));
    }

    #[test]
    fn terminate_runtime_pids_skips_pids_that_do_not_match_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let pid_path = tmp.path().join("test.pid");
        fs::write(&pid_path, "42").unwrap();

        let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
        candidates.insert(42, vec![pid_path.clone()]);

        let terminated = Rc::new(RefCell::new(Vec::new()));
        let terminated_clone = terminated.clone();
        let removed = Rc::new(RefCell::new(Vec::new()));
        let removed_clone = removed.clone();

        terminate_runtime_pids(
            &project_root,
            &candidates,
            |_pid| true,
            |_pid, _root, _hints| false,
            move |pid, _timeout, _alive| {
                terminated_clone.borrow_mut().push(pid);
                false
            },
            move |paths| {
                removed_clone.borrow_mut().extend(paths.iter().cloned());
            },
            None::<fn(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>>,
        );

        assert!(terminated.borrow().is_empty());
        assert!(removed.borrow().is_empty());
    }

    #[test]
    fn terminate_runtime_pids_terminates_and_removes_matching_pids() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let pid_path = tmp.path().join("test.pid");
        fs::write(&pid_path, "42").unwrap();

        let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
        candidates.insert(42, vec![pid_path.clone()]);

        let terminated = Rc::new(RefCell::new(Vec::new()));
        let terminated_clone = terminated.clone();
        let removed = Rc::new(RefCell::new(Vec::new()));
        let removed_clone = removed.clone();

        terminate_runtime_pids(
            &project_root,
            &candidates,
            |_pid| true,
            |_pid, _root, _hints| true,
            move |pid, timeout, _alive| {
                terminated_clone.borrow_mut().push((pid, timeout));
                true
            },
            move |paths| {
                removed_clone.borrow_mut().extend(paths.iter().cloned());
            },
            None::<fn(&std::path::Path) -> HashMap<u32, Vec<PathBuf>>>,
        );

        assert_eq!(terminated.borrow().clone(), vec![(42, 1.0)]);
        assert!(removed.borrow().contains(&pid_path));
    }

    #[test]
    fn terminate_runtime_pids_merges_collected_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();

        let mut candidates: HashMap<u32, Vec<PathBuf>> = HashMap::new();
        candidates.insert(1, vec![tmp.path().join("a.pid")]);

        let terminated = Rc::new(RefCell::new(Vec::new()));
        let terminated_clone = terminated.clone();

        terminate_runtime_pids(
            &project_root,
            &candidates,
            |_pid| true,
            |_pid, _root, _hints| true,
            move |pid, _timeout, _alive| {
                terminated_clone.borrow_mut().push(pid);
                true
            },
            |_paths| {},
            Some(move |_root: &std::path::Path| {
                let mut map = HashMap::new();
                map.insert(2, vec![tmp.path().join("b.pid")]);
                map
            }),
        );

        let mut terminated = terminated.borrow().clone();
        terminated.sort_unstable();
        assert_eq!(terminated, vec![1, 2]);
    }
}
