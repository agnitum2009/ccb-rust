from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import shutil

from provider_core.projected_assets import tree_content_fingerprint, write_projected_marker

_PROJECTION_LABEL = 'claude-binary-versions'
_IGNORED_VERSION_ENTRIES = {'.DS_Store'}


@dataclass(frozen=True)
class _RouteContext:
    home: Path
    versions_dir: Path
    shared_versions_dir: Path
    source_active_version: Path | None
    source_active_version_name: str


def route_claude_binary_cache(
    home_root: Path,
    shared_cache_root: Path,
    *,
    source_home: Path | None = None,
) -> dict[str, object]:
    home = Path(home_root).expanduser().resolve(strict=False)
    shared_versions_dir = Path(shared_cache_root).expanduser().resolve(strict=False) / 'versions'
    versions_dir = home / '.local' / 'share' / 'claude' / 'versions'
    source_active_version = _source_active_version(source_home, managed_home=home)
    context = _RouteContext(
        home=home,
        versions_dir=versions_dir,
        shared_versions_dir=shared_versions_dir,
        source_active_version=source_active_version,
        source_active_version_name=source_active_version.name if source_active_version is not None else '',
    )

    failure = _ensure_shared_versions_dir(context)
    if failure is not None:
        return failure

    if versions_dir.is_symlink():
        return _route_symlinked_versions_dir(context)

    if versions_dir.exists() and not versions_dir.is_dir():
        return _result(
            status='skipped',
            reason='versions_path_not_directory',
            versions_dir=versions_dir,
            shared_versions_dir=shared_versions_dir,
        )

    if not versions_dir.exists():
        return _route_missing_versions_dir(context)

    return _route_local_versions_dir(context)


def _ensure_shared_versions_dir(context: _RouteContext) -> dict[str, object] | None:
    try:
        context.shared_versions_dir.mkdir(parents=True, exist_ok=True)
    except Exception as exc:
        return _result(
            status='skipped',
            reason='shared_cache_unavailable',
            versions_dir=context.versions_dir,
            shared_versions_dir=context.shared_versions_dir,
            error_detail=str(exc),
        )
    return None


def _route_symlinked_versions_dir(context: _RouteContext) -> dict[str, object]:
    if _same_path(context.versions_dir, context.shared_versions_dir):
        return _route_already_shared_versions_dir(context)
    try:
        source_versions_dir = context.versions_dir.resolve(strict=True)
    except Exception as exc:
        return _result(
            status='skipped',
            reason='versions_symlink_target_unavailable',
            versions_dir=context.versions_dir,
            shared_versions_dir=context.shared_versions_dir,
            error_detail=str(exc),
        )
    if not source_versions_dir.is_dir():
        return _result(
            status='skipped',
            reason='versions_dir_symlink_not_shared',
            versions_dir=context.versions_dir,
            shared_versions_dir=context.shared_versions_dir,
        )
    scan = _scan_versions_dir(source_versions_dir)
    if scan['unknown_entries']:
        return _result(
            status='skipped',
            reason='versions_dir_symlink_not_shared',
            versions_dir=context.versions_dir,
            shared_versions_dir=context.shared_versions_dir,
            version_names=scan['version_names'],
            warnings=scan['unknown_entries'],
        )
    return _migrate_scanned_versions(
        context,
        scan,
        reason='migrated_symlink' if scan['version_paths'] else 'linked_empty',
    )


def _route_already_shared_versions_dir(context: _RouteContext) -> dict[str, object]:
    failure = _copy_source_active_version_to_shared(
        context.source_active_version,
        shared_versions_dir=context.shared_versions_dir,
        versions_dir=context.versions_dir,
    )
    if failure is not None:
        return failure
    active_version_name = _ensure_claude_link(
        context.home,
        context.shared_versions_dir,
        preferred_version_name=context.source_active_version_name,
    )
    write_projected_marker(
        context.versions_dir,
        label=_PROJECTION_LABEL,
        mode='symlink',
        source=context.shared_versions_dir,
    )
    return _result(
        status='ok',
        reason='already_shared',
        versions_dir=context.versions_dir,
        shared_versions_dir=context.shared_versions_dir,
        version_names=_version_names(context.shared_versions_dir),
        active_version_name=active_version_name,
    )


