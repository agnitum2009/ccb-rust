from __future__ import annotations

import importlib
import os
from pathlib import Path
from typing import Any


class RoleLookupError(ValueError):
    pass


def role_store_root() -> Path:
    data_home = os.environ.get('XDG_DATA_HOME')
    base = Path(data_home).expanduser() if data_home else Path.home() / '.local' / 'share'
    return base / 'ccb' / 'roles'


def normalize_role_id(value: str) -> str:
    role_id = str(value or '').strip().lower()
    if not role_id or '.' not in role_id:
        raise RoleLookupError('role id must use publisher.role form, for example ccb.archi')
    allowed = set('abcdefghijklmnopqrstuvwxyz0123456789._-')
    if any(ch not in allowed for ch in role_id):
        raise RoleLookupError(f'invalid role id: {value!r}')
    return role_id


def looks_like_role_id(value: str) -> bool:
    try:
        normalize_role_id(value)
        return True
    except RoleLookupError:
        return False


def load_installed_role_manifest(role_id: str) -> tuple[Path, dict[str, Any]]:
    role_id = normalize_role_id(role_id)
    root = role_store_root() / role_id
    current = root / 'current'
    if current.exists():
        role_root = current.resolve()
    else:
        role_root = root
    manifest_path = role_root / 'role.toml'
    if not manifest_path.exists():
        raise RoleLookupError(f'role {role_id} is not installed; run `ccb roles install {role_id}`')
    if not manifest_path.is_file():
        raise RoleLookupError(f'role {role_id} has invalid manifest path: {manifest_path}')
    manifest = _load_toml(manifest_path)
    actual_id = normalize_role_id(str(manifest.get('id') or ''))
    if actual_id != role_id:
        raise RoleLookupError(f'role {role_id} manifest id mismatch: {actual_id}')
    return role_root, manifest


def installed_role_default_agent_name(role_id: str) -> str:
    _root, manifest = load_installed_role_manifest(role_id)
    identity = manifest.get('identity') or {}
    if not isinstance(identity, dict):
        raise RoleLookupError(f'role {role_id} identity must be a table')
    default_name = str(identity.get('default_agent_name') or role_id.rsplit('.', 1)[-1]).strip()
    if not default_name:
        raise RoleLookupError(f'role {role_id} identity.default_agent_name cannot be empty')
    return default_name


def _load_toml(path: Path) -> dict[str, Any]:
    for module_name in ('tomllib', 'tomli', 'toml'):
        try:
            module = importlib.import_module(module_name)
            break
        except ModuleNotFoundError:
            module = None
    if module is None:
        raise RoleLookupError('TOML parsing requires Python 3.11+ or tomli/toml')
    try:
        payload = module.loads(path.read_text(encoding='utf-8'))
    except Exception as exc:
        raise RoleLookupError(f'invalid role manifest {path}: {exc}') from exc
    if not isinstance(payload, dict):
        raise RoleLookupError(f'role manifest must decode to a table: {path}')
    return dict(payload)


__all__ = [
    'RoleLookupError',
    'installed_role_default_agent_name',
    'load_installed_role_manifest',
    'looks_like_role_id',
    'normalize_role_id',
    'role_store_root',
]
