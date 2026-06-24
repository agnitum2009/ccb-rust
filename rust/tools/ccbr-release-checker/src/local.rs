use crate::{
    git_output, infer_repo, normalize_version, read, read_bytes, repo_root, run, Report,
    DEV_HOMEPAGE_PATHS, DEV_RELEASE_TRIGGER_PATHS, DEV_STRICT_PHASES,
};
use camino::Utf8Path;
use regex::Regex;

use crate::markdown::{
    has_substantive_release_text, install_section, markdown_section, readme_release_block,
    release_note_versions, semver_tuple,
};

pub const TRACKED_SKILL_FILES: &[&str] = &["SKILL.md", "agents/openai.yaml"];

pub fn check_local_git_state(root: &Utf8Path, phase: &str, report: &mut Report) {
    let status = git_output(root, &["status", "--porcelain"]).unwrap_or_default();
    if !status.is_empty() {
        let message = "Worktree has uncommitted changes";
        let fix = "commit or intentionally discard local changes before reporting a final dev or release result";
        if DEV_STRICT_PHASES.contains(&phase) {
            report.fail(message, Some(fix));
        } else {
            report.warn(format!("{message}; {fix}"));
        }
    }

    let branch = git_output(root, &["branch", "--show-current"]);
    let Some(branch) = branch else {
        report.warn("Detached HEAD; branch push/merge state cannot be checked");
        return;
    };

    let upstream = git_output(
        root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    );
    let Some(upstream) = upstream else {
        let message = format!("Current branch {branch} has no upstream");
        let fix = format!("push it with upstream tracking: git push -u origin {branch}");
        if DEV_STRICT_PHASES.contains(&phase) {
            report.fail(message, Some(&fix));
        } else {
            report.warn(format!("{message}; {fix}"));
        }
        return;
    };

    let local = git_output(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let remote = git_output(root, &["rev-parse", "@{u}"]).unwrap_or_default();
    let merge_base = git_output(root, &["merge-base", "HEAD", "@{u}"]).unwrap_or_default();
    if local.is_empty() || remote.is_empty() || merge_base.is_empty() || local == remote {
        return;
    }

    if merge_base == remote {
        let message =
            format!("Current branch {branch} has unpushed commits relative to {upstream}");
        let fix = "push the branch before continuing: git push";
        if DEV_STRICT_PHASES.contains(&phase) {
            report.fail(&message, Some(fix));
        } else {
            report.warn(format!("{message}; {fix}"));
        }
    } else if merge_base == local {
        report.warn(format!("Current branch {branch} is behind {upstream}; pull/rebase before release work if this is unexpected"));
    } else {
        let message = format!("Current branch {branch} has diverged from {upstream}");
        let fix = "reconcile the branch with its upstream before publishing";
        if DEV_STRICT_PHASES.contains(&phase) {
            report.fail(&message, Some(fix));
        } else {
            report.warn(format!("{message}; {fix}"));
        }
    }
}

pub fn dev_changed_paths(root: &Utf8Path) -> Vec<String> {
    let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for cmd in [
        vec!["diff", "--name-only"],
        vec!["diff", "--cached", "--name-only"],
    ] {
        if let Some(output) = git_output(root, &cmd) {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    paths.insert(line.to_string());
                }
            }
        }
    }
    if let Some(upstream) = git_output(
        root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    ) {
        if let Some(output) = git_output(
            root,
            &["diff", "--name-only", &format!("{upstream}...HEAD")],
        ) {
            for line in output.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    paths.insert(line.to_string());
                }
            }
        }
    }
    let mut paths: Vec<_> = paths.into_iter().collect();
    paths.sort();
    paths
}

