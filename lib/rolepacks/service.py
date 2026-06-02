from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
from typing import Any

from agents.config_loader_runtime.role_lookup import role_store_root
from agents.config_loader import load_project_config
from agents.config_loader_runtime.paths import project_config_path
from agents.models import normalize_agent_name
from storage.atomic import atomic_write_text

from .manifest import RoleManifest as RolePack
from .manifest import RoleManifestError, load_role_manifest, normalize_role_id


class RolePackError(RoleManifestError):
    pass


def builtin_role_root(script_root: Path | None = None) -> Path:
    root = Path(script_root) if script_root is not None else Path(__file__).resolve().parents[2]
    return root / 'roles'


def list_builtin_roles(*, script_root: Path | None = None) -> tuple[RolePack, ...]:
    root = builtin_role_root(script_root)
    if not root.is_dir():
        return ()
    roles: list[RolePack] = []
    for child in sorted(root.iterdir(), key=lambda item: item.name):
        if not child.is_dir() or not (child / 'role.toml').is_file():
            continue
        roles.append(load_role(child))
    return tuple(roles)


def load_installed_role(role_id: str) -> RolePack | None:
    role_id = normalize_role_id(role_id)
    current = role_store_root() / role_id / 'current'
    if current.exists():
        try:
            return load_role(current.resolve())
        except Exception:
            return None
    direct = role_store_root() / role_id
    if (direct / 'role.toml').is_file():
        try:
            return load_role(direct)
        except Exception:
            return None
    return None


def load_project_agent_role(project_root: Path, agent_name: str) -> RolePack | None:
    try:
        config = load_project_config(project_root).config
        normalized = normalize_agent_name(agent_name)
        spec = config.agents.get(normalized)
        role_id = str(getattr(spec, 'role', '') or '').strip()
        if not role_id:
            return None
        return load_installed_role(role_id) or _load_builtin_role_by_id(role_id)
    except Exception:
        return None


def project_role_memory_sources(project_root: Path, agent_name: str) -> tuple[object, ...]:
    from project_memory.types import ProjectMemorySource

    role = load_project_agent_role(project_root, agent_name)
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
    role = load_project_agent_role(project_root, agent_name)
    if role is None:
        return ()
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


def load_role(path: Path) -> RolePack:
    try:
        return load_role_manifest(path)
    except RoleManifestError as exc:
        raise RolePackError(str(exc)) from exc


def install_role(role_id: str, *, script_root: Path | None = None, with_tools: bool = True) -> dict[str, object]:
    role_id = normalize_role_id(role_id)
    source = _find_builtin_role(role_id, script_root=script_root)
    if source is None:
        raise RolePackError(f'unknown builtin role: {role_id}')
    role = load_role(source)
    payload = _install_role_assets(role, source=source)
    if with_tools:
        installed = load_role(Path(str(payload['path'])))
        tool_results = run_role_tool_hooks(installed, action='install', fail_required=True)
        payload['tools_status'] = _tool_results_status(tool_results)
        payload['tools'] = tool_results
    else:
        payload['tools_status'] = 'skipped'
        payload['tools_reason'] = 'tool dependency install skipped by caller'
    return payload


def update_role(role_id: str, *, script_root: Path | None = None, with_tools: bool = True) -> dict[str, object]:
    role_id = normalize_role_id(role_id)
    source = _find_builtin_role(role_id, script_root=script_root)
    if source is None:
        raise RolePackError(f'unknown builtin role: {role_id}')
    role = load_role(source)
    payload = _install_role_assets(role, source=source)
    payload['role_status'] = 'updated'
    if with_tools:
        installed = load_role(Path(str(payload['path'])))
        tool_results = run_role_tool_hooks(installed, action='update', fail_required=True)
        payload['tools_status'] = _tool_results_status(tool_results)
        payload['tools'] = tool_results
    else:
        payload['tools_status'] = 'skipped'
        payload['tools_reason'] = 'tool dependency update skipped by caller'
    return payload


