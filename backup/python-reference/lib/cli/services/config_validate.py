from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from agents.config_loader import ConfigValidationError, load_project_config
from agents.config_loader_runtime.io_runtime.documents import _load_config_document
from agents.config_loader_runtime.parsing_runtime.validation import _expand_role_id_shorthand
from agents.models import AgentValidationError, normalize_agent_name, parse_layout_spec
from cli.context import CliContext
from provider_profiles import validate_provider_runtime_home_uniqueness


@dataclass(frozen=True)
class ConfigValidationSummary:
    project_root: str
    project_id: str
    source: str | None
    source_kind: str
    used_builtin_default: bool
    default_agents: tuple[str, ...]
    agent_names: tuple[str, ...]
    cmd_enabled: bool
    layout_spec: str
    style_warnings: tuple[str, ...] = ()


def validate_config_context(context: CliContext) -> ConfigValidationSummary:
    result = load_project_config(context.project.project_root)
    try:
        validate_provider_runtime_home_uniqueness(layout=context.paths, specs=result.config.agents.values())
    except ValueError as exc:
        raise ConfigValidationError(str(exc)) from exc
    return ConfigValidationSummary(
        project_root=str(context.project.project_root),
        project_id=context.project.project_id,
        source=str(result.source_path) if result.source_path else None,
        source_kind=result.source_kind,
        used_builtin_default=result.used_default,
        default_agents=result.config.default_agents,
        agent_names=tuple(sorted(result.config.agents)),
        cmd_enabled=bool(result.config.cmd_enabled),
        layout_spec=str(result.config.layout_spec or ''),
        style_warnings=_config_style_warnings(
            source_path=result.source_path,
            project_root=context.project.project_root,
        ),
    )


def _config_style_warnings(*, source_path: Path | None, project_root: Path) -> tuple[str, ...]:
    if source_path is None or not Path(source_path).is_file():
        return ()
    try:
        document = _load_config_document(Path(source_path), project_root=project_root)
        document = _expand_role_id_shorthand(document, project_root=project_root)
    except Exception:
        return ()
    raw_windows = document.get('windows')
    raw_agents = document.get('agents')
    if not isinstance(raw_windows, dict) or not isinstance(raw_agents, dict):
        return ()

    leaf_defaults = _window_leaf_defaults(raw_windows)
    warnings: list[str] = []
    for raw_name, raw_spec in raw_agents.items():
        if not isinstance(raw_name, str) or not isinstance(raw_spec, dict):
            continue
        try:
            agent_name = normalize_agent_name(raw_name)
        except AgentValidationError:
            continue
        defaults = leaf_defaults.get(agent_name)
        if defaults is None:
            warnings.append(
                f'stale_agent_overlay: agents.{raw_name} is ignored because it is not referenced by [windows]'
            )
            continue
        _agent_overlay_style_warnings(
            warnings,
            raw_name=raw_name,
            raw_spec=raw_spec,
            leaf_defaults=defaults,
        )
    return tuple(warnings)


def _window_leaf_defaults(raw_windows: dict[object, object]) -> dict[str, dict[str, str]]:
    defaults: dict[str, dict[str, str]] = {}
    for layout_text in raw_windows.values():
        try:
            layout = parse_layout_spec(str(layout_text))
        except Exception:
            continue
        for leaf in layout.iter_leaves():
            if str(leaf.name or '').strip().lower() == 'cmd':
                continue
            try:
                agent_name = normalize_agent_name(str(leaf.name or ''))
            except AgentValidationError:
                continue
            defaults[agent_name] = {
                'provider': str(leaf.provider or '').strip().lower(),
                'workspace_mode': 'git-worktree'
                if str(leaf.workspace_mode or '').strip() == 'worktree'
                else 'inplace',
            }
    return defaults


def _agent_overlay_style_warnings(
    warnings: list[str],
    *,
    raw_name: str,
    raw_spec: dict[str, Any],
    leaf_defaults: dict[str, str],
) -> None:
    provider = raw_spec.get('provider')
    if provider is not None and str(provider).strip().lower() == leaf_defaults.get('provider'):
        warnings.append(
            f'redundant_agent_provider: agents.{raw_name}.provider repeats [windows] and should be removed'
        )
    workspace_mode = raw_spec.get('workspace_mode')
    if workspace_mode is None:
        return
    normalized = str(workspace_mode).strip().lower()
    if normalized == leaf_defaults.get('workspace_mode'):
        warnings.append(
            f'redundant_agent_workspace_mode: agents.{raw_name}.workspace_mode repeats [windows] and should be removed'
        )
        return
    if normalized in {'inplace', 'git-worktree'}:
        warnings.append(
            f'agent_workspace_mode_override: agents.{raw_name}.workspace_mode overrides [windows]; prefer encoding inplace/git-worktree in the window leaf'
        )
