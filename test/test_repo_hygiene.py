from __future__ import annotations

from pathlib import Path
import subprocess

import pytest


def _is_ephemeral_repo_artifact(path_text: str) -> bool:
    path = Path(path_text.strip())
    if not path.parts:
        return False
    first = path.parts[0]
    return first == ".tmp_pytest" or first.startswith(".tmp_test_env_")


def test_git_index_does_not_track_ephemeral_test_artifacts() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    if not (repo_root / ".git").exists():
        pytest.skip("git checkout required")

    completed = subprocess.run(
        ["git", "-C", str(repo_root), "ls-files"],
        capture_output=True,
        text=True,
        check=True,
    )
    tracked = [
        line
        for line in completed.stdout.splitlines()
        if line.strip() and _is_ephemeral_repo_artifact(line)
    ]

    assert tracked == []


def test_useful_tools_skills_are_provider_paired() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    useful_tools = repo_root / "useful_tools"
    if not useful_tools.exists():
        pytest.skip("useful_tools not present")

    codex_root = useful_tools / "codex_skills"
    claude_root = useful_tools / "claude_skills"
    codex_skills = {
        path.name
        for path in codex_root.iterdir()
        if path.is_dir() and (path / "SKILL.md").is_file()
    }
    claude_skills = {
        path.name
        for path in claude_root.iterdir()
        if path.is_dir() and (path / "SKILL.md").is_file()
    }

    assert codex_skills == claude_skills


def test_inherited_skills_live_under_inherit_skills_only() -> None:
    repo_root = Path(__file__).resolve().parents[1]

    for legacy_root in ("claude_skills", "codex_skills", "droid_skills"):
        assert not (repo_root / legacy_root).exists()

    inherited = repo_root / "inherit_skills"
    assert (inherited / "claude_skills" / "ask" / "SKILL.md").is_file()
    assert (inherited / "codex_skills" / "ask" / "SKILL.md").is_file()
    assert (inherited / "droid_skills" / "ask" / "SKILL.md").is_file()
    assert (inherited / "claude_skills" / "ccb_config" / "SKILL.md").is_file()
    assert (inherited / "codex_skills" / "ccb_config" / "SKILL.md").is_file()

    assert not (repo_root / "useful_tools" / "claude_skills" / "ccb_config").exists()
    assert not (repo_root / "useful_tools" / "codex_skills" / "ccb_config").exists()


def test_ccb_config_skill_uses_current_config_authority() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    for provider_root in ("claude_skills", "codex_skills"):
        skill_text = (
            repo_root
            / "inherit_skills"
            / provider_root
            / "ccb_config"
            / "SKILL.md"
        ).read_text(encoding="utf-8")
        reference_text = (
            repo_root
            / "inherit_skills"
            / provider_root
            / "ccb_config"
            / "references"
            / "ccb-config.md"
        ).read_text(encoding="utf-8")

        assert "The current CCB project config is `.ccb/ccb.config`" in skill_text
        assert "Never write `.ccb_config/ccb.config`" in skill_text
        assert "Never run `ccb`, `ccb -s`, `ccb kill`" in skill_text
        assert "result.used_default" in skill_text
        assert "Do not write `.ccb_config/ccb.config`" in reference_text


def test_ccb_config_memory_patterns_describe_callback_routing() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    for provider_root in ("claude_skills", "codex_skills"):
        skill_text = (
            repo_root
            / "inherit_skills"
            / provider_root
            / "ccb_config"
            / "SKILL.md"
        ).read_text(encoding="utf-8")
        memory_text = (
            repo_root
            / "inherit_skills"
            / provider_root
            / "ccb_config"
            / "references"
            / "memory-patterns.md"
        ).read_text(encoding="utf-8")

        assert "separate root work packages" in skill_text
        assert "main -> worker -> reviewer" in skill_text
        assert "main -> worker1 -> reviewer" in memory_text
        assert "main -> worker2 -> reviewer" in memory_text
        assert "Do not create multiple callback dependencies from one active task" in memory_text
        assert "do not route through `main` only to relay work" in memory_text
