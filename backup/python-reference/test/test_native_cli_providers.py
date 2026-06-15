from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from agents.models import (
    AgentSpec,
    PermissionMode,
    QueuePolicy,
    RestoreMode,
    RuntimeMode,
    WorkspaceMode,
)
from cli.models import ParsedStartCommand
from provider_backends.deepseek.launcher import build_start_cmd as build_deepseek_start_cmd
from provider_backends.kimi.launcher import (
    build_session_payload as build_kimi_session_payload,
    build_start_cmd as build_kimi_start_cmd,
    prepare_launch_context as prepare_kimi_launch_context,
)


def _spec(
    name: str,
    provider: str,
    *,
    startup_args: tuple[str, ...] = (),
    provider_command_template: str | None = None,
) -> AgentSpec:
    return AgentSpec(
        name=name,
        provider=provider,
        target=".",
        workspace_mode=WorkspaceMode.GIT_WORKTREE,
        workspace_root=None,
        runtime_mode=RuntimeMode.PANE_BACKED,
        restore_default=RestoreMode.AUTO,
        permission_default=PermissionMode.MANUAL,
        queue_policy=QueuePolicy.SERIAL_PER_AGENT,
        startup_args=startup_args,
        provider_command_template=provider_command_template,
    )


def test_kimi_start_cmd_uses_env_override_and_auto_without_implicit_restore(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("KIMI_START_CMD", "/tmp/stub-kimi --profile test")
    command = ParsedStartCommand(project=None, agent_names=("kimi_agent",), restore=True, auto_permission=True)
    spec = _spec("kimi_agent", "kimi", startup_args=("--model", "kimi-k2"))

    cmd = build_kimi_start_cmd(command, spec, tmp_path / "runtime", "launch-1")

    assert cmd.endswith("/tmp/stub-kimi --profile test --auto-approve --model kimi-k2")
    assert "--continue" not in cmd


def test_kimi_start_cmd_preserves_explicit_user_restore_and_does_not_duplicate_auto_flags(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("KIMI_START_CMD", raising=False)
    command = ParsedStartCommand(project=None, agent_names=("kimi_agent",), restore=True, auto_permission=True)
    spec = _spec("kimi_agent", "kimi", startup_args=("--yolo", "--session", "abc"))

    cmd = build_kimi_start_cmd(command, spec, tmp_path / "runtime", "launch-1")

    assert cmd.endswith("kimi --yolo --session abc")
    assert "--auto-approve" not in cmd
    assert "--continue" not in cmd


def test_kimi_start_cmd_treats_legacy_auto_flag_as_explicit(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("KIMI_START_CMD", raising=False)
    command = ParsedStartCommand(project=None, agent_names=("kimi_agent",), restore=True, auto_permission=True)
    spec = _spec("kimi_agent", "kimi", startup_args=("--auto", "--session", "abc"))

    cmd = build_kimi_start_cmd(command, spec, tmp_path / "runtime", "launch-1")

    assert cmd.endswith("kimi --auto --session abc")
    assert "--auto-approve" not in cmd
    assert "--continue" not in cmd


def test_kimi_launch_context_materializes_context_file_and_exports_path(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("KIMI_START_CMD", raising=False)
    project_root = tmp_path / "project"
    project_root.mkdir()
    (project_root / "AGENTS.md").write_text("# rules\n", encoding="utf-8")
    paths = __import__("storage.paths", fromlist=["PathLayout"]).PathLayout(project_root)
    context = SimpleNamespace(
        project=SimpleNamespace(project_root=project_root, project_id="proj123456789"),
        paths=paths,
    )
    spec = _spec("slot1_claude", "kimi")
    plan = SimpleNamespace(workspace_path=project_root)
    prepared = prepare_kimi_launch_context(context, spec, plan, tmp_path / "runtime", {"run_cwd": str(project_root)})

    context_path = Path(str(prepared["kimi_context_path"]))
    assert context_path.is_file()
    assert context_path == paths.agent_provider_state_dir("slot1_claude", "kimi") / "home" / "CCB_KIMI_CONTEXT.md"
    assert "CCB Kimi Context Projection" in context_path.read_text(encoding="utf-8")

    command = ParsedStartCommand(project=None, agent_names=("slot1_claude",), restore=False, auto_permission=True)
    cmd = build_kimi_start_cmd(command, spec, tmp_path / "runtime", "launch-1", prepared_state=prepared)

    assert "CCB_KIMI_CONTEXT_PATH=" in cmd
    assert str(context_path) in cmd

    payload = build_kimi_session_payload(
        context,
        spec,
        plan,
        tmp_path / "runtime",
        project_root,
        "%5",
        "marker",
        cmd,
        "launch-1",
        prepared,
    )
    assert payload["kimi_context_path"] == str(context_path)
    assert payload["kimi_context_projection"] == "file"


def test_deepseek_start_cmd_defaults_to_deepcode_and_keeps_startup_args(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("DEEPSEEK_START_CMD", raising=False)
    command = ParsedStartCommand(project=None, agent_names=("deep_agent",), restore=True, auto_permission=True)
    spec = _spec("deep_agent", "deepseek", startup_args=("--raw",))

    cmd = build_deepseek_start_cmd(command, spec, tmp_path / "runtime", "launch-1")

    assert cmd.endswith("deepcode --raw")


def test_deepseek_start_cmd_supports_env_override_and_template(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("DEEPSEEK_START_CMD", "/tmp/deepcode --config demo")
    command = ParsedStartCommand(project=None, agent_names=("deep_agent",), restore=False, auto_permission=False)
    spec = _spec("deep_agent", "deepseek", provider_command_template="sandbox=1 {command}")

    cmd = build_deepseek_start_cmd(command, spec, tmp_path / "runtime", "launch-1")

    assert cmd.endswith("sandbox=1 /tmp/deepcode --config demo")
