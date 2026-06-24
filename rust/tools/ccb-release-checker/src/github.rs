use crate::{
    assets::{
        check_sha256sums, published_wait_status as assets_published_wait_status,
        read_published_release_state, release_workflow_candidates,
    },
    git_output,
    markdown::{
        has_substantive_release_text, install_section, readme_release_block, release_note_versions,
        semver_tuple,
    },
    run,
    workflows::{
        check_branch_validation_runs, check_dev_branch_workflows, format_workflow_wait_status,
        read_github_runs, required_dev_workflows,
    },
    Report, EXPECTED_ASSETS, REQUIRED_TAG_WORKFLOWS,
};
use camino::Utf8Path;
use regex::Regex;
use serde_json::Value;

pub fn check_readme_surface(
    body: &str,
    readme_name: &str,
    version: &str,
    repo: &str,
    source: &str,
    report: &mut Report,
) {
    let bare_version = version.strip_prefix('v').unwrap_or(version);
    let versions = release_note_versions(body);
    if !versions.is_empty() {
        if versions[0] != version {
            report.fail(
                format!(
                    "{source} {readme_name} first release notes entry is {}, expected {version}",
                    versions[0]
                ),
                Some("merge/push the release documentation changes to the default branch"),
            );
        }
        let sorted_versions = {
            let mut v = versions.clone();
            v.sort_by_key(|x| std::cmp::Reverse(semver_tuple(x)));
            v
        };
        if versions != sorted_versions {
            report.warn(format!(
                "{source} {readme_name} release notes are not in descending semver order"
            ));
        }
    } else {
        report.fail(
            format!("{source} {readme_name} has no release notes version entries"),
            Some("merge/push a README with current release notes to the default branch"),
        );
    }

    if !body.contains(&format!("version-{bare_version}-orange.svg")) {
        report.fail(
            format!("{source} {readme_name} version badge does not show {bare_version}"),
            Some("merge/push the release README badge update to the default branch"),
        );
    }
    if !body.contains(&format!("<summary><b>{version}</b>")) {
        report.fail(
            format!("{source} {readme_name} release notes do not include {version}"),
            Some("merge/push release notes for the current version to the default branch"),
        );
    } else if !has_substantive_release_text(readme_release_block(body, version).as_deref()) {
        report.fail(
            format!("{source} {readme_name} release notes entry for {version} is empty"),
            Some("add concrete release bullets before calling the homepage updated"),
        );
    }
    if !body.contains(".ccbr/ccb_memory.md") {
        report.fail(
            format!("{source} {readme_name} does not mention .ccbr/ccb_memory.md"),
            Some("keep the shared memory wording in the default-branch README"),
        );
    }

    let (owner, name) = repo
        .split_once('/')
        .unwrap_or(("SeemSeam", "claude_codex_bridge"));
    let expected_clone = format!("https://github.com/{owner}/{name}.git");
    let install_heading = if readme_name == "README_zh.md" {
        "如何安装"
    } else {
        "How to Install"
    };
    let install_body = install_section(body, install_heading);
    let re = Regex::new(r"git\s+clone\s+(https://github\.com/[^\s`]+\.git)").unwrap();
    let clone_urls: Vec<String> = re
        .captures_iter(&install_body)
        .map(|caps| caps[1].to_string())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let wrong_urls: Vec<_> = clone_urls
        .iter()
        .filter(|url| *url != &expected_clone)
        .cloned()
        .collect();
    if !wrong_urls.is_empty() {
        report.fail(
            format!(
                "{source} {readme_name} has clone URL(s) not matching {expected_clone}: {}",
                wrong_urls.join(", ")
            ),
            Some(&format!(
                "replace default-branch README install clone URLs with {expected_clone}"
            )),
        );
    }

    if body.contains("CCB.md") {
        report.fail(
            format!("{source} {readme_name} mentions current CCB.md support"),
            Some("default-branch README should describe only .ccbr/ccb_memory.md as current shared memory"),
        );
    }
}

pub fn gh_api_text(root: &Utf8Path, path: &str) -> Option<String> {
    let output = run(&["gh", "api", path, "--jq", ".content"], root);
    if !output.status.success() {
        return None;
    }
    let encoded = String::from_utf8_lossy(&output.stdout);
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(encoded.trim())
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}

pub fn check_remote_homepage(
    root: &Utf8Path,
    version: &str,
    repo: &str,
    default_branch: &str,
    report: &mut Report,
) {
    if default_branch.is_empty() {
        report.warn("Could not determine GitHub default branch; homepage README was not checked");
        return;
    }
    for readme_name in ["README.md", "README_zh.md"] {
        let body = gh_api_text(
            root,
            &format!("repos/{repo}/contents/{readme_name}?ref={default_branch}"),
        );
        let Some(body) = body else {
            report.fail(
                format!("Could not read {readme_name} from GitHub default branch {default_branch}"),
                Some("confirm gh auth/repo access and that the default branch contains the README"),
            );
            continue;
        };
        check_readme_surface(
            &body,
            readme_name,
            version,
            repo,
            &format!("GitHub default branch {default_branch}"),
            report,
        );
    }
}