def _route_missing_versions_dir(context: _RouteContext) -> dict[str, object]:
    failure = _copy_source_active_version_to_shared(
        context.source_active_version,
        shared_versions_dir=context.shared_versions_dir,
        versions_dir=context.versions_dir,
    )
    if failure is not None:
        return failure
    linked = _link_versions_dir(
        context.versions_dir,
        context.shared_versions_dir,
        reason='linked_empty',
        version_names=_version_names(context.shared_versions_dir),
    )
    return _with_active_version(linked, context)


def _route_local_versions_dir(context: _RouteContext) -> dict[str, object]:
    scan = _scan_versions_dir(context.versions_dir)
    if scan['unknown_entries']:
        return _result(
            status='skipped',
            reason='unknown_versions_entries',
            versions_dir=context.versions_dir,
            shared_versions_dir=context.shared_versions_dir,
            version_names=scan['version_names'],
            warnings=scan['unknown_entries'],
        )
    return _migrate_scanned_versions(
        context,
        scan,
        reason='migrated' if scan['version_paths'] else 'linked_empty',
    )


def _migrate_scanned_versions(
    context: _RouteContext,
    scan: dict[str, object],
    *,
    reason: str,
) -> dict[str, object]:
    failure = _copy_versions_to_shared(
        version_paths=scan['version_paths'],
        shared_versions_dir=context.shared_versions_dir,
        versions_dir=context.versions_dir,
        version_names=scan['version_names'],
    )
    if failure is not None:
        return failure
    failure = _copy_source_active_version_to_shared(
        context.source_active_version,
        shared_versions_dir=context.shared_versions_dir,
        versions_dir=context.versions_dir,
    )
    if failure is not None:
        return failure

    linked = _link_versions_dir(
        context.versions_dir,
        context.shared_versions_dir,
        reason=reason,
        version_names=_version_names(context.shared_versions_dir),
    )
    linked = _with_active_version(linked, context)
    if scan['ignored_entries'] and linked.get('status') == 'ok':
        linked['warnings'] = tuple(scan['ignored_entries'])
    return linked


def _with_active_version(result: dict[str, object], context: _RouteContext) -> dict[str, object]:
    if result.get('status') == 'ok':
        result['active_version_name'] = _ensure_claude_link(
            context.home,
            context.shared_versions_dir,
            preferred_version_name=context.source_active_version_name,
        ) or ''
    return result


def _copy_versions_to_shared(
    *,
    version_paths: tuple[Path, ...],
    shared_versions_dir: Path,
    versions_dir: Path,
    version_names: tuple[str, ...],
) -> dict[str, object] | None:
    for version_path in version_paths:
        destination = shared_versions_dir / version_path.name
        if destination.exists() and _version_fingerprint(destination) != _version_fingerprint(version_path):
            return _result(
                status='skipped',
                reason='shared_version_content_conflict',
                versions_dir=versions_dir,
                shared_versions_dir=shared_versions_dir,
                version_names=version_names,
                warnings=(version_path.name,),
            )
        if not destination.exists():
            try:
                _copy_version_atomic(version_path, destination)
            except Exception as exc:
                return _result(
                    status='skipped',
                    reason='shared_version_copy_failed',
                    versions_dir=versions_dir,
                    shared_versions_dir=shared_versions_dir,
                    version_names=version_names,
                    error_detail=str(exc),
                )
        write_projected_marker(destination, label='claude-binary-version', mode='copy', source=version_path)
    return None


def _copy_source_active_version_to_shared(
    source_active_version: Path | None,
    *,
    shared_versions_dir: Path,
    versions_dir: Path,
) -> dict[str, object] | None:
    if source_active_version is None:
        return None
    return _copy_versions_to_shared(
        version_paths=(source_active_version,),
        shared_versions_dir=shared_versions_dir,
        versions_dir=versions_dir,
        version_names=(source_active_version.name,),
    )


