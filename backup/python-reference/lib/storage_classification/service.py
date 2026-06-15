from __future__ import annotations

from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
import json
import os

from storage.paths import PathLayout

from .models import StorageClass, StorageEntry
from .provider_home import classify_provider_home

SCHEMA_VERSION = 1

_CCBD_AUTHORITY_FILES = {
    'keeper.json',
    'lease.json',
    'lifecycle.json',
    'restore-report.json',
    'shutdown-intent.json',
    'shutdown-report.json',
    'start-policy.json',
    'startup-report.json',
    'state.json',
}
_CCBD_RUNTIME_DIRS = {'heartbeats', 'leases', 'cursors'}
_AGENT_AUTHORITY_FILES = {'agent.json', 'runtime.json', 'helper.json', 'restore.json', 'provider.json'}
def summarize_storage(context_or_layout) -> dict[str, object]:
    layout = _layout_from_context(context_or_layout)
    entries = list(_scan_layout(layout))
    return _summary_payload(layout, entries)


def _layout_from_context(context_or_layout) -> PathLayout:
    if isinstance(context_or_layout, PathLayout):
        return context_or_layout
    return context_or_layout.paths


def _scan_layout(layout: PathLayout) -> tuple[StorageEntry, ...]:
    roots: list[tuple[str, Path]] = [('project', layout.ccb_dir)]
    try:
        if layout.runtime_state_root != layout.ccb_dir:
            roots.append(('runtime', layout.runtime_state_root))
    except Exception:
        pass

    entries: list[StorageEntry] = []
    seen: set[tuple[object, ...]] = set()
    for root_kind, root in roots:
        if not root.exists():
            continue
        for path in _walk_files(root):
            identity = _scan_identity(path)
            if identity in seen:
                continue
            seen.add(identity)
            entries.append(_classify_path(layout, root, path, root_kind=root_kind))
    return tuple(entries)


def _walk_files(root: Path):
    for current, dirnames, filenames in os.walk(root, followlinks=False):
        current_path = Path(current)
        safe_dirs: list[str] = []
        for dirname in dirnames:
            candidate = current_path / dirname
            try:
                if candidate.is_symlink():
                    yield candidate
                    continue
            except OSError:
                yield candidate
                continue
            safe_dirs.append(dirname)
        dirnames[:] = safe_dirs
        for filename in filenames:
            yield current_path / filename


def _classify_path(layout: PathLayout, root: Path, path: Path, *, root_kind: str) -> StorageEntry:
    size = _safe_size(path)
    relative_path = _relative_display(layout, root, path, root_kind=root_kind)
    if _is_allowed_provider_secret_symlink(path, layout):
        return _classify_relative(layout, path, relative_path, size=size, root_kind=root_kind)
    symlink_reason = _unsafe_symlink_reason(path, layout)
    if symlink_reason is not None and not _is_marked_projected_symlink(path):
        return StorageEntry(
            path=path,
            relative_path=relative_path,
            storage_class=StorageClass.UNKNOWN,
            size_bytes=size,
            reason=symlink_reason,
            root_kind=root_kind,
        )
    return _classify_relative(layout, path, relative_path, size=size, root_kind=root_kind)


def _classify_relative(layout: PathLayout, path: Path, relative_path: str, *, size: int, root_kind: str) -> StorageEntry:
    parts = Path(relative_path).parts
    if not parts:
        return _entry(path, relative_path, StorageClass.UNKNOWN, size, root_kind=root_kind)

    if parts[0] == 'ccb.config':
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, root_kind=root_kind)
    if parts[0] == 'ccb_memory.md':
        return _entry(path, relative_path, StorageClass.USER_CONTENT, size, reason='project_shared_memory', root_kind=root_kind)
    if parts[0] in {'runtime-root.json', 'runtime-root-ref.json'}:
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, root_kind=root_kind)
    if parts[0].startswith('.') and parts[0].endswith('-session'):
        return _provider_session_file_entry(path, relative_path, parts[0], size, root_kind=root_kind)
    if len(parts) >= 2 and parts[0] == 'ccbd':
        return _classify_ccbd(path, relative_path, parts, size=size, root_kind=root_kind)
    if len(parts) >= 3 and parts[0] == 'agents':
        return _classify_agent(path, relative_path, parts, size=size, root_kind=root_kind)
    if len(parts) >= 3 and parts[0] == 'provider-profiles':
        return classify_provider_home(path, relative_path, parts[2], parts[1], parts[3:], size=size, root_kind=root_kind)
    if parts == ('state', 'memory.seed.json'):
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, reason='project_memory_seed', root_kind=root_kind)
    if len(parts) == 3 and parts[0] == 'runtime' and parts[1] == 'memory' and parts[2].endswith('.md'):
        agent = parts[2][:-3]
        return _entry(
            path,
            relative_path,
            StorageClass.RUNTIME_EPHEMERAL,
            size,
            agent=agent,
            reason='project_memory_bundle',
            root_kind=root_kind,
        )
    if len(parts) >= 2 and parts[0] == 'shared-cache':
        return _entry(
            path,
            relative_path,
            StorageClass.REBUILDABLE_CACHE,
            size,
            provider=parts[1],
            reclaimable=False,
            reason='shared_cache',
            root_kind=root_kind,
        )
    if parts[0] == 'workspaces':
        return _entry(path, relative_path, StorageClass.WORKSPACE, size, reason='agent_workspace', root_kind=root_kind)
    if parts[0] == 'history':
        return _entry(path, relative_path, StorageClass.USER_CONTENT, size, reason='project_history', root_kind=root_kind)
    return _entry(path, relative_path, StorageClass.UNKNOWN, size, root_kind=root_kind)


