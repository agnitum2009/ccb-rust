from __future__ import annotations

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time


DEFAULT_INSTALL_SPEC = 'architec @ git+https://github.com/SeemSeam/architec.git'


def install_or_update(action: str) -> int:
    paths = _paths()
    paths['root'].mkdir(parents=True, exist_ok=True)
    paths['bin_dir'].mkdir(parents=True, exist_ok=True)
    paths['bin_link'].parent.mkdir(parents=True, exist_ok=True)
    try:
        _ensure_venv(paths)
    except Exception as exc:
        _print_status(
            {
                'architec_status': 'failed',
                'action': action,
                'reason': 'venv creation failed',
                'venv': str(paths['venv']),
                'stderr': f'{type(exc).__name__}: {exc}',
            }
        )
        return 1
    install_spec = os.environ.get('CCB_ARCHITEC_INSTALL_SPEC') or DEFAULT_INSTALL_SPEC
    pip_result = _run(
        [
            str(paths['venv_python']),
            '-m',
            'pip',
            'install',
            '--upgrade',
            install_spec,
        ],
        timeout_s=_timeout_s(),
    )
    if pip_result.returncode != 0:
        _print_status(
            {
                'architec_status': 'failed',
                'action': action,
                'reason': 'pip install failed',
                'venv': str(paths['venv']),
                'stderr': _one_line(pip_result.stderr),
            }
        )
        return 1
    _write_wrapper(paths)
    _write_bin_link(paths)
    version = _probe_version(paths)
    manifest = {
        'schema': 'ccb-tool-architec/v1',
        'status': 'ok',
        'action': action,
        'install_spec': install_spec,
        'venv': str(paths['venv']),
        'wrapper': str(paths['wrapper']),
        'bin_link': str(paths['bin_link']),
        'archi_binary': str(paths['archi_binary']),
        'version': version,
        'updated_at': int(time.time()),
    }
    paths['manifest'].write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n', encoding='utf-8')
    _print_status(
        {
            'architec_status': 'ok',
            'action': action,
            'venv': str(paths['venv']),
            'wrapper': str(paths['wrapper']),
            'bin_link': str(paths['bin_link']),
            'version': version,
        }
    )
    return 0


def doctor() -> int:
    paths = _paths()
    wrapper_ok = _is_executable(paths['wrapper'])
    managed_binary_ok = _is_executable(paths['archi_binary'])
    resolved = shutil.which('ccb-archi') or shutil.which('archi')
    selected = str(paths['wrapper']) if wrapper_ok else resolved
    help_status = 'skipped'
    if selected:
        result = _run([selected, '--help'], timeout_s=20)
        help_status = 'ok' if result.returncode == 0 else 'failed'
    llmgateway = _llmgateway_config()
    status = 'ok' if selected else 'missing'
    _print_status(
        {
            'architec_status': status,
            'managed_wrapper': str(paths['wrapper']),
            'managed_wrapper_exists': wrapper_ok,
            'managed_archi_binary_exists': managed_binary_ok,
            'path_binary': resolved or '',
            'selected_binary': selected or '',
            'help_status': help_status,
            'llmgateway_config': 'present' if llmgateway else 'missing',
            'llmgateway_config_path': str(llmgateway or ''),
            'venv': str(paths['venv']),
            'manifest': str(paths['manifest']),
        }
    )
    return 0 if selected else 1


def _paths() -> dict[str, Path]:
    data_home = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local' / 'share').expanduser()
    root = data_home / 'ccb' / 'tools' / 'architec'
    venv = root / 'venv'
    venv_bin = venv / ('Scripts' if os.name == 'nt' else 'bin')
    wrapper_name = 'ccb-archi.cmd' if os.name == 'nt' else 'ccb-archi'
    archi_name = 'archi.exe' if os.name == 'nt' else 'archi'
    bin_home = Path(os.environ.get('CODEX_BIN_DIR') or Path.home() / '.local' / 'bin').expanduser()
    return {
        'root': root,
        'bin_dir': root / 'bin',
        'venv': venv,
        'venv_python': venv_bin / ('python.exe' if os.name == 'nt' else 'python'),
        'archi_binary': venv_bin / archi_name,
        'wrapper': root / 'bin' / wrapper_name,
        'bin_link': bin_home / wrapper_name,
        'manifest': root / 'manifest.json',
    }


def _ensure_venv(paths: dict[str, Path]) -> None:
    if _is_executable(paths['venv_python']):
        return
    result = _run([sys.executable, '-m', 'venv', str(paths['venv'])], timeout_s=120)
    if result.returncode != 0:
        raise RuntimeError(f'failed to create Architec venv: {_one_line(result.stderr)}')


def _write_wrapper(paths: dict[str, Path]) -> None:
    wrapper = paths['wrapper']
    if os.name == 'nt':
        wrapper.write_text(f'@echo off\r\n"{paths["archi_binary"]}" %*\r\n', encoding='utf-8')
    else:
        wrapper.write_text(
            '#!/usr/bin/env sh\n'
            f'exec {shlex.quote(str(paths["archi_binary"]))} "$@"\n',
            encoding='utf-8',
        )
        wrapper.chmod(0o755)


def _write_bin_link(paths: dict[str, Path]) -> None:
    source = paths['wrapper']
    target = paths['bin_link']
    if source.resolve() == target.resolve():
        return
    if target.exists() or target.is_symlink():
        target.unlink()
    try:
        target.symlink_to(source)
    except OSError:
        shutil.copy2(source, target)
        if os.name != 'nt':
            target.chmod(0o755)


def _probe_version(paths: dict[str, Path]) -> str:
    if not _is_executable(paths['wrapper']):
        return ''
    for args in ([str(paths['wrapper']), '--version'], [str(paths['wrapper']), 'version']):
        result = _run(args, timeout_s=20)
        if result.returncode == 0 and result.stdout.strip():
            return _one_line(result.stdout)
    return ''


def _llmgateway_config() -> Path | None:
    for name in ('LLMGATEWAY_CONFIG', 'LLM_GATEWAY_CONFIG'):
        raw = os.environ.get(name)
        if raw and Path(raw).expanduser().is_file():
            return Path(raw).expanduser()
    candidates = [
        Path.home() / '.llmgateway' / 'config.toml',
        Path.home() / '.llmgateway' / 'config.yaml',
        Path.home() / '.llmgateway' / 'config.yml',
        Path.home() / '.config' / 'llmgateway' / 'config.toml',
        Path.home() / '.config' / 'llmgateway' / 'config.yaml',
        Path.home() / '.config' / 'llmgateway' / 'config.yml',
    ]
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return None


def _run(args: list[str], *, timeout_s: float) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            args,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_s,
            check=False,
        )
    except Exception as exc:
        return subprocess.CompletedProcess(args, 1, '', f'{type(exc).__name__}: {exc}')


def _timeout_s() -> float:
    try:
        return float(os.environ.get('CCB_ARCHITEC_INSTALL_TIMEOUT_S') or '900')
    except ValueError:
        return 900.0


def _is_executable(path: Path) -> bool:
    return path.is_file() and (os.name == 'nt' or os.access(path, os.X_OK))


def _one_line(text: str) -> str:
    return ' | '.join(line.strip() for line in str(text or '').splitlines() if line.strip())


def _print_status(payload: dict[str, object]) -> None:
    for key, value in payload.items():
        print(f'{key}: {value}')
