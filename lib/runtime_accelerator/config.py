from __future__ import annotations

import os
import shutil
from pathlib import Path

from .client import default_socket_path

_ENABLED_VALUES = {"1", "true", "yes", "on", "auto"}


def codex_accelerator_enabled() -> bool:
    raw = str(os.environ.get("CCB_RUNTIME_ACCELERATOR_CODEX") or "").strip().lower()
    return raw in _ENABLED_VALUES


def accelerator_socket_path(project_root: str | Path | None) -> Path | None:
    override = str(os.environ.get("CCB_RUNTIME_ACCELERATOR_SOCKET") or "").strip()
    if override:
        return Path(override).expanduser()
    if project_root is None:
        return None
    raw_root = str(project_root or "").strip()
    if not raw_root:
        return None
    return default_socket_path(raw_root)


def accelerator_timeout_s(default: float = 0.2) -> float:
    return float_env("CCB_RUNTIME_ACCELERATOR_TIMEOUT_S", default)


def accelerator_startup_timeout_s(default: float = 0.5) -> float:
    return float_env("CCB_RUNTIME_ACCELERATOR_STARTUP_TIMEOUT_S", default)


def accelerator_binary() -> str | None:
    raw = str(os.environ.get("CCB_RUNTIME_ACCELERATOR_BIN") or "ccb-runtime-accelerator").strip()
    if not raw:
        return None
    if "/" in raw:
        return raw if Path(raw).expanduser().exists() else None
    return shutil.which(raw)


def float_env(name: str, default: float) -> float:
    try:
        return max(0.0, float(os.environ.get(name, default)))
    except Exception:
        return max(0.0, default)


__all__ = [
    "accelerator_binary",
    "accelerator_socket_path",
    "accelerator_startup_timeout_s",
    "accelerator_timeout_s",
    "codex_accelerator_enabled",
]