def _classify_ccbd(path: Path, relative_path: str, parts: tuple[str, ...], *, size: int, root_kind: str) -> StorageEntry:
    name = parts[-1]
    top = parts[1]
    if len(parts) == 2 and name in _CCBD_AUTHORITY_FILES:
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, root_kind=root_kind)
    if top in _CCBD_RUNTIME_DIRS or name.endswith('.pid') or name.endswith('.sock') or name.endswith('.lock'):
        return _entry(path, relative_path, StorageClass.RUNTIME_EPHEMERAL, size, root_kind=root_kind)
    if top in {'mailboxes', 'messages', 'attempts', 'replies', 'executions', 'snapshots'}:
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, root_kind=root_kind)
    if name.endswith('.jsonl') or name.endswith('.log'):
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, reason='ccbd_event_log', root_kind=root_kind)
    if name.endswith('.json'):
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, root_kind=root_kind)
    return _entry(path, relative_path, StorageClass.UNKNOWN, size, root_kind=root_kind)


def _classify_agent(path: Path, relative_path: str, parts: tuple[str, ...], *, size: int, root_kind: str) -> StorageEntry:
    agent = parts[1]
    name = parts[-1]
    if len(parts) == 3 and name in _AGENT_AUTHORITY_FILES:
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, agent=agent, root_kind=root_kind)
    if len(parts) == 3 and name == 'memory.md':
        return _entry(path, relative_path, StorageClass.USER_CONTENT, size, agent=agent, reason='agent_private_memory', root_kind=root_kind)
    if len(parts) == 3 and name.endswith('.jsonl'):
        return _entry(path, relative_path, StorageClass.AUTHORITY, size, agent=agent, reason='agent_event_log', root_kind=root_kind)
    if len(parts) >= 4 and parts[2] == 'provider-runtime':
        provider = parts[3]
        return _entry(path, relative_path, StorageClass.RUNTIME_EPHEMERAL, size, provider=provider, agent=agent, root_kind=root_kind)
    if len(parts) >= 5 and parts[2] == 'provider-state':
        provider = parts[3]
        remainder = parts[5:] if len(parts) >= 5 and parts[4] == 'home' else parts[4:]
        return classify_provider_home(path, relative_path, provider, agent, remainder, size=size, root_kind=root_kind)
    if len(parts) >= 3 and parts[2] == 'logs':
        return _entry(path, relative_path, StorageClass.RUNTIME_EPHEMERAL, size, agent=agent, reason='agent_log', root_kind=root_kind)
    return _entry(path, relative_path, StorageClass.UNKNOWN, size, agent=agent, root_kind=root_kind)


def _provider_session_file_entry(path: Path, relative_path: str, filename: str, size: int, *, root_kind: str) -> StorageEntry:
    parts = filename.strip('.').split('-')
    provider = parts[0] if parts else None
    agent = parts[1] if len(parts) >= 3 else None
    return _entry(path, relative_path, StorageClass.SESSION, size, provider=provider, agent=agent, root_kind=root_kind)


def _entry(
    path: Path,
    relative_path: str,
    storage_class: StorageClass,
    size: int,
    *,
    provider: str | None = None,
    agent: str | None = None,
    active: bool | None = None,
    is_active_version: bool | None = None,
    reachable_from_current_symlink: bool | None = None,
    reclaimable: bool | None = None,
    reason: str | None = None,
    root_kind: str,
) -> StorageEntry:
    return StorageEntry(
        path=path,
        relative_path=relative_path,
        storage_class=storage_class,
        size_bytes=size,
        provider=provider,
        agent=agent,
        active=active,
        is_active_version=is_active_version,
        reachable_from_current_symlink=reachable_from_current_symlink,
        reclaimable=reclaimable,
        reason=reason,
        root_kind=root_kind,
    )


