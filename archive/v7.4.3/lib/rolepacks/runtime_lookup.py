from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

from agents.config_loader import load_project_config
from agents.config_loader_runtime.role_lookup import role_store_roots
from agents.models import normalize_agent_name
from project_memory.types import ProjectMemorySource
from role_aliases import role_id_candidates

from .manifest import RoleManifest as RolePack
from .manifest import load_role_manifest, normalize_role_id


@dataclass(frozen=True)
class ProjectRoleResolution:
    role_id: str
    role: RolePack | None
    warning: str = ''
    lock_path: Path | None = None


def load_installed_role(role_id: str) -> RolePack | None:
    role_id = normalize_role_id(role_id)
    for store_root in role_store_roots():
        for candidate_id in role_id_candidates(role_id):
            current = store_root / candidate_id / 'current'
            if current.exists():
                try:
                    return load_role_manifest(current.resolve())
                except Exception:
                    return None
            direct = store_root / candidate_id
            if (direct / 'role.toml').is_file():
                try:
                    return load_role_manifest(direct)
                except Exception:
                    return None
    return None


def load_locked_installed_role(role_id: str, *, version: str, digest: str) -> RolePack | None:
    role_id = normalize_role_id(role_id)
    root = _locked_role_root(role_id, version=version, digest=digest)
    if root is None:
        return None
    try:
        role = load_role_manifest(root)
    except Exception:
        return None
    return role if role.id == role_id else None


def resolve_project_agent_role(project_root: Path, agent_name: str) -> ProjectRoleResolution | None:
    try:
        config = load_project_config(project_root).config
        normalized = normalize_agent_name(agent_name)
        spec = config.agents.get(normalized)
        role_id = str(getattr(spec, 'role', '') or '').strip()
        if not role_id:
            return None
        role_id = normalize_role_id(role_id)
    except Exception:
        return None

    lock_path = project_role_lock_path(project_root)
    try:
        lock_entry = project_role_lock_entry(project_root, role_id)
    except ValueError as exc:
        return ProjectRoleResolution(role_id=role_id, role=None, warning=str(exc), lock_path=lock_path)
    if lock_entry is not None:
        locked_role = load_locked_installed_role(
            role_id,
            version=str(lock_entry.get('version') or '').strip(),
            digest=str(lock_entry.get('digest') or '').strip(),
        )
        if locked_role is not None:
            return ProjectRoleResolution(role_id=role_id, role=locked_role, lock_path=lock_path)

    role = load_installed_role(role_id)
    if role is None:
        return ProjectRoleResolution(
            role_id=role_id,
            role=None,
            warning=f'role_not_installed: {role_id}; run `ccb roles install {role_id}`',
            lock_path=lock_path,
        )
    warning = project_role_lock_warning(project_root, role)
    if warning:
        return ProjectRoleResolution(role_id=role_id, role=None, warning=warning, lock_path=lock_path)
    return ProjectRoleResolution(role_id=role_id, role=role, lock_path=lock_path)


def load_project_agent_role(project_root: Path, agent_name: str) -> RolePack | None:
    resolved = resolve_project_agent_role(project_root, agent_name)
    return resolved.role if resolved is not None else None


def project_role_memory_sources(project_root: Path, agent_name: str) -> tuple[ProjectMemorySource, ...]:
    resolved = resolve_project_agent_role(project_root, agent_name)
    if resolved is None:
        return ()
    if resolved.warning:
        lock_path = resolved.lock_path or project_role_lock_path(project_root)
        return (
            ProjectMemorySource(
                kind='role_memory',
                title=f'Role Memory: {resolved.role_id}',
                path=lock_path,
                content='',
                exists=lock_path.exists(),
                warning=resolved.warning,
            ),
        )
    role = resolved.role
    if role is None:
        return ()
    memory = dict(role.manifest.get('memory') or {})
    sources: list[ProjectMemorySource] = []
    for raw_path in memory.get('files', ()) or ():
        relative = Path(str(raw_path))
        if relative.is_absolute():
            continue
        path = role.root / relative
        if not path.is_file():
            continue
        try:
            content = path.read_text(encoding='utf-8')
        except OSError as exc:
            sources.append(
                ProjectMemorySource(
                    kind='role_memory',
                    title=f'Role Memory: {role.id}',
                    path=path,
                    content='',
                    exists=True,
                    warning=f'failed_to_read_role_memory: {exc}',
                )
            )
            continue
        sources.append(
            ProjectMemorySource(
                kind='role_memory',
                title=f'Role Memory: {role.id}',
                path=path,
                content=content,
                exists=True,
            )
        )
    return tuple(sources)


