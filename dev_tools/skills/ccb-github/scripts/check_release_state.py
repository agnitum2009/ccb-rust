#!/usr/bin/env python3
"""Read-only release surface checker for the CCB GitHub project."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path


EXPECTED_ASSETS = {
    "ccb-linux-x86_64.tar.gz",
    "ccb-macos-universal.tar.gz",
    "SHA256SUMS",
}
CHECKSUMMED_ASSETS = EXPECTED_ASSETS - {"SHA256SUMS"}
REQUIRED_TAG_WORKFLOWS = {"Release Artifacts"}
BRANCH_VALIDATION_WORKFLOWS = {
    "Tests",
    "CCBD Real Platform Smoke",
    "Cross-Platform Compatibility Test",
}


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=False)


def repo_root(start: Path) -> Path:
    proc = run(["git", "rev-parse", "--show-toplevel"], start)
    if proc.returncode == 0:
        return Path(proc.stdout.strip())
    return start.resolve()


def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return ""


def fail(issues: list[str], message: str, *, fix: str | None = None) -> None:
    if fix:
        issues.append(f"FAIL: {message}\n      fix: {fix}")
    else:
        issues.append(f"FAIL: {message}")


def warn(warnings: list[str], message: str) -> None:
    warnings.append(f"WARN: {message}")


def infer_repo(root: Path) -> str:
    proc = run(["git", "remote", "get-url", "origin"], root)
    if proc.returncode != 0:
        return "SeemSeam/claude_codex_bridge"
    url = proc.stdout.strip()
    match = re.search(r"github.com[:/]([^/]+)/([^/.]+)(?:\.git)?$", url)
    if match:
        return f"{match.group(1)}/{match.group(2)}"
    return "SeemSeam/claude_codex_bridge"


def markdown_section(body: str, heading: str) -> str | None:
    pattern = re.compile(
        rf"(?ms)^##\s+{re.escape(heading)}[^\n]*\n(?P<body>.*?)(?=^##\s+|\Z)"
    )
    match = pattern.search(body)
    if not match:
        return None
    return match.group("body").strip()


def readme_release_block(body: str, version: str) -> str | None:
    pattern = re.compile(
        rf"(?ms)<summary><b>{re.escape(version)}</b>.*?</summary>(?P<body>.*?)(?=</details>)"
    )
    match = pattern.search(body)
    if not match:
        return None
    return match.group("body").strip()


def has_substantive_release_text(text: str | None) -> bool:
    if not text:
        return False
    cleaned_lines = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith(("<!--", "-->", "<details", "</details>", "<summary", "</summary>")):
            continue
        cleaned_lines.append(stripped)
    return any(re.search(r"[A-Za-z0-9\u4e00-\u9fff]", line) for line in cleaned_lines)


def semver_tuple(version: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", str(version or "").strip())
    if not match:
        return (-1, -1, -1)
    return tuple(int(part) for part in match.groups())


def release_note_versions(body: str) -> list[str]:
    return re.findall(r"<summary><b>(v\d+\.\d+\.\d+)</b>", body)


def install_section(body: str, heading: str) -> str:
    pattern = re.compile(rf"(?ms)^##\s+{re.escape(heading)}\s*$\n(?P<body>.*?)(?=^##\s+|\Z)")
    match = pattern.search(body)
    return match.group("body") if match else body


def git_output(root: Path, cmd: list[str]) -> str | None:
    proc = run(["git", *cmd], root)
    if proc.returncode != 0:
        return None
    return proc.stdout.strip()


def check_git_tag(root: Path, version: str, phase: str, issues: list[str], warnings: list[str]) -> None:
    local_commit = git_output(root, ["rev-list", "-n", "1", version])
    if phase == "prepare":
        if local_commit:
            warn(warnings, f"Local tag {version} already exists at {local_commit}; confirm this is intentional before publishing")
        return

    if not local_commit:
        fail(
            issues,
            f"Local git tag {version} does not exist",
            fix=f"create the tag on the intended release commit: git tag {version} && git push origin {version}",
        )
        return

    remote = run(["git", "ls-remote", "--tags", "origin", f"refs/tags/{version}^{{}}"], root)
    remote_sha = remote.stdout.split()[0] if remote.returncode == 0 and remote.stdout.strip() else ""
    if not remote_sha:
        remote = run(["git", "ls-remote", "--tags", "origin", f"refs/tags/{version}"], root)
        remote_sha = remote.stdout.split()[0] if remote.returncode == 0 and remote.stdout.strip() else ""

    if not remote_sha:
        fail(
            issues,
            f"Remote git tag {version} is missing on origin",
            fix=f"push the tag: git push origin {version}",
        )
        return

    if remote_sha != local_commit:
        fail(
            issues,
            f"Remote tag {version} points to {remote_sha}, but local tag resolves to {local_commit}",
            fix="stop and inspect the tag mismatch; do not force-push release tags without maintainer approval",
        )


def check_local_files(root: Path, version: str, repo: str, issues: list[str], warnings: list[str]) -> None:
    bare_version = version.removeprefix("v")
    files = {
        "VERSION": read(root / "VERSION"),
        "ccb": read(root / "ccb"),
        "CHANGELOG.md": read(root / "CHANGELOG.md"),
        "README.md": read(root / "README.md"),
        "README_zh.md": read(root / "README_zh.md"),
    }

    if files["VERSION"].strip() != bare_version:
        fail(issues, f"VERSION is {files['VERSION'].strip()!r}, expected {bare_version!r}", fix=f"write exactly {bare_version} to VERSION")
    if f'VERSION = "{bare_version}"' not in files["ccb"]:
        fail(issues, f"ccb does not contain VERSION = {bare_version!r}", fix=f'update ccb to VERSION = "{bare_version}"')

    changelog_section = markdown_section(files["CHANGELOG.md"], version)
    if changelog_section is None:
        fail(issues, f"CHANGELOG.md has no {version} section", fix=f"add a non-empty ## {version} (...) section near the top of CHANGELOG.md")
    elif not has_substantive_release_text(changelog_section):
        fail(issues, f"CHANGELOG.md {version} section is empty", fix="add concrete user-facing release bullets before publishing")

    for readme_name in ("README.md", "README_zh.md"):
        body = files[readme_name]
        versions = release_note_versions(body)
        if versions:
            if versions[0] != version:
                fail(
                    issues,
                    f"{readme_name} first release notes entry is {versions[0]}, expected {version}",
                    fix=f"move the {version} release notes entry above older versions",
                )
            sorted_versions = sorted(versions, key=semver_tuple, reverse=True)
            if versions != sorted_versions:
                warn(warnings, f"{readme_name} release notes are not in descending semver order")
        if f"version-{bare_version}-orange.svg" not in body:
            fail(issues, f"{readme_name} version badge does not show {bare_version}", fix=f"update the top badge to version-{bare_version}-orange.svg")
        if f"<summary><b>{version}</b>" not in body:
            fail(issues, f"{readme_name} release notes do not include {version}", fix=f"add a non-empty {version} entry to Release Notes / 新版本记录")
        elif not has_substantive_release_text(readme_release_block(body, version)):
            fail(issues, f"{readme_name} release notes entry for {version} is empty", fix="add concrete release bullets under the details block")
        if ".ccb/ccb_memory.md" not in body:
            fail(issues, f"{readme_name} does not mention .ccb/ccb_memory.md", fix="state that .ccb/ccb_memory.md is the project-wide shared memory document")

        badge_versions = sorted(set(re.findall(r"version-([0-9]+\.[0-9]+\.[0-9]+)-orange\.svg", body)))
        stale_badges = [item for item in badge_versions if item != bare_version]
        if stale_badges:
            fail(issues, f"{readme_name} has stale version badges: {', '.join(stale_badges)}", fix=f"replace stale current badges with {bare_version}")

    owner, name = repo.split("/", 1)
    expected_clone = f"https://github.com/{owner}/{name}.git"
    readme_install_headings = {
        "README.md": "How to Install",
        "README_zh.md": "如何安装",
    }
    for readme_name, heading in readme_install_headings.items():
        body = files[readme_name]
        install_body = install_section(body, heading)
        clone_urls = sorted(set(re.findall(r"git\s+clone\s+(https://github\.com/[^\s`]+\.git)", install_body)))
        wrong_urls = [url for url in clone_urls if url != expected_clone]
        if wrong_urls:
            fail(issues, f"{readme_name} has clone URL(s) not matching {expected_clone}: {', '.join(wrong_urls)}", fix=f"replace README clone URLs with {expected_clone}")

    if "CCB.md" in files["README.md"] or "CCB.md" in files["README_zh.md"]:
        fail(issues, "README mentions current CCB.md support; current design must only use .ccb/ccb_memory.md", fix="remove current-feature references to CCB.md; keep only .ccb/ccb_memory.md")

    warn(warnings, "Manually inspect README What's New / 最新亮点 for stale prose; this cannot be proven by version regex alone")


def check_sha256sums(root: Path, version: str, repo: str, issues: list[str], warnings: list[str]) -> None:
    with tempfile.TemporaryDirectory(prefix="ccb-release-check-") as tmp:
        tmp_path = Path(tmp)
        download = run(
            [
                "gh",
                "release",
                "download",
                version,
                "--repo",
                repo,
                "--pattern",
                "SHA256SUMS",
                "--dir",
                str(tmp_path),
            ],
            root,
        )
        if download.returncode != 0:
            fail(
                issues,
                "Could not download SHA256SUMS from the GitHub release",
                fix="rerun Release Artifacts or re-upload SHA256SUMS, then rerun the published check",
            )
            return
        sums_path = tmp_path / "SHA256SUMS"
        payload = read(sums_path)
        found: dict[str, str] = {}
        for line in payload.splitlines():
            parts = line.strip().split()
            if len(parts) != 2:
                continue
            digest, name = parts
            if re.fullmatch(r"[0-9a-fA-F]{64}", digest):
                found[Path(name).name] = digest.lower()
        missing = sorted(CHECKSUMMED_ASSETS - set(found))
        if missing:
            fail(
                issues,
                f"SHA256SUMS is missing checksum entry/entries for: {', '.join(missing)}",
                fix="rerun Release Artifacts so SHA256SUMS is regenerated from the uploaded tarballs",
            )
        extra = sorted(set(found) - CHECKSUMMED_ASSETS)
        if extra:
            warn(warnings, f"SHA256SUMS contains unexpected extra asset checksum(s): {', '.join(extra)}")


def check_github(root: Path, version: str, repo: str, issues: list[str], warnings: list[str]) -> None:
    auth = run(["gh", "auth", "status", "--hostname", "github.com"], root)
    if auth.returncode != 0:
        fail(
            issues,
            "GitHub CLI is not authenticated",
            fix="run gh auth login, then rerun the published release check",
        )
        return

    release = run(["gh", "release", "view", version, "--repo", repo, "--json", "tagName,url,assets,isDraft"], root)
    if release.returncode != 0:
        fail(
            issues,
            f"GitHub release {version} not found for {repo}: {release.stderr.strip() or release.stdout.strip()}",
            fix=f"create the release page first: gh release create {version} --repo {repo} --title {version} --notes-file <notes-file>",
        )
        return

    try:
        payload = json.loads(release.stdout)
    except json.JSONDecodeError as exc:
        fail(issues, f"Could not parse gh release JSON: {exc}")
        return

    if payload.get("tagName") != version:
        fail(issues, f"GitHub release tag is {payload.get('tagName')!r}, expected {version!r}")
    if payload.get("isDraft"):
        fail(issues, f"GitHub release {version} is still a draft", fix="publish the draft after assets and notes are ready")

    asset_names = {asset.get("name") for asset in payload.get("assets", [])}
    missing = sorted(EXPECTED_ASSETS - asset_names)
    if missing:
        fail(
            issues,
            f"GitHub release missing asset(s): {', '.join(missing)}",
            fix=f"rerun Release Artifacts for {version}, then verify assets again",
        )
    elif "SHA256SUMS" in asset_names:
        check_sha256sums(root, version, repo, issues, warnings)

    repo_view = run(["gh", "repo", "view", repo, "--json", "description,repositoryTopics,latestRelease,url"], root)
    if repo_view.returncode != 0:
        warn(warnings, f"Could not read GitHub repo metadata: {repo_view.stderr.strip()}")
    else:
        try:
            repo_payload = json.loads(repo_view.stdout)
        except json.JSONDecodeError as exc:
            warn(warnings, f"Could not parse gh repo JSON: {exc}")
        else:
            latest = (repo_payload.get("latestRelease") or {}).get("tagName")
            if latest != version:
                fail(issues, f"GitHub latest release is {latest!r}, expected {version!r}", fix="publish the GitHub release and ensure it is not draft/prerelease unless intended")
            description = repo_payload.get("description") or ""
            if "Claude, Codex & Gemini" in description and "OpenCode" not in description:
                warn(warnings, "GitHub description may be stale: it mentions Claude/Codex/Gemini but not newer supported providers")

    runs = run(
        [
            "gh",
            "run",
            "list",
            "--repo",
            repo,
            "--limit",
            "20",
            "--json",
            "name,status,conclusion,headBranch,event,databaseId,url,headSha",
        ],
        root,
    )
    if runs.returncode != 0:
        warn(warnings, f"Could not read GitHub Actions runs: {runs.stderr.strip()}")
        return
    try:
        run_payload = json.loads(runs.stdout)
    except json.JSONDecodeError as exc:
        warn(warnings, f"Could not parse gh run JSON: {exc}")
        return

    tag_commit = git_output(root, ["rev-list", "-n", "1", version]) or ""

    for workflow_name in sorted(REQUIRED_TAG_WORKFLOWS):
        candidates = [
            item
            for item in run_payload
            if item.get("name") == workflow_name and _release_artifacts_run_matches(item, version=version, tag_commit=tag_commit)
        ]
        successes = [
            item
            for item in candidates
            if item.get("status") == "completed" and item.get("conclusion") == "success"
        ]
        if successes:
            accepted = successes[0]
            if accepted.get("event") == "workflow_dispatch" and tag_commit and accepted.get("headSha") != tag_commit:
                warn(
                    warnings,
                    f"{workflow_name} was accepted from workflow_dispatch but its headSha {accepted.get('headSha')} does not match tag {tag_commit}; confirm it used input tag={version}",
                )
            continue
        if candidates:
            latest = candidates[0]
            fail(
                issues,
                f"GitHub Actions {workflow_name} for {version} is {latest.get('status')}/{latest.get('conclusion')}: {latest.get('url')}",
                fix="open the run, fix the root cause, rerun the failed workflow, and do not call the release complete while red",
            )
            continue
        fail(
            issues,
            f"Missing release workflow run for {version}: {workflow_name}",
            fix=f"push tag {version} or manually dispatch {workflow_name} with input tag={version}",
        )

    _check_branch_validation_runs(run_payload, tag_commit=tag_commit, warnings=warnings)


def _release_artifacts_run_matches(item: dict[str, object], *, version: str, tag_commit: str) -> bool:
    if item.get("headBranch") == version:
        return True
    if tag_commit and item.get("headSha") == tag_commit:
        return True
    return item.get("event") == "workflow_dispatch"


def _check_branch_validation_runs(run_payload: list[dict[str, object]], *, tag_commit: str, warnings: list[str]) -> None:
    if not tag_commit:
        return
    found: set[str] = set()
    for workflow_name in sorted(BRANCH_VALIDATION_WORKFLOWS):
        candidates = [
            item
            for item in run_payload
            if item.get("name") == workflow_name
            and item.get("headSha") == tag_commit
            and item.get("headBranch") not in {"", None}
            and not str(item.get("headBranch")).startswith("v")
        ]
        if not candidates:
            continue
        found.add(workflow_name)
        latest = candidates[0]
        if latest.get("status") != "completed" or latest.get("conclusion") != "success":
            warn(
                warnings,
                f"Branch validation workflow {workflow_name} for release commit is {latest.get('status')}/{latest.get('conclusion')}: {latest.get('url')}",
            )
    missing = sorted(BRANCH_VALIDATION_WORKFLOWS - found)
    if missing:
        warn(warnings, f"No recent branch validation run found for release commit for: {', '.join(missing)}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Check CCB release-facing local and GitHub state.")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--repo", default=None, help="GitHub repo, e.g. SeemSeam/claude_codex_bridge")
    parser.add_argument("--version", default=None, help="Release version, with or without leading v")
    parser.add_argument("--phase", choices=("prepare", "published"), default="prepare")
    args = parser.parse_args()

    root = repo_root(args.repo_root)
    repo = args.repo or infer_repo(root)
    raw_version = args.version or read(root / "VERSION").strip()
    version = raw_version if raw_version.startswith("v") else f"v{raw_version}"

    issues: list[str] = []
    warnings: list[str] = []

    check_local_files(root, version, repo, issues, warnings)
    check_git_tag(root, version, args.phase, issues, warnings)
    if args.phase == "published":
        check_github(root, version, repo, issues, warnings)

    print(f"CCB release check: {version} ({args.phase})")
    print(f"repo root: {root}")
    print(f"github repo: {repo}")

    if warnings:
        print("\nWarnings:")
        for item in warnings:
            print(f"- {item}")

    if issues:
        print("\nIssues:")
        for item in issues:
            print(f"- {item}")
        return 1

    print("\nOK: no blocking release-surface drift found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