def _install_role_assets(role: RolePack, *, source: Path) -> dict[str, object]:
    target = role_store_root() / role.id / 'versions' / role.version
    if target.exists():
        shutil.rmtree(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, target)
    digest = tree_digest(target)
    current = role_store_root() / role.id / 'current'
    if current.exists() or current.is_symlink():
        if current.is_symlink() or current.is_file():
            current.unlink()
        else:
            shutil.rmtree(current)
    try:
        current.symlink_to(target, target_is_directory=True)
    except OSError:
        shutil.copytree(target, current)
    metadata = {
        'schema': 'rolepack-install/v1',
        'id': role.id,
        'version': role.version,
        'source': 'builtin',
        'digest': f'sha256:{digest}',
    }
    atomic_write_text(role_store_root() / role.id / 'install.json', json.dumps(metadata, sort_keys=True, indent=2) + '\n')
    return {
        'role_status': 'installed',
        'role_id': role.id,
        'version': role.version,
        'digest': f'sha256:{digest}',
        'path': str(target),
    }


def role_status(role_id: str, *, script_root: Path | None = None, include_tools: bool = False) -> dict[str, object]:
    role_id = normalize_role_id(role_id)
    installed = load_installed_role(role_id)
    builtin = _find_builtin_role(role_id, script_root=script_root)
    payload: dict[str, object] = {
        'role_id': role_id,
        'builtin': bool(builtin),
        'installed': installed is not None,
        'store_root': str(role_store_root()),
    }
    if installed is not None:
        payload.update({
            'name': installed.name,
            'version': installed.version,
            'providers': ','.join(installed.providers),
            'path': str(installed.root),
        })
    if include_tools:
        role = installed
        if role is None and builtin is not None:
            role = load_role(builtin)
        if role is not None:
            tool_results = run_role_tool_hooks(role, action='doctor')
            payload['tools_status'] = _tool_results_status(tool_results)
            payload['tools'] = tool_results
        else:
            payload['tools_status'] = 'missing'
    return payload


def run_role_tool_hooks(
    role: RolePack,
    *,
    action: str,
    fail_required: bool = False,
) -> tuple[dict[str, object], ...]:
    tools = dict(role.manifest.get('tools') or {})
    results: list[dict[str, object]] = []
    for tool_id in sorted(tools):
        spec = tools.get(tool_id)
        if not isinstance(spec, dict):
            continue
        command = str(spec.get(action) or '').strip()
        required = bool(spec.get('required', False))
        if not command:
            results.append(
                {
                    'tool_id': tool_id,
                    'action': action,
                    'status': 'skipped',
                    'required': required,
                    'reason': f'no {action} hook declared',
                }
            )
            continue
        result = _run_role_tool_command(role, tool_id=tool_id, action=action, command=command, required=required)
        results.append(result)
        if fail_required and result.get('status') == 'failed' and required:
            raise RolePackError(
                f'role tool {tool_id} {action} failed with exit code {result.get("returncode")}: '
                f'{result.get("stderr") or result.get("stdout") or "no output"}'
            )
    return tuple(results)


def _run_role_tool_command(
    role: RolePack,
    *,
    tool_id: str,
    action: str,
    command: str,
    required: bool,
) -> dict[str, object]:
    try:
        argv = shlex.split(command)
    except ValueError as exc:
        return {
            'tool_id': tool_id,
            'action': action,
            'status': 'failed',
            'required': required,
            'returncode': 2,
            'stderr': f'invalid command: {exc}',
        }
    if not argv:
        return {
            'tool_id': tool_id,
            'action': action,
            'status': 'skipped',
            'required': required,
            'reason': 'empty hook command',
        }
    if argv[0] in {'python', 'python3'}:
        argv[0] = sys.executable
    env = dict(os.environ)
    env.update(
        {
            'CCB_ROLE_ID': role.id,
            'CCB_ROLE_ROOT': str(role.root),
            'CCB_ROLE_TOOL_ID': tool_id,
            'CCB_ROLE_TOOL_ACTION': action,
        }
    )
    try:
        completed = subprocess.run(
            argv,
            cwd=role.root,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=float(os.environ.get('CCB_ROLE_TOOL_TIMEOUT_S') or '900'),
            check=False,
        )
    except Exception as exc:
        return {
            'tool_id': tool_id,
            'action': action,
            'status': 'failed',
            'required': required,
            'returncode': 1,
            'stderr': f'{type(exc).__name__}: {exc}',
        }
    status = 'ok' if completed.returncode == 0 else 'failed'
    return {
        'tool_id': tool_id,
        'action': action,
        'status': status,
        'required': required,
        'returncode': completed.returncode,
        'stdout': completed.stdout.strip(),
        'stderr': completed.stderr.strip(),
    }


