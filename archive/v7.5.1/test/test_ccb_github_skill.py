from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "dev_tools" / "skills" / "ccb-github" / "scripts" / "check_release_state.py"


def _load_checker():
    spec = importlib.util.spec_from_file_location("ccb_github_release_checker", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True)


def _init_repo_with_remote(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    remote = tmp_path / "remote.git"
    repo.mkdir()
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "ccb@example.invalid")
    _git(repo, "config", "user.name", "CCB Test")
    (repo / "file.txt").write_text("initial\n", encoding="utf-8")
    _git(repo, "add", "file.txt")
    _git(repo, "commit", "-m", "initial")
    subprocess.run(["git", "init", "--bare", str(remote)], check=True, capture_output=True, text=True)
    _git(repo, "remote", "add", "origin", str(remote))
    _git(repo, "push", "-u", "origin", "main")
    return repo


def test_check_local_git_state_warns_before_publish_but_fails_after_publish(tmp_path: Path) -> None:
    checker = _load_checker()
    repo = _init_repo_with_remote(tmp_path)
    (repo / "file.txt").write_text("updated\n", encoding="utf-8")
    _git(repo, "add", "file.txt")
    _git(repo, "commit", "-m", "local-only")

    issues: list[str] = []
    warnings: list[str] = []
    checker.check_local_git_state(repo, "prepare", issues, warnings)
    assert not issues
    assert any("unpushed commits" in item for item in warnings)

    issues = []
    warnings = []
    checker.check_local_git_state(repo, "published", issues, warnings)
    assert any("unpushed commits" in item for item in issues)

    issues = []
    warnings = []
    checker.check_local_git_state(repo, "dev", issues, warnings)
    assert any("unpushed commits" in item for item in issues)


def test_check_local_git_state_fails_dirty_worktree_after_publish(tmp_path: Path) -> None:
    checker = _load_checker()
    repo = _init_repo_with_remote(tmp_path)
    (repo / "file.txt").write_text("dirty\n", encoding="utf-8")

    issues: list[str] = []
    warnings: list[str] = []
    checker.check_local_git_state(repo, "published", issues, warnings)

    assert any("Worktree has uncommitted changes" in item for item in issues)

    issues = []
    warnings = []
    checker.check_local_git_state(repo, "dev", issues, warnings)
    assert any("Worktree has uncommitted changes" in item for item in issues)


def test_default_branch_compare_rejects_release_tag_missing_from_main(monkeypatch, tmp_path: Path) -> None:
    checker = _load_checker()

    def fake_run(cmd: list[str], cwd: Path):
        assert cmd[:2] == ["gh", "api"]
        assert cmd[2] == "repos/SeemSeam/claude_codex_bridge/compare/v9.9.9...main"
        return subprocess.CompletedProcess(cmd, 0, stdout="diverged\n", stderr="")

    monkeypatch.setattr(checker.github, "run", fake_run)
    issues: list[str] = []
    warnings: list[str] = []

    checker.check_default_branch_contains_release(
        root=tmp_path,
        version="v9.9.9",
        repo="SeemSeam/claude_codex_bridge",
        default_branch="main",
        issues=issues,
        warnings=warnings,
    )

    assert any("does not contain release tag v9.9.9" in item for item in issues)


def test_default_branch_compare_accepts_ahead_or_identical(monkeypatch, tmp_path: Path) -> None:
    checker = _load_checker()

    def fake_run(cmd: list[str], cwd: Path):
        return subprocess.CompletedProcess(cmd, 0, stdout="ahead\n", stderr="")

    monkeypatch.setattr(checker.github, "run", fake_run)
    issues: list[str] = []
    warnings: list[str] = []

    checker.check_default_branch_contains_release(
        root=tmp_path,
        version="v9.9.9",
        repo="SeemSeam/claude_codex_bridge",
        default_branch="main",
        issues=issues,
        warnings=warnings,
    )

    assert not issues


def test_dev_change_classification_flags_release_and_dev_only_paths() -> None:
    checker = _load_checker()

    assert checker.classify_dev_path("dev_tools/skills/ccb-github/SKILL.md") == "dev_tools"
    assert checker.classify_dev_path("test/test_ccb_github_skill.py") == "verification"
    assert checker.classify_dev_path(".github/workflows/test.yml") == "verification"
    assert checker.classify_dev_path("docs/plan.md") == "docs"
    assert checker.classify_dev_path("README.md") == "homepage"
    assert checker.classify_dev_path("CHANGELOG.md") == "release_notes"
    assert checker.classify_dev_path("lib/runtime.py") == "runtime_package"
    assert checker.classify_dev_path("ccb") == "runtime_package"