pub fn classify_dev_path(path: &str) -> &'static str {
    if path.starts_with("dev_tools/") {
        return "dev_tools";
    }
    if path.starts_with("test/") || path.starts_with(".github/workflows/") {
        return "verification";
    }
    if path.starts_with("docs/") {
        return "docs";
    }
    if DEV_HOMEPAGE_PATHS.contains(&path) {
        return "homepage";
    }
    if path == "CHANGELOG.md" {
        return "release_notes";
    }
    if DEV_RELEASE_TRIGGER_PATHS.contains(&path)
        || path.starts_with("lib/")
        || path.starts_with("bin/")
        || path.starts_with("scripts/")
    {
        return "runtime_package";
    }
    "other"
}

pub fn check_dev_change_set(root: &Utf8Path, report: &mut Report) {
    let paths = dev_changed_paths(root);
    if paths.is_empty() {
        report.warn("No local/branch delta found relative to upstream; dev check is validating git/GitHub state only");
        return;
    }
    let mut categories: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for path in &paths {
        *categories.entry(classify_dev_path(path)).or_insert(0) += 1;
    }
    let mut summary_parts: Vec<String> = categories
        .iter()
        .map(|(name, count)| format!("{name}={count}"))
        .collect();
    summary_parts.sort();
    report.warn(format!(
        "Development change classification: {}",
        summary_parts.join(", ")
    ));

    let category_set: std::collections::HashSet<_> = categories.keys().cloned().collect();
    if category_set.contains("runtime_package") {
        report.warn("Runtime/package files changed; decide whether this should become a versioned release before calling the work complete");
    }
    if category_set.contains("homepage") {
        report.warn("Homepage README files changed; push/merge to the default branch before expecting GitHub's homepage to update");
    }
    if category_set.contains("release_notes") {
        report.warn("CHANGELOG changed; if this is a public package change, use prepare/published release phases");
    }
    if category_set.is_subset(
        &["dev_tools", "verification", "docs"]
            .iter()
            .cloned()
            .collect(),
    ) {
        report.warn("Change set appears development-only; a release tag is usually not needed");
    }
}

pub fn check_git_tag(root: &Utf8Path, version: &str, phase: &str, report: &mut Report) {
    let local_commit = git_output(root, &["rev-list", "-n", "1", version]);
    if phase == "prepare" {
        if let Some(commit) = local_commit {
            report.warn(format!("Local tag {version} already exists at {commit}; confirm this is intentional before publishing"));
        }
        return;
    }

    let Some(local_commit) = local_commit else {
        report.fail(
            format!("Local git tag {version} does not exist"),
            Some(&format!("create the tag on the intended release commit: git tag {version} && git push origin {version}")),
        );
        return;
    };

    let mut remote_sha = String::new();
    let remote = run(
        &[
            "git",
            "ls-remote",
            "--tags",
            "origin",
            &format!("refs/tags/{version}^{{}}"),
        ],
        root,
    );
    if remote.status.success() {
        let stdout = String::from_utf8_lossy(&remote.stdout);
        remote_sha = stdout
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
    }
    if remote_sha.is_empty() {
        let remote = run(
            &[
                "git",
                "ls-remote",
                "--tags",
                "origin",
                &format!("refs/tags/{version}"),
            ],
            root,
        );
        if remote.status.success() {
            let stdout = String::from_utf8_lossy(&remote.stdout);
            remote_sha = stdout
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
        }
    }

    if remote_sha.is_empty() {
        report.fail(
            format!("Remote git tag {version} is missing on origin"),
            Some(&format!("push the tag: git push origin {version}")),
        );
        return;
    }

    if remote_sha != local_commit {
        report.fail(
            format!("Remote tag {version} points to {remote_sha}, but local tag resolves to {local_commit}"),
            Some("stop and inspect the tag mismatch; do not force-push release tags without maintainer approval"),
        );
    }
}