def _summary_payload(layout: PathLayout, entries: list[StorageEntry]) -> dict[str, object]:
    by_class: dict[str, dict[str, int]] = defaultdict(lambda: {'bytes': 0, 'count': 0})
    by_provider: dict[str, dict[str, int]] = defaultdict(lambda: {'bytes': 0, 'count': 0})
    by_agent: dict[str, dict[str, int]] = defaultdict(lambda: {'bytes': 0, 'count': 0})
    total_bytes = 0
    for entry in entries:
        total_bytes += entry.size_bytes
        _accumulate(by_class[entry.storage_class.value], entry.size_bytes)
        if entry.provider:
            _accumulate(by_provider[entry.provider], entry.size_bytes)
        if entry.agent:
            _accumulate(by_agent[entry.agent], entry.size_bytes)
    shared_cache_reason = _shared_cache_disabled_reason(layout)
    shared_cache_enabled = shared_cache_reason is None
    return {
        'schema_version': SCHEMA_VERSION,
        'generated_at': datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
        'project': str(layout.project_root),
        'project_id': layout.project_id,
        'runtime_root_kind': layout.runtime_state_placement.root_kind,
        'runtime_state_root': str(layout.runtime_state_root),
        'shared_cache_root': _shared_cache_root(layout, disabled_reason=shared_cache_reason or ''),
        'shared_cache_root_usable': shared_cache_enabled,
        'shared_cache_status': 'enabled' if shared_cache_enabled else 'disabled',
        'shared_cache_reason': 'enabled' if shared_cache_enabled else shared_cache_reason,
        'total_bytes': total_bytes,
        'total_count': len(entries),
        'by_class': dict(sorted(by_class.items())),
        'by_provider': dict(sorted(by_provider.items())),
        'by_agent': dict(sorted(by_agent.items())),
        'entries': [entry.to_record() for entry in sorted(entries, key=lambda item: item.size_bytes, reverse=True)],
    }


def _shared_cache_root(layout: PathLayout, *, disabled_reason: str) -> str | None:
    if disabled_reason == 'wsl_drvfs_requires_runtime_relocation':
        return None
    return str(layout.shared_cache_dir)


def _shared_cache_disabled_reason(layout: PathLayout) -> str | None:
    placement = layout.runtime_state_placement
    if placement.filesystem_hint == 'wsl_drvfs' and placement.root_kind != 'relocated':
        return 'wsl_drvfs_requires_runtime_relocation'
    return None


def _accumulate(bucket: dict[str, int], size: int) -> None:
    bucket['bytes'] += size
    bucket['count'] += 1


def _relative_display(layout: PathLayout, root: Path, path: Path, *, root_kind: str) -> str:
    try:
        relative = path.relative_to(root)
    except Exception:
        return str(path)
    if root_kind == 'runtime' and root != layout.ccb_dir:
        return str(relative)
    return str(relative)


def _safe_size(path: Path) -> int:
    try:
        stat = path.lstat()
    except OSError:
        return 0
    return int(stat.st_size)


def _scan_identity(path: Path) -> tuple[object, ...]:
    try:
        stat = path.lstat()
        return ('inode', stat.st_dev, stat.st_ino)
    except OSError:
        return ('path', str(path.absolute()))


def _unsafe_symlink_reason(path: Path, layout: PathLayout) -> str | None:
    try:
        if not path.is_symlink():
            return None
    except OSError:
        return 'symlink_unreadable'
    try:
        target = path.resolve(strict=True)
    except RuntimeError:
        return 'symlink_loop'
    except OSError:
        return 'symlink_target_missing'
    allowed_roots = (layout.ccb_dir, layout.runtime_state_root)
    if any(_is_within(target, root) for root in allowed_roots):
        return None
    return 'symlink_out_of_bounds'


def _is_allowed_provider_secret_symlink(path: Path, layout: PathLayout) -> bool:
    try:
        relative = path.relative_to(layout.ccb_dir)
    except Exception:
        return False
    parts = relative.parts
    return (
        len(parts) >= 7
        and parts[0] == 'agents'
        and parts[2] == 'provider-state'
        and parts[3] == 'claude'
        and parts[4] == 'home'
        and parts[5:7] == ('Library', 'Keychains')
    )


def _is_marked_projected_symlink(path: Path) -> bool:
    try:
        if not path.is_symlink():
            return False
        payload = json.loads(Path(f'{path}.ccb-projection.json').read_text(encoding='utf-8'))
    except Exception:
        return False
    if not isinstance(payload, dict) or payload.get('record_type') != 'ccb_projected_asset':
        return False
    if str(payload.get('label') or '') not in {
        'claude-binary-versions',
        'claude-inherited-skills',
        'claude-inherited-commands',
        'codex-inherited-skills',
        'codex-inherited-commands',
        'codex-plugin-bundle',
        'droid-inherited-skills',
    }:
        return False
    source = str(payload.get('source') or '').strip()
    if not source:
        return False
    try:
        return Path(source).expanduser().resolve() == path.resolve()
    except Exception:
        return False


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root.resolve(strict=False))
        return True
    except Exception:
        return False


__all__ = ['summarize_storage']
