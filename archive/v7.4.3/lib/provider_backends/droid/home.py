from __future__ import annotations

import os
from pathlib import Path

from provider_core.projected_assets import route_projected_tree
from provider_core.source_home import current_provider_source_home

_DROID_SKILLS_PROJECTION_LABEL = 'droid-inherited-skills'


def managed_droid_home_for_runtime(runtime_dir: Path) -> Path:
    runtime_dir = Path(runtime_dir).expanduser()
    if runtime_dir.parent.name == 'provider-runtime':
        return runtime_dir.parent.parent / 'provider-state' / 'droid' / 'home'
    return runtime_dir / 'droid-home'


def materialize_droid_home_config(
    target_home: Path,
    *,
    profile=None,
    source_home: Path | None = None,
) -> Path:
    target_home = Path(target_home).expanduser()
    source_home = Path(source_home).expanduser() if source_home is not None else _system_factory_home()
    target_home.mkdir(parents=True, exist_ok=True)
    (target_home / 'sessions').mkdir(parents=True, exist_ok=True)
    _route_inherited_tree(
        source_home / 'skills',
        target_home / 'skills',
        enabled=_inherits_skills(profile),
        label=_DROID_SKILLS_PROJECTION_LABEL,
    )
    return target_home


def _route_inherited_tree(source: Path, target: Path, *, enabled: bool, label: str) -> None:
    route_projected_tree(source, target, enabled=enabled, label=label, allow_unmarked_replace=True)


def _system_factory_home() -> Path:
    if os.environ.get('CCB_SOURCE_HOME'):
        return current_provider_source_home() / '.factory'
    for name in ('FACTORY_HOME', 'FACTORY_ROOT'):
        raw = str(os.environ.get(name) or '').strip()
        if not raw:
            continue
        candidate = Path(raw).expanduser()
        if not _looks_like_ccb_provider_home(candidate):
            return candidate
    return current_provider_source_home() / '.factory'


def _inherits_skills(profile) -> bool:
    return True if profile is None else bool(getattr(profile, 'inherit_skills', True))


def _looks_like_ccb_provider_home(path: Path) -> bool:
    parts = Path(path).expanduser().parts
    for index in range(0, max(len(parts) - 4, 0)):
        if parts[index] != 'agents':
            continue
        if parts[index + 2] == 'provider-state' and parts[index + 4] == 'home':
            return True
    return False


__all__ = ['managed_droid_home_for_runtime', 'materialize_droid_home_config']