def _scan_versions_dir(versions_dir: Path) -> dict[str, object]:
    version_paths: list[Path] = []
    unknown_entries: list[str] = []
    ignored_entries: list[str] = []
    try:
        entries = sorted(versions_dir.iterdir(), key=lambda item: item.name)
    except Exception:
        return {'version_paths': (), 'version_names': (), 'unknown_entries': ('unreadable_versions_dir',), 'ignored_entries': ()}
    for entry in entries:
        if entry.name in _IGNORED_VERSION_ENTRIES or entry.name.endswith('.ccb-projection.json'):
            ignored_entries.append(entry.name)
            continue
        if not _looks_like_claude_version_name(entry.name):
            unknown_entries.append(entry.name)
            continue
        if entry.is_dir() and not entry.is_symlink() and (entry / 'claude').is_file():
            version_paths.append(entry)
            continue
        if entry.is_file() and not entry.is_symlink():
            version_paths.append(entry)
            continue
        unknown_entries.append(entry.name)
    return {
        'version_paths': tuple(version_paths),
        'version_names': tuple(path.name for path in version_paths),
        'unknown_entries': tuple(unknown_entries),
        'ignored_entries': tuple(ignored_entries),
    }


def _link_versions_dir(
    versions_dir: Path,
    shared_versions_dir: Path,
    *,
    reason: str,
    version_names: tuple[str, ...] = (),
) -> dict[str, object]:
    try:
        versions_dir.parent.mkdir(parents=True, exist_ok=True)
        if versions_dir.exists() or versions_dir.is_symlink():
            _remove_path(versions_dir)
        versions_dir.symlink_to(shared_versions_dir, target_is_directory=True)
        write_projected_marker(
            versions_dir,
            label=_PROJECTION_LABEL,
            mode='symlink',
            source=shared_versions_dir,
        )
    except Exception as exc:
        return _result(
            status='skipped',
            reason='versions_link_failed',
            versions_dir=versions_dir,
            shared_versions_dir=shared_versions_dir,
            version_names=version_names,
            error_detail=str(exc),
        )
    return _result(
        status='ok',
        reason=reason,
        versions_dir=versions_dir,
        shared_versions_dir=shared_versions_dir,
        version_names=version_names or _version_names(shared_versions_dir),
    )


def _ensure_claude_link(home: Path, shared_versions_dir: Path, *, preferred_version_name: str = '') -> str:
    target_version = _preferred_or_newest_version_path(shared_versions_dir, preferred_version_name=preferred_version_name)
    if target_version is None:
        return ''
    executable = _version_executable_path(target_version)
    if executable is None:
        return ''
    link = home / '.local' / 'bin' / 'claude'
    try:
        if link.is_symlink() and _same_path(link, executable):
            return target_version.name
        if link.exists() and not link.is_symlink():
            return ''
        link.parent.mkdir(parents=True, exist_ok=True)
        link.unlink(missing_ok=True)
        link.symlink_to(executable)
    except Exception:
        return ''
    return target_version.name


def _preferred_or_newest_version_path(versions_dir: Path, *, preferred_version_name: str) -> Path | None:
    preferred = str(preferred_version_name or '').strip()
    if preferred:
        candidate = versions_dir / preferred
        if _looks_like_claude_version_name(candidate.name) and _version_executable_path(candidate) is not None:
            return candidate
    return _newest_version_path(versions_dir)


def _newest_version_path(versions_dir: Path) -> Path | None:
    try:
        candidates = [
            child
            for child in versions_dir.iterdir()
            if _looks_like_claude_version_name(child.name)
            and not child.name.endswith('.ccb-projection.json')
            and not child.is_symlink()
            and _version_executable_path(child) is not None
        ]
    except Exception:
        return None
    if not candidates:
        return None
    return max(candidates, key=lambda path: (_version_key(path.name), path.stat().st_mtime, path.name))


