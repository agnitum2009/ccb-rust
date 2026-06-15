from __future__ import annotations

import os
from pathlib import Path

try:
    import pwd
except Exception:  # pragma: no cover - Windows fallback
    pwd = None


def current_provider_source_home() -> Path:
    explicit = _env_path('CCB_SOURCE_HOME')
    if explicit is not None:
        return explicit

    env_home = _env_path('HOME')
    if env_home is not None and not _looks_like_ccb_provider_home(env_home):
        return env_home

    account_home = _account_home_root()
    if account_home is not None:
        return account_home

    if env_home is not None:
        return env_home
    return Path.home().expanduser()


def _env_path(name: str) -> Path | None:
    raw = str(os.environ.get(name) or '').strip()
    if not raw:
        return None
    try:
        return Path(raw).expanduser()
    except Exception:
        return None


def _account_home_root() -> Path | None:
    if pwd is None:
        return _env_path('USERPROFILE')
    try:
        raw = pwd.getpwuid(os.getuid()).pw_dir
    except Exception:
        return _env_path('USERPROFILE')
    try:
        return Path(raw).expanduser() if raw else None
    except Exception:
        return None


def _looks_like_ccb_provider_home(path: Path) -> bool:
    parts = Path(path).expanduser().parts
    for index in range(0, max(len(parts) - 4, 0)):
        if parts[index] != 'agents':
            continue
        if parts[index + 2] == 'provider-state' and parts[index + 4] == 'home':
            return True
    return False


__all__ = ['current_provider_source_home']