pub fn check_local_files(root: &Utf8Path, version: &str, repo: &str, report: &mut Report) {
    let bare_version = version.strip_prefix('v').unwrap_or(version);
    let version_path = root.join("VERSION");
    let ccbr_path = root.join("ccb");
    let changelog_path = root.join("CHANGELOG.md");
    let readme_path = root.join("README.md");
    let readme_zh_path = root.join("README_zh.md");

    let files = [
        ("VERSION", read(&version_path)),
        ("ccb", read(&ccbr_path)),
        ("CHANGELOG.md", read(&changelog_path)),
        ("README.md", read(&readme_path)),
        ("README_zh.md", read(&readme_zh_path)),
    ];
    let file_map: std::collections::HashMap<&str, String> = files.into_iter().collect();

    if file_map["VERSION"].trim() != bare_version {
        report.fail(
            format!(
                "VERSION is {:?}, expected {bare_version:?}",
                file_map["VERSION"].trim()
            ),
            Some(&format!("write exactly {bare_version} to VERSION")),
        );
    }
    if !file_map["ccb"].contains(&format!(r#"VERSION = "{bare_version}""#)) {
        report.fail(
            format!("ccb does not contain VERSION = {bare_version:?}"),
            Some(&format!(r#"update ccb to VERSION = "{bare_version}""#)),
        );
    }

    let changelog_section = markdown_section(&file_map["CHANGELOG.md"], version);
    if changelog_section.is_none() {
        report.fail(
            format!("CHANGELOG.md has no {version} section"),
            Some(&format!(
                "add a non-empty ## {version} (...) section near the top of CHANGELOG.md"
            )),
        );
    } else if !has_substantive_release_text(changelog_section.as_deref()) {
        report.fail(
            format!("CHANGELOG.md {version} section is empty"),
            Some("add concrete user-facing release bullets before publishing"),
        );
    }

    let badge_re = Regex::new(r"version-([0-9]+\.[0-9]+\.[0-9]+)-orange\.svg").unwrap();
    for readme_name in ["README.md", "README_zh.md"] {
        let body = &file_map[readme_name];
        let versions = release_note_versions(body);
        if !versions.is_empty() {
            if versions[0] != version {
                report.fail(
                    format!(
                        "{readme_name} first release notes entry is {}, expected {version}",
                        versions[0]
                    ),
                    Some(&format!(
                        "move the {version} release notes entry above older versions"
                    )),
                );
            }
            let sorted_versions = {
                let mut v = versions.clone();
                v.sort_by_key(|x| std::cmp::Reverse(semver_tuple(x)));
                v
            };
            if versions != sorted_versions {
                report.warn(format!(
                    "{readme_name} release notes are not in descending semver order"
                ));
            }
        }
        if !body.contains(&format!("version-{bare_version}-orange.svg")) {
            report.fail(
                format!("{readme_name} version badge does not show {bare_version}"),
                Some(&format!(
                    "update the top badge to version-{bare_version}-orange.svg"
                )),
            );
        }
        if !body.contains(&format!("<summary><b>{version}</b>")) {
            report.fail(
                format!("{readme_name} release notes do not include {version}"),
                Some(&format!(
                    "add a non-empty {version} entry to Release Notes / 新版本记录"
                )),
            );
        } else if !has_substantive_release_text(readme_release_block(body, version).as_deref()) {
            report.fail(
                format!("{readme_name} release notes entry for {version} is empty"),
                Some("add concrete release bullets under the details block"),
            );
        }
        if !body.contains(".ccbr/ccbr_memory.md") {
            report.fail(
                format!("{readme_name} does not mention .ccbr/ccbr_memory.md"),
                Some("state that .ccbr/ccbr_memory.md is the project-wide shared memory document"),
            );
        }

        let badge_versions: std::collections::HashSet<_> = badge_re
            .captures_iter(body)
            .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
            .collect();
        let stale_badges: Vec<_> = badge_versions
            .iter()
            .filter(|item| *item != bare_version)
            .cloned()
            .collect();
        if !stale_badges.is_empty() {
            report.fail(
                format!(
                    "{readme_name} has stale version badges: {}",
                    stale_badges.join(", ")
                ),
                Some(&format!("replace stale current badges with {bare_version}")),
            );
        }
    }

    let (owner, name) = repo
        .split_once('/')
        .unwrap_or(("SeemSeam", "claude_codex_bridge"));
    let expected_clone = format!("https://github.com/{owner}/{name}.git");
    let readme_install_headings = [
        ("README.md", "How to Install"),
        ("README_zh.md", "如何安装"),
    ];
    let clone_re = Regex::new(r"git\s+clone\s+(https://github\.com/[^\s`]+\.git)").unwrap();
    for (readme_name, heading) in readme_install_headings {
        let body = &file_map[readme_name];
        let install_body = install_section(body, heading);
        let clone_urls: std::collections::HashSet<_> = clone_re
            .captures_iter(&install_body)
            .map(|caps| caps[1].to_string())
            .collect();
        let wrong_urls: Vec<_> = clone_urls
            .iter()
            .filter(|url| *url != &expected_clone)
            .cloned()
            .collect();
        if !wrong_urls.is_empty() {
            report.fail(
                format!(
                    "{readme_name} has clone URL(s) not matching {expected_clone}: {}",
                    wrong_urls
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some(&format!("replace README clone URLs with {expected_clone}")),
            );
        }
    }

    if file_map["README.md"].contains("CCB.md") || file_map["README_zh.md"].contains("CCB.md") {
        report.fail(
            "README mentions current CCB.md support; current design must only use .ccbr/ccbr_memory.md",
            Some("remove current-feature references to CCB.md; keep only .ccbr/ccbr_memory.md"),
        );
    }

    report.warn("Manually inspect README What's New / 最新亮点 for stale prose; this cannot be proven by version regex alone");
}

fn file_sha256(path: &Utf8Path) -> Option<String> {
    let bytes = read_bytes(path)?;
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(&bytes);
    Some(format!("{hash:x}"))
}

pub fn check_active_skill_sync(root: &Utf8Path, report: &mut Report) {
    let source_dir = root.join("dev_tools/skills/ccbr-github");
    if !source_dir.is_dir() {
        return;
    }
    let ccbr_dir = root.join(".ccbr");
    if !ccbr_dir.is_dir() {
        return;
    }
    let pattern =
        ccbr_dir.as_str().to_string() + "/agents/*/provider-state/codex/home/skills/ccbr-github";
    let glob_results = match glob::glob(&pattern) {
        Ok(g) => g.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return,
    };
    for active_dir in glob_results {
        let active_utf8 = match camino::Utf8PathBuf::from_path_buf(active_dir) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if active_utf8 == source_dir {
            continue;
        }
        let mismatched: Vec<_> = TRACKED_SKILL_FILES
            .iter()
            .filter(|relative| {
                file_sha256(&source_dir.join(**relative))
                    != file_sha256(&active_utf8.join(**relative))
            })
            .map(|s| (*s).to_string())
            .collect();
        if !mismatched.is_empty() {
            report.warn(format!(
                "Active ccbr-github skill copy differs from dev_tools at {active_utf8}: {}",
                mismatched.join(", ")
            ));
        }
    }
}

// Re-exports for CLI and tests.
pub fn repo_root_cli(start: &Utf8Path) -> camino::Utf8PathBuf {
    repo_root(start)
}

pub fn infer_repo_cli(root: &Utf8Path) -> String {
    infer_repo(root)
}

pub fn normalize_version_cli(version: &str) -> String {
    normalize_version(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_dev_path() {
        assert_eq!(classify_dev_path("dev_tools/x"), "dev_tools");
        assert_eq!(classify_dev_path("test/x"), "verification");
        assert_eq!(classify_dev_path("docs/x"), "docs");
        assert_eq!(classify_dev_path("README.md"), "homepage");
        assert_eq!(classify_dev_path("CHANGELOG.md"), "release_notes");
        assert_eq!(classify_dev_path("lib/x"), "runtime_package");
        assert_eq!(classify_dev_path("unknown"), "other");
    }
}