def _tool_results_status(results: tuple[dict[str, object], ...]) -> str:
    if not results:
        return 'none'
    if any(result.get('status') == 'failed' for result in results):
        return 'failed'
    if all(result.get('status') == 'skipped' for result in results):
        return 'skipped'
    return 'ok'


def add_role_to_project_config(
    *,
    project_root: Path,
    role_id: str,
    agent_name: str | None,
    provider: str | None,
    window_name: str | None = None,
    script_root: Path | None = None,
) -> dict[str, object]:
    role_id = normalize_role_id(role_id)
    role = load_installed_role(role_id)
    if role is None:
        source = _find_builtin_role(role_id, script_root=script_root)
        if source is None:
            raise RolePackError(f'role is not installed and no builtin role exists: {role_id}')
        role = load_role(source)
    selected_agent = normalize_agent_name(agent_name or role.default_agent_name)
    selected_provider = str(provider or (role.providers[0] if role.providers else 'codex')).strip().lower()
    if role.providers and selected_provider not in role.providers:
        raise RolePackError(
            f'role {role_id} does not support provider {selected_provider}; supported: {", ".join(role.providers)}'
        )
    config_path = project_config_path(project_root)
    if not config_path.is_file():
        raise RolePackError(f'project config not found: {config_path}')
    current_config = load_project_config(project_root).config
    if not tuple(current_config.windows or ()):
        raise RolePackError('roles add requires [windows] topology in .ccb/ccb.config')
    target_window = _select_window_name(current_config, window_name=window_name)
    before = config_path.read_text(encoding='utf-8')
    after = before
    use_shorthand = selected_agent == normalize_agent_name(role.default_agent_name)
    if selected_agent not in current_config.agents:
        after = _append_agent_to_window_layout(
            after,
            window_name=target_window,
            agent_name=role_id if use_shorthand else selected_agent,
            provider=selected_provider,
        )
    if not use_shorthand:
        after = _upsert_agent_role_overlay(
            after,
            agent_name=selected_agent,
            provider=selected_provider,
            role_id=role_id,
        )
    loaded = _load_project_config_from_text(after)
    if selected_agent not in loaded.config.agents:
        raise RolePackError(
            f'role overlay for {selected_agent} did not produce a configured agent; '
            'check the [windows] topology'
        )
    if after != before:
        atomic_write_text(config_path, after)
    _write_project_role_lock(project_root, role)
    return {
        'role_status': 'added' if after != before else 'unchanged',
        'role_id': role_id,
        'agent': selected_agent,
        'provider': selected_provider,
        'window': target_window,
        'config': str(config_path),
        'config_binding': 'shorthand' if use_shorthand else 'explicit',
        'note': 'run ccb reload to mount new role agent' if after != before else '',
    }


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


def _find_builtin_role(role_id: str, *, script_root: Path | None) -> Path | None:
    root = builtin_role_root(script_root)
    candidate = root / role_id
    if (candidate / 'role.toml').is_file():
        return candidate
    for role in list_builtin_roles(script_root=script_root):
        if role.id == role_id:
            return role.root
    return None


def _load_builtin_role_by_id(role_id: str) -> RolePack | None:
    source = _find_builtin_role(normalize_role_id(role_id), script_root=None)
    if source is None:
        return None
    try:
        return load_role(source)
    except Exception:
        return None


def _upsert_agent_role_overlay(text: str, *, agent_name: str, provider: str, role_id: str) -> str:
    lines = text.rstrip().splitlines()
    header = f'[agents.{agent_name}]'
    start = None
    end = len(lines)
    for index, line in enumerate(lines):
        if line.strip() == header:
            start = index
            continue
        if start is not None and index > start and line.strip().startswith('['):
            end = index
            break
    if start is None:
        block = [
            '',
            header,
            f'role = "{role_id}"',
            f'provider = "{provider}"',
        ]
        return '\n'.join(lines + block).rstrip() + '\n'

    block = lines[start:end]
    block = _upsert_key(block, 'role', role_id)
    block = _upsert_key(block, 'provider', provider)
    return '\n'.join(lines[:start] + block + lines[end:]).rstrip() + '\n'


