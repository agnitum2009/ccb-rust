from __future__ import annotations

import importlib
import hashlib
import json
import os
from pathlib import Path
from typing import Any

from role_aliases import canonical_role_id, role_id_candidates


class RoleLookupError(ValueError):
    pass


def role_store_root() -> Path:
    data_home = os.environ.get('XDG_DATA_HOME')
    base = Path(data_home).expanduser() if data_home else Path.home() / '.local' / 'share'
    return base / 'ccb' / 'roles'


def agent_roles_store_root() -> Path:
    value = str(os.environ.get('AGENT_ROLES_STORE') or '').strip()
    if value:
        return Path(value).expanduser()
    return Path.home() / '.roles'


def agent_roles_installed_root() -> Path:
    return agent_roles_store_root() / 'installed'


def role_store_roots() -> tuple[Path, ...]:
    return (agent_roles_installed_root(),)


def normalize_role_id(value: str) -> str:
    role_id = str(value or '').strip().lower()
    if not role_id or '.' not in role_id:
        raise RoleLookupError('role id must use publisher.role form, for example agentroles.archi')
    allowed = set('abcdefghijklmnopqrstuvwxyz0123456789._-')
    if any(ch not in allowed for ch in role_id):
        raise RoleLookupError(f'invalid role id: {value!r}')
    return canonical_role_id(role_id)


def looks_like_role_id(value: str) -> bool:
    try:
        normalize_role_id(value)
        return True
    except RoleLookupError:
        return False


def load_installed_role_manifest(role_id: str) -> tuple[Path, dict[str, Any]]:
    role_id = normalize_role_id(role_id)
    role_root = None
    root = None
    for store_root in role_store_roots():
        for candidate_id in role_id_candidates(role_id):
            candidate_root = store_root / candidate_id
            current = candidate_root / 'current'
            if current.exists():
                role_root = current.resolve()
                root = candidate_root
                break
            if (candidate_root / 'role.toml').is_file():
                role_root = candidate_root
                root = candidate_root
                break
        if role_root is not None:
            break
    if role_root is None or root is None:
        root = agent_roles_installed_root() / role_id
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


def load_locked_role_manifest(role_id: str, *, version: str, digest: str) -> tuple[Path, dict[str, Any]] | None:
    role_id = normalize_role_id(role_id)
    role_root = _locked_role_root(role_id, version=version, digest=digest)
    if role_root is None:
        return None
    manifest_path = role_root / 'role.toml'
    if not manifest_path.is_file():
        return None
    manifest = _load_toml(manifest_path)
    actual_id = normalize_role_id(str(manifest.get('id') or ''))
    if actual_id != role_id:
        raise RoleLookupError(f'role {role_id} manifest id mismatch: {actual_id}')
    return role_root, manifest


def installed_role_default_agent_name(role_id: str, *, project_root: Path | None = None) -> str:
    role_id = normalize_role_id(role_id)
    lock_entry = _project_role_lock_entry(project_root, role_id)
    if lock_entry is not None:
        locked = load_locked_role_manifest(
            role_id,
            version=str(lock_entry.get('version') or '').strip(),
            digest=str(lock_entry.get('digest') or '').strip(),
        )
        if locked is not None:
            root, manifest = locked
            return _default_agent_name_from_manifest(role_id, root, manifest)
        locked_name = _project_locked_default_agent_name(lock_entry)
        if locked_name is not None:
            return locked_name
    root, manifest = load_installed_role_manifest(role_id)
    if project_root is not None:
        warning = _project_role_lock_warning(
            lock_entry,
            role_id=role_id,
            role_root=root,
            role_version=str(manifest.get('version') or '').strip(),
        )
        if warning:
            raise RoleLookupError(warning)
    return _default_agent_name_from_manifest(role_id, root, manifest)


def _default_agent_name_from_manifest(role_id: str, root: Path, manifest: dict[str, Any]) -> str:
    identity = manifest.get('identity') or {}
    if not isinstance(identity, dict):
        raise RoleLookupError(f'role {role_id} identity must be a table')
    default_name = str(
        identity.get('default_agent_name')
        or identity.get('default_name')
        or _ccb_adapter_default_agent_name(root)
        or role_id.rsplit('.', 1)[-1]
    ).strip()
    if not default_name:
        raise RoleLookupError(f'role {role_id} identity.default_agent_name cannot be empty')
    return default_name


