from __future__ import annotations

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
from typing import Any

from agents.config_loader import load_project_config
from agents.config_loader_runtime.paths import project_config_path
from agents.models import normalize_agent_name
from storage.atomic import atomic_write_text

from . import agent_roles_manager
from .manifest import RoleManifest as RolePack
from .manifest import RoleManifestError, load_role_manifest, normalize_role_id
from .runtime_lookup import (
    load_installed_role,
    load_project_agent_role,
    project_role_memory_sources,
    project_role_skill_sources,
    role_store_roots,
    tree_digest,
)
from .sources import (
    default_agent_roles_source,
    discover_source_roles,
    find_source_role,
    find_system_source_role,
    installed_role_metadata,
    migrate_legacy_installed_roles,
)


ARCHITEC_ROLE_ID = 'agentroles.archi'
ARCHITEC_TOOL_ID = 'architec'
ARCHITEC_NPM_PACKAGE = '@seemseam/architec'


class RolePackError(RoleManifestError):
    pass


def builtin_role_root(script_root: Path | None = None) -> Path:
    source = _default_catalog_source_path()
    if source is not None:
        return source
    root = Path(script_root) if script_root is not None else Path(__file__).resolve().parents[2]
    return root / 'roles'


def list_builtin_roles(*, script_root: Path | None = None) -> tuple[RolePack, ...]:
    return tuple(load_role(item.path) for item in discover_source_roles())


def load_role(path: Path) -> RolePack:
    try:
        return load_role_manifest(path)
    except RoleManifestError as exc:
        raise RolePackError(str(exc)) from exc


def install_role(
    role_id: str | None = None,
    *,
    script_root: Path | None = None,
    source_path: Path | None = None,
    with_tools: bool = True,
) -> dict[str, object]:
    migrate_legacy_installed_roles(role_id)
    return _install_role_via_agent_roles_manager(role_id, source_path=source_path, with_tools=with_tools)


def update_role(
    role_id: str | None = None,
    *,
    script_root: Path | None = None,
    source_path: Path | None = None,
    with_tools: bool = True,
) -> dict[str, object]:
    migrate_legacy_installed_roles(role_id)
    return _update_role_via_agent_roles_manager(role_id, source_path=source_path, with_tools=with_tools)


def sync_roles_from_path(source_path: Path, *, with_tools: bool = False) -> dict[str, object]:
    source_root = Path(source_path).expanduser().resolve()
    migrate_legacy_installed_roles()
    try:
        payload = agent_roles_manager.sync(source_root)
    except agent_roles_manager.AgentRolesManagerError as exc:
        raise RolePackError(str(exc)) from exc
    normalized = _normalize_agent_roles_sync_payload(payload)
    if with_tools:
        rows = []
        for row in normalized['roles']:
            copied = dict(row)
            if str(copied.get('status') or '') == 'synced':
                installed = load_role(Path(str(copied.get('path') or '')))
                tool_results = run_role_tool_hooks(installed, action='update', fail_required=True)
                copied['tools_status'] = _tool_results_status(tool_results)
                copied['tools'] = tool_results
            rows.append(copied)
        normalized['roles'] = tuple(rows)
    return normalized


def _install_role_via_agent_roles_manager(
    role_id: str | None,
    *,
    source_path: Path | None,
    with_tools: bool,
) -> dict[str, object]:
    try:
        payload = agent_roles_manager.install(role_id, source_path=source_path)
    except agent_roles_manager.AgentRolesManagerError as exc:
        raise RolePackError(str(exc)) from exc
    payload = _normalize_agent_roles_payload(payload, default_role_status='installed')
    if with_tools:
        installed = load_role(Path(str(payload['path'])))
        tool_results = run_role_tool_hooks(installed, action='install', fail_required=True)
        payload['tools_status'] = _tool_results_status(tool_results)
        payload['tools'] = tool_results
    else:
        payload['tools_status'] = 'skipped'
        payload['tools_reason'] = 'tool dependency install skipped by caller'
    return payload


def _update_role_via_agent_roles_manager(
    role_id: str | None,
    *,
    source_path: Path | None = None,
    with_tools: bool,
) -> dict[str, object]:
    try:
        if source_path is not None:
            payload = agent_roles_manager.install(role_id, source_path=source_path)
        else:
            payload = agent_roles_manager.update(role_id)
    except agent_roles_manager.AgentRolesManagerError as exc:
        raise RolePackError(str(exc)) from exc
    payload = _normalize_agent_roles_payload(payload, default_role_status='updated')
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


def _normalize_agent_roles_payload(payload: dict[str, object], *, default_role_status: str) -> dict[str, object]:
    normalized = dict(payload)
    normalized.pop('schema', None)
    normalized.pop('status', None)
    normalized['role_status'] = str(normalized.get('role_status') or default_role_status)
    path = str(normalized.get('path') or normalized.get('installed_path') or '').strip()
    if not path:
        raise RolePackError('agent-roles did not return an installed path')
    normalized['path'] = path
    return normalized