def project_role_skill_sources(project_root: Path, agent_name: str, provider: str) -> tuple[tuple[str, Path, str], ...]:
    resolved = resolve_project_agent_role(project_root, agent_name)
    if resolved is None or resolved.warning or resolved.role is None:
        return ()
    role = resolved.role
    skills = dict(role.manifest.get('skills') or {})
    provider_name = str(provider or '').strip().lower()
    sources: list[tuple[str, Path, str]] = []
    for raw_path in skills.get(provider_name, ()) or ():
        relative = Path(str(raw_path))
        if relative.is_absolute():
            continue
        source = role.root / relative
        if not source.is_dir():
            continue
        sources.append((source.name, source, role.id))
    return tuple(sources)


def project_role_lock_path(project_root: Path) -> Path:
    return Path(project_root).expanduser().resolve() / '.ccb' / 'role-lock.json'


def project_role_lock_warning(project_root: Path, role: RolePack) -> str:
    try:
        entry = project_role_lock_entry(project_root, role.id)
    except ValueError as exc:
        return str(exc)
    if entry is None:
        return ''
    locked_version = str(entry.get('version') or '').strip()
    locked_digest = str(entry.get('digest') or '').strip()
    locked_role = load_locked_installed_role(role.id, version=locked_version, digest=locked_digest)
    if locked_role is not None:
        return ''
    current_digest = f'sha256:{tree_digest(role.root)}'
    if locked_version == role.version and locked_digest == current_digest:
        return ''
    return (
        f'role_lock_mismatch: {role.id} locked version={locked_version or "unknown"} '
        f'digest={locked_digest or "unknown"} but installed current is '
        f'version={role.version} digest={current_digest}; run `ccb` interactively '
        'and accept role lock refresh to adopt the installed role version'
    )


def project_role_lock_entry(project_root: Path, role_id: str) -> dict[str, object] | None:
    lock_path = project_role_lock_path(project_root)
    try:
        payload = json.loads(lock_path.read_text(encoding='utf-8'))
    except FileNotFoundError:
        return None
    except Exception as exc:
        raise ValueError(f'role_lock_unreadable: {lock_path}: {exc}') from exc
    if not isinstance(payload, dict):
        raise ValueError(f'role_lock_invalid: {lock_path}')
    roles = payload.get('roles') or {}
    if not isinstance(roles, dict):
        raise ValueError(f'role_lock_invalid: {lock_path}')
    entry = None
    for candidate_id in role_id_candidates(role_id):
        entry = roles.get(candidate_id)
        if entry is not None:
            break
    if not isinstance(entry, dict):
        return None
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
            if (version_root / 'role.toml').is_file() and f'sha256:{tree_digest(version_root)}' == digest_text:
                return version_root
    return None


def tree_digest(root: Path) -> str:
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


__all__ = [
    'ProjectRoleResolution',
    'load_installed_role',
    'load_locked_installed_role',
    'load_project_agent_role',
    'project_role_lock_entry',
    'project_role_lock_path',
    'project_role_lock_warning',
    'project_role_memory_sources',
    'project_role_skill_sources',
    'resolve_project_agent_role',
    'role_store_roots',
    'tree_digest',
]
