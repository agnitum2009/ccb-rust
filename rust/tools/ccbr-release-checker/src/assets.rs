use crate::{
    eprintln, run, Report, CHECKSUMMED_ASSETS, EXPECTED_ASSETS, RELEASE_RUN_LIMIT,
    REQUIRED_TAG_WORKFLOWS,
};
use camino::Utf8Path;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub type GithubRunsReader<'a> = &'a dyn Fn(&Utf8Path, &str, usize) -> Option<Vec<Value>>;
pub type WorkflowStatusFormatter<'a> = &'a dyn Fn(&HashMap<String, Value>, &[String]) -> String;

pub fn check_sha256sums(root: &Utf8Path, version: &str, repo: &str, report: &mut Report) {
    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(_) => {
            report.fail(
                "Could not create temporary directory for SHA256SUMS download",
                None,
            );
            return;
        }
    };
    let tmp_path = tmp.path().to_string_lossy();
    let output = run(
        &[
            "gh",
            "release",
            "download",
            version,
            "--repo",
            repo,
            "--pattern",
            "SHA256SUMS",
            "--dir",
            &tmp_path,
        ],
        root,
    );
    if !output.status.success() {
        report.fail(
            "Could not download SHA256SUMS from the GitHub release",
            Some("rerun Release Artifacts or re-upload SHA256SUMS, then rerun the published check"),
        );
        return;
    }

    let sums_path = tmp.path().join("SHA256SUMS");
    let payload = std::fs::read_to_string(&sums_path).unwrap_or_default();
    let re = regex::Regex::new(r"^[0-9a-fA-F]{64}$").unwrap();
    let mut found: HashMap<String, String> = HashMap::new();
    for line in payload.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }
        let (digest, name) = (parts[0], parts[1]);
        if re.is_match(digest) {
            let basename = std::path::Path::new(name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(name)
                .to_string();
            found.insert(basename, digest.to_lowercase());
        }
    }

    let expected: HashSet<String> = CHECKSUMMED_ASSETS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let found_keys: HashSet<String> = found.keys().cloned().collect();
    let missing: Vec<_> = expected.difference(&found_keys).cloned().collect();
    if !missing.is_empty() {
        report.fail(
            format!(
                "SHA256SUMS is missing checksum entry/entries for: {}",
                missing.join(", ")
            ),
            Some("rerun Release Artifacts so SHA256SUMS is regenerated from the uploaded tarballs"),
        );
    }
    let extra: Vec<_> = found_keys.difference(&expected).cloned().collect();
    if !extra.is_empty() {
        report.warn(format!(
            "SHA256SUMS contains unexpected extra asset checksum(s): {}",
            extra.join(", ")
        ));
    }
}

pub fn read_release_payload(root: &Utf8Path, version: &str, repo: &str) -> (Option<Value>, String) {
    let output = run(
        &[
            "gh",
            "release",
            "view",
            version,
            "--repo",
            repo,
            "--json",
            "tagName,url,assets,isDraft",
        ],
        root,
    );
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return (None, if err.is_empty() { out } else { err });
    }
    match serde_json::from_slice(&output.stdout) {
        Ok(v) => (Some(v), String::new()),
        Err(e) => (None, format!("Could not parse gh release JSON: {e}")),
    }
}

pub fn release_artifacts_run_matches(item: &Value, version: &str, tag_commit: &str) -> bool {
    if item.get("headBranch").and_then(|v| v.as_str()) == Some(version) {
        return true;
    }
    if !tag_commit.is_empty() && item.get("headSha").and_then(|v| v.as_str()) == Some(tag_commit) {
        return true;
    }
    false
}

pub fn release_workflow_candidates(
    run_payload: &[Value],
    workflow_name: &str,
    version: &str,
    tag_commit: &str,
) -> Vec<Value> {
    run_payload
        .iter()
        .filter(|item| {
            item.get("name").and_then(|v| v.as_str()) == Some(workflow_name)
                && release_artifacts_run_matches(item, version, tag_commit)
        })
        .cloned()
        .collect()
}

pub fn latest_release_workflows(
    run_payload: &[Value],
    version: &str,
    tag_commit: &str,
) -> HashMap<String, Value> {
    let mut latest = HashMap::new();
    for workflow_name in REQUIRED_TAG_WORKFLOWS {
        let candidates =
            release_workflow_candidates(run_payload, workflow_name, version, tag_commit);
        if let Some(first) = candidates.first() {
            latest.insert((*workflow_name).to_string(), first.clone());
        }
    }
    latest
}

pub fn published_state_is_pending(
    release_payload: Option<&Value>,
    run_payload: Option<&[Value]>,
    version: &str,
    tag_commit: &str,
) -> bool {
    let (Some(release_payload), Some(run_payload)) = (release_payload, run_payload) else {
        return false;
    };
    let latest = latest_release_workflows(run_payload, version, tag_commit);
    for workflow_name in REQUIRED_TAG_WORKFLOWS {
        if let Some(item) = latest.get(*workflow_name) {
            let status = item.get("status").and_then(|v| v.as_str());
            let conclusion = item.get("conclusion").and_then(|v| v.as_str());
            if status == Some("completed") && conclusion != Some("success") {
                return false;
            }
        }
    }
    let asset_names: HashSet<String> = release_payload
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|a| a.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let expected: HashSet<String> = EXPECTED_ASSETS.iter().map(|s| (*s).to_string()).collect();
    if !expected.is_subset(&asset_names) {
        return true;
    }
    for workflow_name in REQUIRED_TAG_WORKFLOWS {
        let item = latest.get(*workflow_name);
        if item.is_none() {
            return true;
        }
        let status = item.and_then(|i| i.get("status").and_then(|v| v.as_str()));
        if status != Some("completed") {
            return true;
        }
    }
    false
}