def _normalize_agent_roles_sync_payload(payload: dict[str, object]) -> dict[str, object]:
    rows = payload.get('roles') or ()
    if isinstance(rows, list):
        if not all(isinstance(item, dict) for item in rows):
            raise RolePackError('agent-roles returned invalid sync roles payload')
        rows = tuple(item for item in rows if isinstance(item, dict))
    elif isinstance(rows, tuple):
        if not all(isinstance(item, dict) for item in rows):
            raise RolePackError('agent-roles returned invalid sync roles payload')
    else:
        raise RolePackError('agent-roles returned invalid sync roles payload')
    return {
        'sync_status': 'ok' if str(payload.get('status') or '') == 'ok' else str(payload.get('status') or 'unknown'),
        'path': str(payload.get('path') or ''),
        'roles': rows,
    }


def role_status(role_id: str, *, script_root: Path | None = None, include_tools: bool = False) -> dict[str, object]:
    role_id = normalize_role_id(role_id)
    migrate_legacy_installed_roles(role_id)
    source_role = find_source_role(role_id)
    installed = load_installed_role(role_id)
    payload: dict[str, object] = {
        'role_id': role_id,
        'available': source_role is not None,
        'source': source_role.source if source_role is not None else '',
        'source_path': str(source_role.path) if source_role is not None else '',
        'installed': installed is not None,
        'store_root': str(role_store_roots()[0]),
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
        if role is None and source_role is not None:
            role = load_role(source_role.path)
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
        if _is_architec_tool_hook(role, tool_id):
            result = _run_architec_tool_hook(action=action, required=required)
            results.append(result)
            if fail_required and result.get('status') == 'failed' and required:
                raise RolePackError(
                    f'role tool {tool_id} {action} failed with exit code {result.get("returncode")}: '
                    f'{result.get("stderr") or result.get("stdout") or "no output"}'
                )
            continue
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


def _is_architec_tool_hook(role: RolePack, tool_id: str) -> bool:
    return role.id == ARCHITEC_ROLE_ID and str(tool_id or '').strip().lower() == ARCHITEC_TOOL_ID


def _run_architec_tool_hook(*, action: str, required: bool) -> dict[str, object]:
    if action in {'install', 'update'}:
        return _run_architec_npm_install(action=action, required=required)
    if action == 'doctor':
        return _run_architec_doctor(action=action, required=required)
    return {
        'tool_id': ARCHITEC_TOOL_ID,
        'action': action,
        'status': 'skipped',
        'required': required,
        'reason': f'no built-in {action} hook declared',
    }


def _run_architec_npm_install(*, action: str, required: bool) -> dict[str, object]:
    npm_bin = _architec_npm_bin()
    package = _architec_npm_package()
    display_command = f'npm install -g {package}'
    command = ['npm', 'install', '-g', package]
    if not npm_bin:
        return {
            'tool_id': ARCHITEC_TOOL_ID,
            'action': action,
            'status': 'failed',
            'required': required,
            'returncode': 127,
            'stdout': _architec_status_text(
                architec_status='missing',
                action=action,
                package=package,
                install_command=display_command,
                reason='npm is not available on PATH',
            ),
            'stderr': 'npm is not available; install Node.js/npm, then run `npm install -g @seemseam/architec`',
        }
    command[0] = npm_bin
    try:
        completed = subprocess.run(
            command,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=float(os.environ.get('CCB_ARCHITEC_NPM_TIMEOUT_S') or os.environ.get('CCB_ROLE_TOOL_TIMEOUT_S') or '900'),
            check=False,
        )
    except Exception as exc:
        return {
            'tool_id': ARCHITEC_TOOL_ID,
            'action': action,
            'status': 'failed',
            'required': required,
            'returncode': 1,
            'stdout': _architec_status_text(
                architec_status='failed',
                action=action,
                package=package,
                npm_bin=npm_bin,
                install_command=display_command,
                reason=f'{type(exc).__name__}: {exc}',
            ),
            'stderr': f'{type(exc).__name__}: {exc}',
        }
    status = 'ok' if completed.returncode == 0 else 'failed'
    return {
        'tool_id': ARCHITEC_TOOL_ID,
        'action': action,
        'status': status,
        'required': required,
        'returncode': completed.returncode,
        'stdout': _architec_status_text(
            architec_status=status,
            action=action,
            package=package,
            npm_bin=npm_bin,
            install_command=display_command,
            stdout=_one_line(completed.stdout),
        ),
        'stderr': completed.stderr.strip(),
    }


def _run_architec_doctor(*, action: str, required: bool) -> dict[str, object]:
    package = _architec_npm_package()
    archi = _which_first('archi', 'architec')
    archi_probe = _probe_archi_help(archi)
    archi_help = str(archi_probe.get('status') or 'missing')
    hippo_capability = 'available' if bool(archi_probe.get('hippo_capability')) else 'unknown'
    llmgateway_capability = 'available' if bool(archi_probe.get('llmgateway_capability')) else 'unknown'
    if not archi or archi_help == 'failed':
        architec_status = 'missing' if not archi else 'failed'
        returncode = 1
        status = 'failed'
    elif hippo_capability != 'available' or llmgateway_capability != 'available':
        architec_status = 'degraded'
        returncode = 0
        status = 'ok'
    else:
        architec_status = 'ok'
        returncode = 0
        status = 'ok'
    stdout = _architec_status_text(
        architec_status=architec_status,
        action=action,
        package=package,
        install_command=f'npm install -g {package}',
        archi_binary=archi or '',
        bundled_hippo=hippo_capability,
        bundled_llmgateway=llmgateway_capability,
        archi_help=archi_help,
        hippo_check='archi --help includes --refresh-from-hippos',
        llmgateway_check='archi --help includes --check or --full',
        reason=_architec_doctor_reason(
            architec_status,
            hippo_capability=hippo_capability,
            llmgateway_capability=llmgateway_capability,
        ),
    )
    return {
        'tool_id': ARCHITEC_TOOL_ID,
        'action': action,
        'status': status,
        'required': required,
        'returncode': returncode,
        'stdout': stdout,
        'stderr': '',
    }


def _architec_npm_bin() -> str | None:
    configured = str(os.environ.get('CCB_ARCHITEC_NPM_BIN') or os.environ.get('NPM_BIN') or '').strip()
    if configured:
        return configured
    return shutil.which('npm')


def _architec_npm_package() -> str:
    return str(os.environ.get('CCB_ARCHITEC_NPM_PACKAGE') or ARCHITEC_NPM_PACKAGE).strip() or ARCHITEC_NPM_PACKAGE


def _which_first(*names: str) -> str | None:
    for name in names:
        path = shutil.which(name)
        if path:
            return path
    return None


def _probe_binary_help(path: str | None) -> str:
    if not path:
        return 'missing'
    try:
        completed = subprocess.run(
            [path, '--help'],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=20,
            check=False,
        )
    except Exception:
        return 'failed'
    return 'ok' if completed.returncode == 0 else 'failed'


def _probe_archi_help(path: str | None) -> dict[str, object]:
    if not path:
        return {'status': 'missing'}
    try:
        completed = subprocess.run(
            [path, '--help'],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=20,
            check=False,
        )
    except Exception:
        return {'status': 'failed'}
    if completed.returncode != 0:
        return {'status': 'failed'}
    help_text = '\n'.join((completed.stdout or '', completed.stderr or '')).lower()
    return {
        'status': 'ok',
        'hippo_capability': '--refresh-from-hippos' in help_text or 'hippo' in help_text,
        'llmgateway_capability': '--check' in help_text or '--full' in help_text or 'llm' in help_text,
    }


def _architec_doctor_reason(
    status: str,
    *,
    hippo_capability: str,
    llmgateway_capability: str,
) -> str:
    if status == 'ok':
        return '@seemseam/architec CLI bundle is available'
    missing = []
    if hippo_capability != 'available':
        missing.append('hippo capability')
    if llmgateway_capability != 'available':
        missing.append('llmgateway capability')
    if missing:
        return 'archi CLI did not advertise bundled ' + ', '.join(missing)
    return 'install or update @seemseam/architec'


def _architec_status_text(**fields: object) -> str:
    lines = []
    for key, value in fields.items():
        if value is None:
            continue
        text = str(value)
        lines.append(f'{key}: {text}')
    return '\n'.join(lines)


def _one_line(text: str) -> str:
    return ' | '.join(line.strip() for line in str(text or '').splitlines() if line.strip())


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
            'PYTHONDONTWRITEBYTECODE': '1',
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
    auto_installed = False
    if role is None:
        source_role = find_system_source_role(role_id)
        if source_role is not None:
            install_payload = _install_role_via_agent_roles_manager(
                role_id,
                source_path=source_role.path,
                with_tools=False,
            )
            role = load_role(Path(str(install_payload['path'])))
            auto_installed = True
        if role is None:
            raise RolePackError(f'role is not installed; run `ccb roles install {role_id}`')
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
        'install': 'snapshotted_from_system_source' if auto_installed else '',
        'note': 'run ccb reload to mount new role agent' if after != before else '',
    }


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
    metadata = installed_role_metadata(role.id)
    digest = str(metadata.get('digest') or '').strip() or f'sha256:{tree_digest(role.root)}'
    payload = {
        'schema': 'rolepack-lock/v1',
        'roles': {
            role.id: {
                'version': role.version,
                'digest': digest,
                'source': 'installed',
                'default_agent_name': role.default_agent_name,
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


def _default_catalog_source_path() -> Path | None:
    default = default_agent_roles_source()
    if default is not None:
        return default
    source_role = next(iter(discover_source_roles()), None)
    if source_role is None:
        return None
    root = Path(source_role.path)
    for parent in (root, *root.parents):
        if parent.name in {'roles', 'reference_roles'}:
            return parent.parent
    return root.parent


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
    'sync_roles_from_path',
    'update_role',
]