def _project_locked_default_agent_name(entry: dict[str, Any]) -> str | None:
    locked_name = str(entry.get('default_agent_name') or '').strip()
    if not locked_name:
        return None
    return locked_name


def _project_role_lock_warning(
    entry: dict[str, Any] | None,
    *,
    role_id: str,
    role_root: Path,
    role_version: str,
) -> str:
    if entry is None:
        return ''
    locked_role_root = _locked_role_root(
        role_id,
        version=str(entry.get('version') or '').strip(),
        digest=str(entry.get('digest') or '').strip(),
    )
    if locked_role_root is not None:
        return ''
    locked_version = str(entry.get('version') or '').strip()
    locked_digest = str(entry.get('digest') or '').strip()
    current_digest = f'sha256:{_tree_digest(role_root)}'
    if locked_version == role_version and locked_digest == current_digest:
        return ''
    return (
        f'role_lock_mismatch: {role_id} locked version={locked_version or "unknown"} '
        f'digest={locked_digest or "unknown"} but installed current is '
        f'version={role_version or "unknown"} digest={current_digest}; run `ccb` interactively '
        'and accept role lock refresh to adopt the installed role version'
    )


def _project_role_lock_entry(project_root: Path | None, role_id: str) -> dict[str, Any] | None:
    if project_root is None:
        return None
    lock_path = Path(project_root).expanduser().resolve() / '.ccb' / 'role-lock.json'
    try:
        payload = json.loads(lock_path.read_text(encoding='utf-8'))
    except FileNotFoundError:
        return None
    except Exception as exc:
        raise RoleLookupError(f'role_lock_unreadable: {lock_path}: {exc}') from exc
    if not isinstance(payload, dict):
        raise RoleLookupError(f'role_lock_invalid: {lock_path}')
    roles = payload.get('roles') or {}
    if not isinstance(roles, dict):
        raise RoleLookupError(f'role_lock_invalid: {lock_path}')
    entry = None
    for candidate_id in role_id_candidates(role_id):
        entry = roles.get(candidate_id)
        if entry is not None:
            break
    if entry is None:
        return None
    if not isinstance(entry, dict):
        raise RoleLookupError(f'role_lock_invalid: {lock_path}')
    return dict(entry)


def _locked_role_root(role_id: str, *, version: str, digest: str) -> Path | None:
    version_text = str(version or '').strip()
    digest_text = str(digest or '').strip()
    if not version_text or not digest_text:
        return None
    digest_hex = digest_text.removeprefix('sha256:')
    if not digest_hex:
        return None
    for store_root in role_store_roots():
        for candidate_id in role_id_candidates(role_id):
            version_root = store_root / candidate_id / 'versions' / version_text
            candidate = version_root / digest_hex
            if (candidate / 'role.toml').is_file():
                return candidate
            if (version_root / 'role.toml').is_file() and f'sha256:{_tree_digest(version_root)}' == digest_text:
                return version_root
    return None


def _tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(Path(root).rglob('*')):
        rel = path.relative_to(root)
        digest.update(str(rel).encode('utf-8'))
        digest.update(b'\0')
        if path.is_file():
            digest.update(path.read_bytes())
        elif path.is_symlink():
            digest.update(str(path.readlink()).encode('utf-8'))
        digest.update(b'\0')
    return digest.hexdigest()


def _ccb_adapter_default_agent_name(role_root: Path) -> str | None:
    adapter_path = Path(role_root) / 'adapters' / 'ccb' / 'adapter.toml'
    if not adapter_path.is_file():
        return None
    try:
        adapter = _load_toml(adapter_path)
    except RoleLookupError:
        return None
    default_name = str(adapter.get('default_agent_name') or '').strip()
    return default_name or None


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
    'agent_roles_installed_root',
    'agent_roles_store_root',
    'installed_role_default_agent_name',
    'load_locked_role_manifest',
    'load_installed_role_manifest',
    'looks_like_role_id',
    'normalize_role_id',
    'role_store_root',
    'role_store_roots',
]
