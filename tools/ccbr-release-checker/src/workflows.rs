use crate::{
    eprintln, git_output, run, Report, BRANCH_VALIDATION_WORKFLOWS, DEV_ALWAYS_REQUIRED_WORKFLOWS,
    DEV_DEFAULT_BRANCH_WORKFLOWS, RELEASE_RUN_LIMIT,
};
use camino::Utf8Path;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub type GithubAuthCheck<'a> = &'a dyn Fn(&Utf8Path, &mut Report) -> bool;
pub type RepoDefaultBranch<'a> = &'a dyn Fn(&Utf8Path, &str, &mut Report) -> String;

pub fn read_github_runs(root: &Utf8Path, repo: &str, limit: usize) -> Option<Vec<Value>> {
    let output = run(
        &[
            "gh",
            "run",
            "list",
            "--repo",
            repo,
            "--limit",
            &limit.to_string(),
            "--json",
            "name,status,conclusion,headBranch,event,databaseId,url,headSha",
        ],
        root,
    );
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

pub fn required_dev_workflows(branch: &str, default_branch: &str) -> Vec<String> {
    let mut required: Vec<String> = DEV_ALWAYS_REQUIRED_WORKFLOWS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    if branch == default_branch || branch == "main" || branch == "dev" {
        for w in DEV_DEFAULT_BRANCH_WORKFLOWS {
            required.push((*w).to_string());
        }
    }
    required
}

pub fn check_dev_branch_workflows(
    root: &Utf8Path,
    repo: &str,
    wait_seconds: u64,
    poll_interval: u64,
    report: &mut Report,
    gh_auth_is_ready_fn: GithubAuthCheck<'_>,
    repo_default_branch_fn: RepoDefaultBranch<'_>,
) {
    if !gh_auth_is_ready_fn(root, report) {
        return;
    }

    let branch = git_output(root, &["branch", "--show-current"]).unwrap_or_default();
    let head = git_output(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let upstream_head = git_output(root, &["rev-parse", "@{u}"]).unwrap_or_default();
    if branch.is_empty() || head.is_empty() {
        report
            .warn("Could not determine current branch/head; GitHub workflow state was not checked");
        return;
    }
    if !upstream_head.is_empty() && head != upstream_head {
        report.warn("Current HEAD is not pushed to upstream; GitHub workflow state for this commit cannot be complete yet");
        return;
    }

    let default_branch = repo_default_branch_fn(root, repo, report);
    let required = required_dev_workflows(&branch, &default_branch);
    if !required.contains(&"Cross-Platform Compatibility Test".to_string()) {
        report.warn(format!(
            "Cross-Platform Compatibility Test is not required for branch {branch:?}; it only runs on main/dev, PRs, or manual dispatch"
        ));
    }

    let deadline = Instant::now() + Duration::from_secs(wait_seconds);
    let mut latest_by_name: HashMap<String, Value> = HashMap::new();
    let mut last_wait_status = String::new();
    loop {
        let run_payload = read_github_runs(root, repo, RELEASE_RUN_LIMIT);
        let Some(run_payload) = run_payload else {
            report.fail(
                "Could not read GitHub Actions runs for dev workflow check",
                Some("retry after GitHub/API connectivity recovers; final dev verification requires workflow status"),
            );
            return;
        };

        latest_by_name.clear();
        for item in run_payload {
            let item_head = item
                .get("headSha")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if item_head != head {
                continue;
            }
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if required.contains(&name) && !latest_by_name.contains_key(&name) {
                latest_by_name.insert(name, item);
            }
        }

        let all_done = required.iter().all(|name| {
            latest_by_name
                .get(name)
                .and_then(|item| item.get("status").and_then(|v| v.as_str()))
                == Some("completed")
        });

        if all_done || wait_seconds == 0 || Instant::now() >= deadline {
            break;
        }

        let wait_status = format_workflow_wait_status(&latest_by_name, &required);
        if wait_status != last_wait_status {
            eprintln(format!("Waiting for dev workflows: {wait_status}"));
            last_wait_status = wait_status;
        }
        std::thread::sleep(Duration::from_secs(poll_interval.max(1)));
    }

    for workflow_name in sorted(&required) {
        let item = latest_by_name.get(&workflow_name);
        let Some(item) = item else {
            report.fail(
                format!("No GitHub Actions run found for current commit {}: {workflow_name}", &head[..head.len().min(12)]),
                Some("push the branch and wait for GitHub Actions, or confirm this workflow is intentionally not triggered"),
            );
            continue;
        };
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let conclusion = item
            .get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if status != "completed" || conclusion != "success" {
            report.fail(
                format!("GitHub Actions {workflow_name} for current commit is {status}/{conclusion}: {url}"),
                Some("wait for the run to complete or fix/rerun it; use --wait-seconds to let the checker wait automatically"),
            );
        }
    }
}

pub fn format_workflow_wait_status(
    workflows: &HashMap<String, Value>,
    required: &[String],
) -> String {
    let parts: Vec<String> = sorted(required)
        .iter()
        .map(|name| {
            if let Some(item) = workflows.get(name) {
                let status = item
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let conclusion = item
                    .get("conclusion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                format!("{name}={status}/{conclusion}")
            } else {
                format!("{name}=missing")
            }
        })
        .collect();
    parts.join(", ")
}

pub fn check_branch_validation_runs(run_payload: &[Value], tag_commit: &str, report: &mut Report) {
    if tag_commit.is_empty() {
        return;
    }
    let mut found: Vec<String> = Vec::new();
    for workflow_name in BRANCH_VALIDATION_WORKFLOWS {
        let candidates: Vec<&Value> = run_payload
            .iter()
            .filter(|item| {
                item.get("name").and_then(|v| v.as_str()) == Some(*workflow_name)
                    && item.get("headSha").and_then(|v| v.as_str()) == Some(tag_commit)
                    && item
                        .get("headBranch")
                        .and_then(|v| v.as_str())
                        .map(|b| !b.is_empty() && !b.starts_with('v'))
                        .unwrap_or(false)
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }
        found.push((*workflow_name).to_string());
        let latest = candidates[0];
        let status = latest
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let conclusion = latest
            .get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let url = latest.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if status != "completed" || conclusion != "success" {
            report.warn(format!(
                "Branch validation workflow {workflow_name} for release commit is {status}/{conclusion}: {url}"
            ));
        }
    }
    let found_set: std::collections::HashSet<_> = found.iter().cloned().collect();
    let missing: Vec<_> = BRANCH_VALIDATION_WORKFLOWS
        .iter()
        .filter(|w| !found_set.contains(**w))
        .copied()
        .collect();
    if !missing.is_empty() {
        report.warn(format!(
            "No recent branch validation run found for release commit for: {}",
            missing.join(", ")
        ));
    }
}

fn sorted(items: &[String]) -> Vec<String> {
    let mut items = items.to_vec();
    items.sort();
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_dev_workflows() {
        let main = required_dev_workflows("main", "main");
        assert!(main.contains(&"Tests".to_string()));
        assert!(main.contains(&"Cross-Platform Compatibility Test".to_string()));
        let feature = required_dev_workflows("feature", "main");
        assert!(feature.contains(&"Tests".to_string()));
        assert!(!feature.contains(&"Cross-Platform Compatibility Test".to_string()));
    }

    #[test]
    fn test_format_workflow_wait_status() {
        let mut map = HashMap::new();
        map.insert(
            "Tests".to_string(),
            serde_json::json!({"status": "completed", "conclusion": "success"}),
        );
        let required = vec!["Tests".to_string(), "Missing".to_string()];
        assert_eq!(
            format_workflow_wait_status(&map, &required),
            "Missing=missing, Tests=completed/success"
        );
    }
}
