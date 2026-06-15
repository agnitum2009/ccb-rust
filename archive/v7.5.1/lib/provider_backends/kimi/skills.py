from __future__ import annotations

import os
from collections.abc import Mapping
from pathlib import Path

from provider_core.inherited_skills import inherits_skills, route_packaged_inherited_skills_dir
from rolepacks.projection import project_role_skills_to_home


_KIMI_INHERITED_SKILLS_LABEL = 'kimi-inherited-skills'


def kimi_skill_dirs_for_state_dir(state_dir: Path) -> tuple[Path, Path]:
    root = Path(state_dir)
    return root / 'inherited-skills', root / 'role-skills'


def kimi_skill_dirs_for_launch(
    *,
    project_root: Path | None,
    workspace_path: Path | None,
    state_dir: Path,
    env: Mapping[str, object] | None = None,
) -> tuple[Path, ...]:
    return _dedupe_paths(
        (
            *kimi_default_skill_dirs(project_root=project_root, workspace_path=workspace_path, env=env),
            *kimi_skill_dirs_for_state_dir(state_dir),
        )
    )


def kimi_default_skill_dirs(
    *,
    project_root: Path | None,
    workspace_path: Path | None,
    env: Mapping[str, object] | None = None,
) -> tuple[Path, ...]:
    source = dict(os.environ)
    if env:
        source.update({str(key): str(value) for key, value in env.items() if value is not None})
    home = _env_path(source, 'HOME') or Path.home()
    paths: list[Path] = []
    for root in _project_skill_roots(project_root=project_root, workspace_path=workspace_path):
        paths.extend(
            (
                root / '.kimi' / 'skills',
                root / '.claude' / 'skills',
                root / '.codex' / 'skills',
                root / '.agents' / 'skills',
            )
        )
    paths.extend(
        (
            home / '.kimi' / 'skills',
            home / '.claude' / 'skills',
            home / '.codex' / 'skills',
            home / '.config' / 'agents' / 'skills',
            home / '.agents' / 'skills',
        )
    )
    kimi_code_home = _env_path(source, 'KIMI_CODE_HOME')
    if kimi_code_home is not None:
        paths.append(kimi_code_home / 'skills')
    else:
        paths.append(home / '.kimi-code' / 'skills')
    for root in _project_skill_roots(project_root=project_root, workspace_path=workspace_path):
        paths.append(root / '.kimi-code' / 'skills')
    return _dedupe_paths(paths)


def materialize_kimi_skills(
    *,
    project_root: Path | None,
    agent_name: str,
    state_dir: Path,
    profile,
) -> tuple[Path, ...]:
    inherited_dir, role_dir = kimi_skill_dirs_for_state_dir(state_dir)
    active_dirs: list[Path] = []
    if route_packaged_inherited_skills_dir(
        provider='kimi',
        target_dir=inherited_dir,
        enabled=inherits_skills(profile),
        label=_KIMI_INHERITED_SKILLS_LABEL,
    ):
        active_dirs.append(inherited_dir)
    project_role_skills_to_home(
        project_root=project_root,
        agent_name=agent_name,
        provider='kimi',
        target_skills_dir=role_dir,
    )
    if role_dir.is_dir():
        active_dirs.append(role_dir)
    return tuple(active_dirs)


def _project_skill_roots(*, project_root: Path | None, workspace_path: Path | None) -> tuple[Path, ...]:
    candidates: list[Path] = []
    if workspace_path is not None:
        candidates.append(_nearest_git_root(Path(workspace_path)))
    if project_root is not None:
        candidates.append(Path(project_root))
    return _dedupe_paths(candidates)


def _nearest_git_root(path: Path) -> Path:
    start = path if path.is_dir() else path.parent
    current = start.resolve() if start.exists() else start
    while True:
        if (current / '.git').exists():
            return current
        parent = current.parent
        if parent == current:
            return start
        current = parent


def _env_path(env: Mapping[str, object], key: str) -> Path | None:
    value = str(env.get(key) or '').strip()
    if not value:
        return None
    return Path(value).expanduser()


def _dedupe_paths(paths) -> tuple[Path, ...]:
    seen: set[str] = set()
    result: list[Path] = []
    for raw in paths:
        path = Path(raw).expanduser()
        key = str(path)
        if not key or key in seen:
            continue
        seen.add(key)
        result.append(path)
    return tuple(result)


__all__ = [
    'kimi_default_skill_dirs',
    'kimi_skill_dirs_for_launch',
    'kimi_skill_dirs_for_state_dir',
    'materialize_kimi_skills',
]
