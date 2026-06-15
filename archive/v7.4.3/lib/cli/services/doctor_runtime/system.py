from __future__ import annotations

import getpass
import os
from pathlib import Path
import platform
import shutil
import sys
from typing import Any

from cli.management import find_install_dir, get_version_info
from provider_core.registry import CORE_PROVIDER_NAMES, OPTIONAL_PROVIDER_NAMES
from provider_core.runtime_shared import provider_executable


def installation_summary() -> dict[str, object]:
    install_dir = find_install_dir(_script_root())
    info = get_version_info(install_dir)
    return {
        'path': str(install_dir),
        'version': info.get('version'),
        'commit': info.get('commit'),
        'date': info.get('date'),
        'channel': info.get('channel'),
        'platform': info.get('platform'),
        'arch': info.get('arch'),
        'build_time': info.get('build_time'),
        'installed_at': info.get('installed_at'),
        'source_kind': info.get('source_kind'),
        'install_mode': info.get('install_mode'),
        'install_user_id': info.get('install_user_id'),
        'install_user_name': info.get('install_user_name'),
        'root_install': info.get('root_install'),
        'sudo_user': info.get('sudo_user'),
    }


def runtime_identity_summary(
    project_root: Path,
    *,
    ccb_dir: Path | None = None,
    installation: dict[str, object] | None = None,
) -> dict[str, object]:
    installation = installation or {}
    uid = _effective_uid()
    user_name = _user_name(uid)
    home = str(Path.home())
    install_path_text = str(installation.get('path') or '').strip()
    project_owner = _path_owner(project_root)
    ccb_owner = _path_owner(ccb_dir) if ccb_dir is not None else None
    install_owner = _path_owner(Path(install_path_text)) if install_path_text else None
    install_root_owned = _install_root_owned(installation, install_owner=install_owner)
    root_runtime = uid == 0
    warnings: list[str] = []
    if root_runtime and project_owner is not None and project_owner.get('uid') not in (None, 0):
        warnings.append('Running CCB as root in a non-root-owned project can create root-owned .ccb files.')
    return {
        'user_id': uid,
        'user_name': user_name,
        'home': home,
        'root_runtime': root_runtime,
        'install_root_owned': install_root_owned,
        'install_user_id': installation.get('install_user_id'),
        'install_user_name': installation.get('install_user_name'),
        'sudo_user': installation.get('sudo_user') or os.environ.get('SUDO_USER') or None,
        'project_owner': _owner_display(project_owner),
        'ccb_dir_owner': _owner_display(ccb_owner),
        'install_owner': _owner_display(install_owner),
        'warnings': tuple(warnings),
    }


def requirements_summary() -> dict[str, object]:
    tmux_path = shutil.which('tmux')
    providers = []
    for provider in tuple(CORE_PROVIDER_NAMES + OPTIONAL_PROVIDER_NAMES):
        executable = provider_executable(provider)
        command_path = shutil.which(executable)
        providers.append(
            {
                'provider': provider,
                'executable': executable,
                'available': command_path is not None,
                'path': command_path,
            }
        )
    return {
        'python_executable': sys.executable,
        'python_version': platform.python_version(),
        'tmux_available': tmux_path is not None,
        'tmux_path': tmux_path,
        'provider_commands': providers,
    }


def _script_root() -> Path:
    return Path(__file__).resolve().parents[4]


def _effective_uid() -> int:
    getter = getattr(os, 'geteuid', None) or getattr(os, 'getuid', None)
    if getter is None:
        return -1
    try:
        return int(getter())
    except Exception:
        return -1


def _user_name(uid: int) -> str:
    try:
        import pwd

        return pwd.getpwuid(uid).pw_name
    except Exception:
        try:
            return getpass.getuser()
        except Exception:
            return 'unknown'


def _path_owner(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    try:
        stat = path.stat()
    except Exception:
        return None
    uid = int(getattr(stat, 'st_uid', -1))
    return {'uid': uid, 'name': _user_name(uid)}


def _owner_display(owner: dict[str, Any] | None) -> str | None:
    if owner is None:
        return None
    uid = owner.get('uid')
    name = owner.get('name') or 'unknown'
    return f'{uid}:{name}'


def _install_root_owned(installation: dict[str, object], *, install_owner: dict[str, Any] | None) -> bool | None:
    root_install = _coerce_bool(installation.get('root_install'))
    if root_install is not None:
        return root_install
    install_user_id = _coerce_int(installation.get('install_user_id'))
    if install_user_id is not None:
        return install_user_id == 0
    if install_owner is not None:
        owner_uid = _coerce_int(install_owner.get('uid'))
        return owner_uid == 0 if owner_uid is not None else None
    return None


def _coerce_bool(value: object) -> bool | None:
    if isinstance(value, bool):
        return value
    text = str(value or '').strip().lower()
    if text in {'1', 'true', 'yes', 'on'}:
        return True
    if text in {'0', 'false', 'no', 'off'}:
        return False
    return None


def _coerce_int(value: object) -> int | None:
    try:
        return int(str(value).strip())
    except Exception:
        return None


__all__ = ['installation_summary', 'requirements_summary', 'runtime_identity_summary']