def _select_window_name(config, *, window_name: str | None) -> str:
    requested = str(window_name or '').strip()
    windows = tuple(config.windows or ())
    if requested:
        for window in windows:
            if window.name == requested:
                return requested
        raise RolePackError(f'unknown window for role agent: {requested}')
    entry = str(config.entry_window or '').strip()
    if entry:
        for window in windows:
            if window.name == entry:
                return entry
    return windows[0].name


def _append_agent_to_window_layout(text: str, *, window_name: str, agent_name: str, provider: str) -> str:
    lines = text.rstrip().splitlines()
    windows_start = None
    windows_end = len(lines)
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped == '[windows]':
            windows_start = index
            continue
        if windows_start is not None and index > windows_start and stripped.startswith('['):
            windows_end = index
            break
    if windows_start is None:
        raise RolePackError('roles add requires a [windows] table in .ccb/ccb.config')
    key_prefixes = (f'{window_name} =', f'{window_name}=')
    rendered_leaf = f'{agent_name}:{provider}'
    for index in range(windows_start + 1, windows_end):
        stripped = lines[index].strip()
        if not any(stripped.startswith(prefix) for prefix in key_prefixes):
            continue
        quote = '"' if '"' in lines[index] else "'"
        first = lines[index].find(quote)
        last = lines[index].rfind(quote)
        if first < 0 or last <= first:
            raise RolePackError(f'cannot update windows.{window_name}; expected single-line quoted layout')
        current = lines[index][first + 1:last].strip()
        updated = f'{current}, {rendered_leaf}' if current else rendered_leaf
        lines[index] = lines[index][:first + 1] + updated + lines[index][last:]
        return '\n'.join(lines).rstrip() + '\n'
    insert_at = windows_end
    lines.insert(insert_at, f'{window_name} = "{rendered_leaf}"')
    return '\n'.join(lines).rstrip() + '\n'


def _load_project_config_from_text(text: str):
    with tempfile.TemporaryDirectory(prefix='ccb-role-config-') as tmp:
        root = Path(tmp)
        ccb_dir = root / '.ccb'
        ccb_dir.mkdir()
        (ccb_dir / 'ccb.config').write_text(text, encoding='utf-8')
        return load_project_config(root)


def _upsert_key(block: list[str], key: str, value: str) -> list[str]:
    prefix = f'{key} '
    rendered = f'{key} = "{value}"'
    for index, line in enumerate(block[1:], start=1):
        stripped = line.strip()
        if stripped.startswith(prefix) or stripped.startswith(f'{key}='):
            block[index] = rendered
            return block
    return block + [rendered]


def _write_project_role_lock(project_root: Path, role: RolePack) -> None:
    path = Path(project_root).expanduser().resolve() / '.ccb' / 'role-lock.json'
    digest = tree_digest(role.root)
    payload = {
        'schema': 'rolepack-lock/v1',
        'roles': {
            role.id: {
                'version': role.version,
                'digest': f'sha256:{digest}',
                'source': 'builtin' if _is_under(role.root, builtin_role_root(None)) else 'installed',
            }
        },
    }
    existing: dict[str, Any] = {}
    try:
        loaded = json.loads(path.read_text(encoding='utf-8'))
        if isinstance(loaded, dict):
            existing = loaded
    except Exception:
        existing = {}
    roles = dict(existing.get('roles') or {})
    roles.update(payload['roles'])
    merged = {'schema': 'rolepack-lock/v1', 'roles': roles}
    atomic_write_text(path, json.dumps(merged, ensure_ascii=True, sort_keys=True, indent=2) + '\n')


def _is_under(path: Path, root: Path) -> bool:
    try:
        Path(path).resolve().relative_to(Path(root).resolve())
        return True
    except Exception:
        return False


__all__ = [
    'RolePack',
    'RolePackError',
    'add_role_to_project_config',
    'builtin_role_root',
    'install_role',
    'list_builtin_roles',
    'load_project_agent_role',
    'load_installed_role',
    'load_role',
    'project_role_memory_sources',
    'project_role_skill_sources',
    'run_role_tool_hooks',
    'role_status',
    'role_store_root',
    'update_role',
]