pub fn published_wait_status(
    release_payload: Option<&Value>,
    run_payload: Option<&[Value]>,
    version: &str,
    tag_commit: &str,
    format_workflow_wait_status_fn: WorkflowStatusFormatter<'_>,
) -> String {
    let release_payload = match release_payload {
        Some(p) => p,
        None => return "release=missing".to_string(),
    };
    let asset_names: HashSet<String> = release_payload
        .get("assets")
        .and_then(|v| v.as_array())
        .map(|assets| {
            assets
                .iter()
                .filter_map(|a| a.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let expected: HashSet<String> = EXPECTED_ASSETS.iter().map(|s| (*s).to_string()).collect();
    let missing_assets: Vec<_> = expected.difference(&asset_names).cloned().collect();
    let latest = latest_release_workflows(run_payload.unwrap_or_default(), version, tag_commit);
    let required: Vec<String> = REQUIRED_TAG_WORKFLOWS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let workflows = format_workflow_wait_status_fn(&latest, &required);
    let assets = if missing_assets.is_empty() {
        "assets=ready".to_string()
    } else {
        format!("assets=missing({})", missing_assets.join(","))
    };
    format!("{assets}; {workflows}")
}

#[allow(clippy::too_many_arguments)]
pub fn read_published_release_state(
    root: &Utf8Path,
    version: &str,
    repo: &str,
    tag_commit: &str,
    wait_seconds: u64,
    poll_interval: u64,
    report: &mut Report,
    read_github_runs_fn: GithubRunsReader<'_>,
    format_workflow_wait_status_fn: WorkflowStatusFormatter<'_>,
) -> (Option<Value>, Option<Vec<Value>>) {
    let deadline = Instant::now() + Duration::from_secs(wait_seconds);
    let mut last_wait_status = String::new();
    loop {
        let (release_payload, release_error) = read_release_payload(root, version, repo);
        if release_payload.is_none() {
            report.fail(
                format!("GitHub release {version} not found for {repo}: {release_error}"),
                Some(&format!(
                    "create the release page first: gh release create {version} --repo {repo} --title {version} --notes-file <notes-file>"
                )),
            );
            return (None, None);
        }

        let run_payload = read_github_runs_fn(root, repo, RELEASE_RUN_LIMIT);
        if run_payload.is_none() {
            report.fail(
                "Could not read GitHub Actions runs",
                Some("retry after GitHub/API connectivity recovers; final release verification requires workflow status"),
            );
            return (release_payload, None);
        }

        let pending = published_state_is_pending(
            release_payload.as_ref(),
            run_payload.as_deref(),
            version,
            tag_commit,
        );
        if !pending || wait_seconds == 0 || Instant::now() >= deadline {
            return (release_payload, run_payload);
        }

        let wait_status = published_wait_status(
            release_payload.as_ref(),
            run_payload.as_deref(),
            version,
            tag_commit,
            format_workflow_wait_status_fn,
        );
        if wait_status != last_wait_status {
            eprintln(format!(
                "Waiting for published release state: {wait_status}"
            ));
            last_wait_status = wait_status;
        }
        std::thread::sleep(Duration::from_secs(poll_interval.max(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(status: &str, conclusion: &str, head_branch: &str, head_sha: &str) -> Value {
        serde_json::json!({
            "name": "Release Artifacts",
            "status": status,
            "conclusion": conclusion,
            "headBranch": head_branch,
            "headSha": head_sha,
            "event": "push",
            "url": "https://example.com/run",
        })
    }

    #[test]
    fn test_release_artifacts_run_matches() {
        let v = item("completed", "success", "v1.0.0", "abc");
        assert!(release_artifacts_run_matches(&v, "v1.0.0", "def"));
        assert!(release_artifacts_run_matches(&v, "v2.0.0", "abc"));
        assert!(!release_artifacts_run_matches(&v, "v2.0.0", "def"));
    }

    #[test]
    fn test_published_state_is_pending() {
        let release = serde_json::json!({
            "tagName": "v1.0.0",
            "assets": [
                {"name": "ccbr-linux-x86_64.tar.gz"},
                {"name": "ccbr-macos-universal.tar.gz"},
                {"name": "SHA256SUMS"},
            ]
        });
        let runs = vec![item("completed", "success", "v1.0.0", "abc")];
        assert!(!published_state_is_pending(
            Some(&release),
            Some(&runs),
            "v1.0.0",
            "abc"
        ));
        // Missing workflow
        assert!(published_state_is_pending(
            Some(&release),
            Some(&[]),
            "v1.0.0",
            "abc"
        ));
    }
}