def test_required_dev_workflows_depend_on_branch() -> None:
    checker = _load_checker()

    assert checker.required_dev_workflows("main", "main") == {
        "Tests",
        "CCBD Real Platform Smoke",
        "Cross-Platform Compatibility Test",
    }
    assert checker.required_dev_workflows("feature/x", "main") == {
        "Tests",
        "CCBD Real Platform Smoke",
    }


def test_check_dev_branch_workflows_entrypoint_uses_compat_signature(monkeypatch, tmp_path: Path) -> None:
    checker = _load_checker()
    observed: dict[str, object] = {}

    def fake_check(**kwargs):
        observed.update(kwargs)

    monkeypatch.setattr(checker.github, "_check_dev_branch_workflows", fake_check)
    issues: list[str] = []
    warnings: list[str] = []

    checker.check_dev_branch_workflows(
        root=tmp_path,
        repo="SeemSeam/claude_codex_bridge",
        wait_seconds=0,
        poll_interval=1,
        issues=issues,
        warnings=warnings,
    )

    assert observed["root"] == tmp_path
    assert observed["repo"] == "SeemSeam/claude_codex_bridge"
    assert observed["gh_auth_is_ready_fn"] is checker.github.gh_auth_is_ready
    assert observed["repo_default_branch_fn"] is checker.github.repo_default_branch


def test_release_artifacts_run_match_is_scoped_to_tag_or_commit() -> None:
    checker = _load_checker()

    assert checker._release_artifacts_run_matches(
        {"headBranch": "v9.9.9", "headSha": "other", "event": "push"},
        version="v9.9.9",
        tag_commit="abc123",
    )
    assert checker._release_artifacts_run_matches(
        {"headBranch": "main", "headSha": "abc123", "event": "workflow_dispatch"},
        version="v9.9.9",
        tag_commit="abc123",
    )
    assert not checker._release_artifacts_run_matches(
        {"headBranch": "main", "headSha": "wrong", "event": "workflow_dispatch"},
        version="v9.9.9",
        tag_commit="abc123",
    )


def test_published_wait_does_not_hide_failed_release_workflow() -> None:
    checker = _load_checker()

    assert not checker._published_state_is_pending(
        release_payload={"assets": []},
        run_payload=[
            {
                "name": "Release Artifacts",
                "headBranch": "v9.9.9",
                "headSha": "abc123",
                "status": "completed",
                "conclusion": "failure",
            }
        ],
        version="v9.9.9",
        tag_commit="abc123",
    )


def test_check_sha256sums_requires_entries_for_tarballs(monkeypatch, tmp_path: Path) -> None:
    checker = _load_checker()

    def fake_run(cmd: list[str], cwd: Path, *, timeout: int = 60):
        assert cmd[:3] == ["gh", "release", "download"]
        target_dir = Path(cmd[cmd.index("--dir") + 1])
        (target_dir / "SHA256SUMS").write_text(
            "0" * 64 + "  ccb-linux-x86_64.tar.gz\n",
            encoding="utf-8",
        )
        return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

    monkeypatch.setattr(checker.assets, "run", fake_run)
    issues: list[str] = []
    warnings: list[str] = []

    checker.check_sha256sums(tmp_path, "v9.9.9", "SeemSeam/claude_codex_bridge", issues, warnings)

    assert any("ccb-macos-universal.tar.gz" in item for item in issues)


def test_check_git_tag_reports_missing_published_tag(tmp_path: Path) -> None:
    checker = _load_checker()
    repo = _init_repo_with_remote(tmp_path)
    issues: list[str] = []
    warnings: list[str] = []

    checker.check_git_tag(repo, "v9.9.9", "published", issues, warnings)

    assert any("Local git tag v9.9.9 does not exist" in item for item in issues)


def test_active_skill_sync_warns_when_runtime_copy_differs(tmp_path: Path) -> None:
    checker = _load_checker()
    source = tmp_path / "dev_tools" / "skills" / "ccb-github"
    active = tmp_path / ".ccb" / "agents" / "agent4" / "provider-state" / "codex" / "home" / "skills" / "ccb-github"
    for base, marker in ((source, "source"), (active, "active")):
        (base / "agents").mkdir(parents=True)
        (base / "scripts").mkdir(parents=True)
        (base / "SKILL.md").write_text("same\n", encoding="utf-8")
        (base / "agents" / "openai.yaml").write_text("same\n", encoding="utf-8")
        (base / "scripts" / "check_release_state.py").write_text(f"{marker}\n", encoding="utf-8")

    warnings: list[str] = []
    checker.check_active_skill_sync(tmp_path, warnings)

    assert any("Active ccb-github skill copy differs" in item for item in warnings)


def test_active_skill_sync_tracks_release_checker_helpers() -> None:
    checker = _load_checker()

    assert "scripts/check_release_state.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_shared.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_markdown.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_local.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_github.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_workflows.py" in checker.TRACKED_SKILL_FILES
    assert "scripts/release_checker_assets.py" in checker.TRACKED_SKILL_FILES