def _version_executable_path(version_path: Path) -> Path | None:
    if version_path.is_file():
        return version_path
    executable = version_path / 'claude'
    if executable.is_file():
        return executable
    return None


def _version_names(versions_dir: Path) -> tuple[str, ...]:
    try:
        return tuple(
            sorted(
                child.name
                for child in versions_dir.iterdir()
                if not child.name.endswith('.ccb-projection.json')
                if _looks_like_claude_version_name(child.name)
                and not child.is_symlink()
                and (child.is_file() or child.is_dir())
            )
        )
    except Exception:
        return ()


def _source_active_version(source_home: Path | None, *, managed_home: Path) -> Path | None:
    if source_home is None:
        return None
    try:
        home = Path(source_home).expanduser().resolve(strict=False)
    except Exception:
        return None
    if _same_path(home, managed_home):
        return None
    link = home / '.local' / 'bin' / 'claude'
    if not link.is_symlink():
        return None
    try:
        target = link.resolve(strict=True)
        versions_dir = (home / '.local' / 'share' / 'claude' / 'versions').resolve(strict=True)
        relative = target.relative_to(versions_dir)
    except Exception:
        return None
    if not relative.parts:
        return None
    version_name = relative.parts[0]
    if not _looks_like_claude_version_name(version_name):
        return None
    candidate = versions_dir / version_name
    return candidate if _version_executable_path(candidate) is not None else None


def _same_path(left: Path, right: Path) -> bool:
    try:
        return left.resolve() == right.resolve()
    except Exception:
        return False


def _copy_version_atomic(source: Path, destination: Path) -> None:
    if source.is_file():
        _copyfile_atomic(source, destination)
        return
    _copytree_atomic(source, destination)


def _copyfile_atomic(source: Path, destination: Path) -> None:
    tmp = destination.with_name(f'.{destination.name}.tmp')
    _remove_path(tmp)
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, tmp, follow_symlinks=False)
        tmp.rename(destination)
    except Exception:
        _remove_path(tmp)
        raise


def _copytree_atomic(source: Path, destination: Path) -> None:
    tmp = destination.with_name(f'.{destination.name}.tmp')
    _remove_path(tmp)
    try:
        shutil.copytree(source, tmp, symlinks=True)
        tmp.rename(destination)
    except Exception:
        _remove_path(tmp)
        raise


def _version_fingerprint(path: Path) -> str:
    if path.is_file():
        return _file_content_fingerprint(path)
    return tree_content_fingerprint(path)


def _file_content_fingerprint(path: Path) -> str:
    from hashlib import sha256

    digest = sha256()
    try:
        with Path(path).open('rb') as handle:
            for chunk in iter(lambda: handle.read(64 * 1024), b''):
                digest.update(chunk)
    except Exception:
        return ''
    return digest.hexdigest()


def _looks_like_claude_version_name(value: str) -> bool:
    if not value or not value[0].isdigit():
        return False
    return all(item.isalnum() or item in {'.', '_', '-'} for item in value)


def _version_key(value: str) -> tuple[tuple[int, object], ...]:
    parts: list[tuple[int, object]] = []
    for item in value.replace('-', '.').split('.'):
        if item.isdigit():
            parts.append((1, int(item)))
        else:
            parts.append((0, item))
    return tuple(parts)


def _remove_path(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink(missing_ok=True)
        return
    if path.is_dir():
        shutil.rmtree(path)


def _result(
    *,
    status: str,
    reason: str,
    versions_dir: Path,
    shared_versions_dir: Path,
    version_names: tuple[str, ...] = (),
    warnings: tuple[str, ...] = (),
    error_detail: str = '',
    active_version_name: str = '',
) -> dict[str, object]:
    return {
        'status': status,
        'reason': reason,
        'versions_dir': str(versions_dir),
        'shared_versions_dir': str(shared_versions_dir),
        'version_names': tuple(version_names),
        'warnings': tuple(warnings),
        'error_detail': error_detail,
        'active_version_name': active_version_name,
    }


__all__ = ['route_claude_binary_cache']
