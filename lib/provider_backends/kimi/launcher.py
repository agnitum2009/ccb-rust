from __future__ import annotations

from datetime import datetime, timezone
import shlex
from pathlib import Path

from agents.models import AgentSpec
from cli.context import CliContext
from cli.models import ParsedStartCommand
from provider_core.caller_env import (
    caller_context_env,
    export_env_clause,
    join_env_prefix,
    provider_user_session_env,
)
from provider_core.contracts import ProviderRuntimeLauncher
from provider_core.runtime_shared import apply_provider_command_template, provider_start_parts
from project_memory.materializer import materialize_runtime_memory_bundle
from storage.atomic import atomic_write_text
from workspace.models import WorkspacePlan


_AUTO_FLAG = "--auto-approve"
_AUTO_FLAGS = {"--auto-approve", "--auto", "--yes", "-y", "--yolo"}


def build_runtime_launcher() -> ProviderRuntimeLauncher:
    return ProviderRuntimeLauncher(
        provider="kimi",
        launch_mode="simple_tmux",
        prepare_launch_context=prepare_launch_context,
        build_start_cmd=build_start_cmd,
        build_session_payload=build_session_payload,
    )


def prepare_launch_context(
    context: CliContext,
    spec: AgentSpec,
    plan: WorkspacePlan,
    runtime_dir: Path,
    prepared_state: dict[str, object],
) -> dict[str, object]:
    del runtime_dir
    payload = dict(prepared_state or {})
    payload["agent_name"] = spec.name
    payload["project_root"] = str(context.project.project_root)
    payload["workspace_path"] = str(prepared_state.get("run_cwd") or plan.workspace_path)
    payload["agent_events_path"] = str(context.paths.agent_events_path(spec.name))
    context_path = _materialize_kimi_context(context, spec, payload["workspace_path"])
    if context_path is not None:
        payload["kimi_context_path"] = str(context_path)
    return payload


def build_start_cmd(
    command: ParsedStartCommand,
    spec: AgentSpec,
    runtime_dir,
    launch_session_id: str,
    *,
    prepared_state: dict[str, object] | None = None,
) -> str:
    runtime_dir = Path(runtime_dir)
    cmd_parts = provider_start_parts("kimi")
    if command.auto_permission and not _has_any(cmd_parts, _AUTO_FLAGS) and not _has_any(spec.startup_args, _AUTO_FLAGS):
        cmd_parts.append(_AUTO_FLAG)
    cmd_parts.extend(spec.startup_args)
    cmd = " ".join(shlex.quote(str(part)) for part in cmd_parts)
    cmd = apply_provider_command_template(cmd, spec.provider_command_template)
    env_prefix = join_env_prefix(
        export_env_clause(provider_user_session_env()),
        export_env_clause(spec.env),
        export_env_clause(_context_env(prepared_state)),
        export_env_clause(
            caller_context_env(actor=spec.name, runtime_dir=runtime_dir, launch_session_id=launch_session_id)
        ),
    )
    if env_prefix:
        return f"{env_prefix}; {cmd}"
    return cmd


def build_session_payload(
    context: CliContext,
    spec: AgentSpec,
    plan: WorkspacePlan,
    runtime_dir,
    run_cwd,
    pane_id: str,
    pane_title_marker: str,
    start_cmd: str,
    launch_session_id: str,
    prepared_state: dict[str, object],
) -> dict[str, object]:
    payload = {
        "ccb_session_id": launch_session_id,
        "agent_name": spec.name,
        "ccb_project_id": context.project.project_id,
        "runtime_dir": str(runtime_dir),
        "completion_artifact_dir": str(runtime_dir / "completion"),
        "terminal": "tmux",
        "tmux_session": pane_id,
        "pane_id": pane_id,
        "pane_title_marker": pane_title_marker,
        "workspace_path": str(plan.workspace_path),
        "work_dir": str(run_cwd),
        "start_dir": str(context.project.project_root),
        "start_cmd": start_cmd,
    }
    context_path = str(prepared_state.get("kimi_context_path") or "").strip()
    if context_path:
        payload["kimi_context_path"] = context_path
        payload["kimi_context_projection"] = "file"
        payload["kimi_context_projection_version"] = 1
    return payload


def _has_any(parts: tuple[str, ...] | list[str], flags: set[str]) -> bool:
    normalized = {str(part).strip() for part in parts}
    return bool(flags & normalized)


def _context_env(prepared_state: dict[str, object] | None) -> dict[str, str]:
    context_path = str((prepared_state or {}).get("kimi_context_path") or "").strip()
    return {"CCB_KIMI_CONTEXT_PATH": context_path} if context_path else {}


def _materialize_kimi_context(context: CliContext, spec: AgentSpec, workspace_path: object) -> Path | None:
    provider_home = context.paths.agent_provider_state_dir(spec.name, "kimi") / "home"
    provider_home.mkdir(parents=True, exist_ok=True)
    memory = materialize_runtime_memory_bundle(
        context.project.project_root,
        agent_name=spec.name,
        provider="kimi",
        workspace_path=Path(str(workspace_path)),
        now=_utc_now(),
    )
    target = provider_home / "CCB_KIMI_CONTEXT.md"
    agents_path = context.project.project_root / "AGENTS.md"
    ccb_memory_path = context.paths.project_memory_path
    rendered = "\n".join(
        [
            "# CCB Kimi Context Projection",
            "",
            "This file is generated by CCB for Kimi panes. Kimi does not load Codex/Claude skills directly.",
            "Read and follow this context for every CCB task in this project.",
            "",
            "## Authority",
            f"- Agent: {spec.name}",
            "- Provider: kimi",
            f"- Project root: {context.project.project_root}",
            f"- Workspace path: {workspace_path}",
            f"- AGENTS authority: {agents_path if agents_path.exists() else 'missing'}",
            f"- CCB shared memory: {ccb_memory_path if ccb_memory_path.exists() else 'missing'}",
            f"- Runtime memory bundle: {memory.path if memory.path else 'missing'}",
            "",
            "## Critical CCB Rules",
            "- CCB ask is submit-only unless diagnostics are explicitly requested.",
            "- Completed implementation receipts are not review pass, archive, or production acceptance.",
            "- Lifecycle/archive changes must go through governed writers, CAS, EventJournal, and reconcile paths.",
            "- Keep source graph, runtime graph, and lifecycle truth separate.",
            "- Runtime claims require PID, command, cwd/source path, port, and smoke evidence.",
            "- Use DDD-first boundaries for business code: truth owner, evidence, canonical state, command/admission, receipt/audit.",
            "- Do not make BFF/UI/cache/read-model/provider session state own production truth.",
            "- Keep files bounded; prefer <=400 lines and <=20KB where this project declares that budget.",
            "",
            "## Collaboration Shape",
            "- Kimi slots are execution channels for coding/debugging.",
            "- Paired Codex channels are for lightweight review, adversarial checks, and optimization guidance.",
            "- Replies should be concise implementation receipts: changed files, commands/results, proved claims, risks/blockers.",
            "",
        ]
    )
    try:
        atomic_write_text(target, rendered)
    except OSError:
        return None
    return target


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


__all__ = ["build_runtime_launcher", "build_start_cmd", "prepare_launch_context"]