pub fn check_default_branch_contains_release(
    root: &Utf8Path,
    version: &str,
    repo: &str,
    default_branch: &str,
    report: &mut Report,
) {
    if default_branch.is_empty() {
        report.warn(
            "Could not determine GitHub default branch; default-branch containment was not checked",
        );
        return;
    }
    let output = run(
        &[
            "gh",
            "api",
            &format!("repos/{repo}/compare/{version}...{default_branch}"),
            "--jq",
            ".status",
        ],
        root,
    );
    if !output.status.success() {
        report.fail(
            format!("Could not compare release tag {version} with default branch {default_branch}"),
            Some("confirm the tag exists on GitHub, then merge the release commit into the default branch if needed"),
        );
        return;
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if status != "identical" && status != "ahead" {
        report.fail(
            format!("GitHub default branch {default_branch} does not contain release tag {version} (compare status: {})", if status.is_empty() { "unknown" } else { &status }),
            Some(&format!("merge the release commit/tag into {default_branch} and push; GitHub homepage README only renders from the default branch")),
        );
    }
}

pub fn gh_auth_is_ready(root: &Utf8Path, report: &mut Report) -> bool {
    let auth = run(&["gh", "auth", "status", "--hostname", "github.com"], root);
    if !auth.status.success() {
        report.fail(
            "GitHub CLI is not authenticated",
            Some("run gh auth login, then rerun the GitHub state check"),
        );
        return false;
    }
    true
}

pub fn repo_default_branch(root: &Utf8Path, repo: &str, report: &mut Report) -> String {
    let output = run(
        &["gh", "repo", "view", repo, "--json", "defaultBranchRef"],
        root,
    );
    if !output.status.success() {
        report.warn(format!(
            "Could not read GitHub default branch: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
        return String::new();
    }
    let payload: Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            report.warn(format!("Could not parse gh repo JSON: {e}"));
            return String::new();
        }
    };
    payload
        .get("defaultBranchRef")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn check_dev_branch_workflows_wrapper(
    root: &Utf8Path,
    repo: &str,
    wait_seconds: u64,
    poll_interval: u64,
    report: &mut Report,
) {
    check_dev_branch_workflows(
        root,
        repo,
        wait_seconds,
        poll_interval,
        report,
        &gh_auth_is_ready,
        &repo_default_branch,
    );
}

fn _published_wait_status(
    release_payload: Option<&Value>,
    run_payload: Option<&[Value]>,
    version: &str,
    tag_commit: &str,
) -> String {
    assets_published_wait_status(
        release_payload,
        run_payload,
        version,
        tag_commit,
        &format_workflow_wait_status,
    )
}

fn read_published_release_state_wrapper(
    root: &Utf8Path,
    version: &str,
    repo: &str,
    tag_commit: &str,
    wait_seconds: u64,
    poll_interval: u64,
    report: &mut Report,
) -> (Option<Value>, Option<Vec<Value>>) {
    read_published_release_state(
        root,
        version,
        repo,
        tag_commit,
        wait_seconds,
        poll_interval,
        report,
        &read_github_runs,
        &format_workflow_wait_status,
    )
}

pub fn check_github(
    root: &Utf8Path,
    version: &str,
    repo: &str,
    report: &mut Report,
    wait_seconds: u64,
    poll_interval: u64,
) {
    if !gh_auth_is_ready(root, report) {
        return;
    }

    let tag_commit = git_output(root, &["rev-list", "-n", "1", version]).unwrap_or_default();
    let (payload, run_payload) = read_published_release_state_wrapper(
        root,
        version,
        repo,
        &tag_commit,
        wait_seconds,
        poll_interval,
        report,
    );
    let Some(payload) = payload else {
        return;
    };

    if payload.get("tagName").and_then(|v| v.as_str()) != Some(version) {
        report.fail(
            format!(
                "GitHub release tag is {:?}, expected {:?}",
                payload
                    .get("tagName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                version
            ),
            None,
        );
    }
    if payload
        .get("isDraft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        report.fail(
            format!("GitHub release {version} is still a draft"),
            Some("publish the draft after assets and notes are ready"),
        );
    }

    let asset_names: std::collections::HashSet<String> = payload
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|a| a.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let expected: std::collections::HashSet<String> =
        EXPECTED_ASSETS.iter().map(|s| (*s).to_string()).collect();
    let missing: Vec<_> = expected.difference(&asset_names).cloned().collect();
    if !missing.is_empty() {
        report.fail(
            format!("GitHub release missing asset(s): {}", missing.join(", ")),
            Some(&format!(
                "rerun Release Artifacts for {version}, then verify assets again"
            )),
        );
    } else if asset_names.contains("SHA256SUMS") {
        check_sha256sums(root, version, repo, report);
    }

    let mut default_branch = String::new();
    let repo_view = run(
        &[
            "gh",
            "repo",
            "view",
            repo,
            "--json",
            "description,repositoryTopics,latestRelease,url,defaultBranchRef",
        ],
        root,
    );
    if !repo_view.status.success() {
        report.warn(format!(
            "Could not read GitHub repo metadata: {}",
            String::from_utf8_lossy(&repo_view.stderr).trim()
        ));
    } else {
        match serde_json::from_slice::<Value>(&repo_view.stdout) {
            Ok(repo_payload) => {
                let latest = repo_payload
                    .get("latestRelease")
                    .and_then(|v| v.get("tagName"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                default_branch = repo_payload
                    .get("defaultBranchRef")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if latest != version {
                    report.fail(
                        format!("GitHub latest release is {latest:?}, expected {version:?}"),
                        Some("publish the GitHub release and ensure it is not draft/prerelease unless intended"),
                    );
                }
                let description = repo_payload
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if description.contains("Claude, Codex & Gemini")
                    && !description.contains("OpenCode")
                {
                    report.warn("GitHub description may be stale: it mentions Claude/Codex/Gemini but not newer supported providers");
                }
            }
            Err(e) => {
                report.warn(format!("Could not parse gh repo JSON: {e}"));
            }
        }
    }

    check_default_branch_contains_release(root, version, repo, &default_branch, report);
    check_remote_homepage(root, version, repo, &default_branch, report);

    let Some(run_payload) = run_payload else {
        return;
    };

    for workflow_name in REQUIRED_TAG_WORKFLOWS {
        let candidates =
            release_workflow_candidates(&run_payload, workflow_name, version, &tag_commit);
        let successes: Vec<&Value> = candidates
            .iter()
            .filter(|item| {
                item.get("status").and_then(|v| v.as_str()) == Some("completed")
                    && item.get("conclusion").and_then(|v| v.as_str()) == Some("success")
            })
            .collect();
        if let Some(accepted) = successes.first() {
            let event = accepted
                .get("event")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let head_sha = accepted
                .get("headSha")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if event == "workflow_dispatch" && !tag_commit.is_empty() && head_sha != tag_commit {
                report.warn(format!(
                    "{workflow_name} was accepted from workflow_dispatch but its headSha {head_sha} does not match tag {tag_commit}; confirm it used input tag={version}"
                ));
            }
            continue;
        }
        if let Some(latest) = candidates.first() {
            let status = latest
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let conclusion = latest
                .get("conclusion")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let url = latest.get("url").and_then(|v| v.as_str()).unwrap_or("");
            report.fail(
                format!("GitHub Actions {workflow_name} for {version} is {status}/{conclusion}: {url}"),
                Some("open the run, fix the root cause, rerun the failed workflow, and do not call the release complete while red"),
            );
            continue;
        }
        report.fail(
            format!("Missing release workflow run for {version}: {workflow_name}"),
            Some(&format!(
                "push tag {version} or manually dispatch {workflow_name} with input tag={version}"
            )),
        );
    }

    check_branch_validation_runs(&run_payload, &tag_commit, report);
}

pub fn required_dev_workflows_for_cli(branch: &str, default_branch: &str) -> Vec<String> {
    required_dev_workflows(branch, default_branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_readme_surface_basic() {
        let body = r#"<img src="version-1.0.0-orange.svg">
<details>
<summary><b>v1.0.0</b> - Release</summary>
- Fixed bug
</details>
.ccbr/ccb_memory.md
## How to Install
```bash
git clone https://github.com/SeemSeam/claude_codex_bridge.git
```
"#;
        let mut report = Report::default();
        check_readme_surface(
            body,
            "README.md",
            "v1.0.0",
            "SeemSeam/claude_codex_bridge",
            "local",
            &mut report,
        );
        assert!(!report.has_issues(), "{:#?}", report.issues);
    }

    #[test]
    fn test_check_readme_surface_ccb_md_fail() {
        let body = "version-1.0.0-orange.svg\n<summary><b>v1.0.0</b></summary>\n- x\n.ccbr/ccb_memory.md\nCCB.md";
        let mut report = Report::default();
        check_readme_surface(
            body,
            "README.md",
            "v1.0.0",
            "SeemSeam/claude_codex_bridge",
            "local",
            &mut report,
        );
        assert!(report.has_issues());
    }
}
