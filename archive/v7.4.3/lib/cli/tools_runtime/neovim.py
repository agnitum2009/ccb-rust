from __future__ import annotations

import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
from typing import TextIO
import urllib.request


NEOVIM_RELEASE_API_URL = 'https://api.github.com/repos/neovim/neovim/releases/tags/stable'
LAZY_NVIM_TARBALL_URLS = (
    'https://codeload.github.com/folke/lazy.nvim/tar.gz/refs/heads/main',
    'https://github.com/folke/lazy.nvim/archive/refs/heads/main.tar.gz',
)
NEOVIM_DOWNLOAD_TIMEOUT_S = 30.0
LAZYVIM_SYNC_TIMEOUT_S = 180.0
LAZYVIM_BOOTSTRAP_TIMEOUT_S = 30.0


def cmd_tools(argv: list[str], *, script_root: Path | None = None, stdout: TextIO | None = None, stderr: TextIO | None = None) -> int:
    del script_root
    stdout = stdout or sys.stdout
    stderr = stderr or sys.stderr
    if not argv or argv[0] in {'-h', '--help', 'help'}:
        _print_help(stdout)
        return 0
    if len(argv) < 2:
        _print_help(stdout)
        return 2
    action, tool = argv[0], argv[1]
    if tool != 'neovim':
        print(f'ERROR: unsupported tool: {tool}', file=stderr)
        return 2
    if action == 'doctor':
        status = neovim_status()
        _print_status(status, stdout)
        return 0 if status['status'] in {'ok', 'missing'} else 1
    if action in {'install', 'update'}:
        required = _install_required()
        result = provision_neovim(required=required)
        _print_status(result, stdout)
        if result['status'] == 'ok':
            return 0
        return 1
    print(f'ERROR: unsupported tools action: {action}', file=stderr)
    return 2


def provision_neovim(*, required: bool = False) -> dict[str, object]:
    paths = _paths()
    paths['bin_dir'].mkdir(parents=True, exist_ok=True)
    paths['config_nvim'].mkdir(parents=True, exist_ok=True)
    paths['data'].mkdir(parents=True, exist_ok=True)
    paths['state'].mkdir(parents=True, exist_ok=True)
    paths['cache'].mkdir(parents=True, exist_ok=True)
    nvim = _resolve_nvim()
    if nvim is None:
        download = _ensure_managed_nvim(paths)
        if download.get('status') != 'ok':
            status = {
                'status': 'failed' if required else 'missing',
                'reason': download.get('reason') or 'managed Neovim download failed',
                **_status_paths(paths),
            }
            if download.get('error'):
                status['error'] = download.get('error')
            _write_manifest(paths, status)
            return status
        nvim = Path(str(download['binary']))
    lazyvim_enabled = _lazyvim_profile_enabled()
    if lazyvim_enabled:
        _write_lazyvim_profile(paths)
    _write_wrapper(paths, nvim)
    _write_bin_link(paths)
    lazyvim_sync = (
        _sync_lazyvim_profile(paths)
        if lazyvim_enabled
        else {'status': 'skipped', 'reason': 'disabled by CCB_LAZYVIM_PROFILE=0'}
    )
    manifest = _read_manifest(paths)
    status_value = 'ok'
    reason = None
    if lazyvim_enabled and lazyvim_sync.get('status') != 'ok':
        status_value = 'failed' if required else 'degraded'
        reason = lazyvim_sync.get('reason') or 'LazyVim sync failed'
    status = {
        'status': status_value,
        'binary': str(nvim),
        'wrapper': str(paths['wrapper']),
        'bin_link': str(paths['bin_link']),
        'lazyvim_profile': str(paths['profile']),
        'managed_profile': lazyvim_enabled,
        'lazyvim_profile_enabled': lazyvim_enabled,
        'lazyvim_sync_status': lazyvim_sync.get('status'),
        'lazyvim_health_status': lazyvim_sync.get('health_status'),
        'lazyvim_repaired': bool(lazyvim_sync.get('repaired')),
        'uses_system_nvim': not _is_managed_binary(paths, nvim),
        'managed_neovim_target': manifest.get('managed_neovim_target'),
        'managed_neovim_version': manifest.get('managed_neovim_version'),
        'managed_neovim_asset': manifest.get('managed_neovim_asset'),
        **_status_paths(paths),
    }
    if reason:
        status['reason'] = reason
    if lazyvim_sync.get('error'):
        status['lazyvim_sync_error'] = lazyvim_sync.get('error')
    if lazyvim_sync.get('health_error'):
        status['lazyvim_health_error'] = lazyvim_sync.get('health_error')
    _write_manifest(paths, status)
    return status


def neovim_status() -> dict[str, object]:
    paths = _paths()
    wrapper_exists = paths['wrapper'].is_file() and os.access(paths['wrapper'], os.X_OK)
    nvim = _resolve_nvim()
    manifest = _read_manifest(paths)
    if wrapper_exists:
        lazyvim_enabled = bool(manifest.get('lazyvim_profile_enabled', paths['marker'].exists()))
        health = _check_lazyvim_health(paths) if lazyvim_enabled else {'status': 'skipped'}
        status_value = 'degraded' if lazyvim_enabled and health.get('status') != 'ok' else 'ok'
        return {
            'status': status_value,
            'reason': health.get('reason') if lazyvim_enabled and health.get('status') != 'ok' else None,
            'binary': str(nvim) if nvim is not None else str(manifest.get('binary') or ''),
            'wrapper': str(paths['wrapper']),
            'bin_link': str(paths['bin_link']),
            'lazyvim_profile': str(paths['profile']),
            'managed_profile': lazyvim_enabled,
            'lazyvim_profile_enabled': lazyvim_enabled,
            'lazyvim_sync_status': manifest.get('lazyvim_sync_status'),
            'lazyvim_health_status': health.get('status'),
            'lazyvim_sync_error': manifest.get('lazyvim_sync_error'),
            'lazyvim_health_error': health.get('error') if lazyvim_enabled and health.get('status') != 'ok' else None,
            'uses_system_nvim': manifest.get('uses_system_nvim'),
            'managed_neovim_target': manifest.get('managed_neovim_target'),
            'managed_neovim_version': manifest.get('managed_neovim_version'),
            'managed_neovim_asset': manifest.get('managed_neovim_asset'),
            **_status_paths(paths),
        }
    return {
        'status': 'missing',
        'reason': 'ccb-nvim wrapper is not installed',
        'binary': str(nvim) if nvim is not None else None,
        **_status_paths(paths),
    }


def _paths() -> dict[str, Path]:
    data_home = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local' / 'share')
    state_home = Path(os.environ.get('XDG_STATE_HOME') or Path.home() / '.local' / 'state')
    cache_home = Path(os.environ.get('XDG_CACHE_HOME') or Path.home() / '.cache')
    root = data_home / 'ccb' / 'tools' / 'neovim'
    profile = root / 'lazyvim' / 'profile'
    return {
        'root': root,
        'bin_dir': root / 'bin',
        'wrapper': root / 'bin' / 'ccb-nvim',
        'bin_link': Path(os.environ.get('CODEX_BIN_DIR') or Path.home() / '.local' / 'bin') / 'ccb-nvim',
        'profile': profile,
        'config_nvim': profile / 'config' / 'nvim',
        'data': profile / 'share',
        'state': state_home / 'ccb' / 'tools' / 'neovim' / 'xdg-state',
        'cache': cache_home / 'ccb' / 'tools' / 'neovim' / 'xdg-cache',
        'marker': profile / '.ccb-managed-lazyvim',
        'manifest': root / 'manifest.json',
        'downloads': root / 'downloads',
        'versions': root / 'versions',
        'managed_nvim': root / 'bin' / 'nvim',
    }


def _resolve_nvim() -> Path | None:
    managed = _paths()['managed_nvim']
    if managed.is_file() and os.access(managed, os.X_OK):
        return managed
    resolved = shutil.which('nvim')
    return Path(resolved) if resolved else None


def _ensure_managed_nvim(paths: dict[str, Path]) -> dict[str, object]:
    if paths['managed_nvim'].is_file() and os.access(paths['managed_nvim'], os.X_OK):
        manifest = _read_manifest(paths)
        return {
            'status': 'ok',
            'binary': str(paths['managed_nvim']),
            'managed_neovim_target': manifest.get('managed_neovim_target'),
            'managed_neovim_version': manifest.get('managed_neovim_version'),
            'managed_neovim_asset': manifest.get('managed_neovim_asset'),
        }
    asset = _platform_release_asset()
    if asset is None:
        return {
            'status': 'unsupported',
            'reason': f'no managed Neovim asset for {platform.system().lower()}-{platform.machine().lower()}',
        }
    try:
        release = _fetch_release_metadata()
        selected = _select_release_asset(release, asset)
        return _download_and_activate_nvim(paths, selected, release)
    except Exception as exc:
        return {
            'status': 'failed',
            'reason': 'managed Neovim download failed',
            'error': f'{type(exc).__name__}: {exc}',
        }


def _platform_release_asset() -> str | None:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == 'linux':
        if machine in {'x86_64', 'amd64'}:
            return 'nvim-linux-x86_64.tar.gz'
        if machine in {'aarch64', 'arm64'}:
            return 'nvim-linux-arm64.tar.gz'
    if system == 'darwin':
        if machine in {'arm64', 'aarch64'}:
            return 'nvim-macos-arm64.tar.gz'
        if machine in {'x86_64', 'amd64'}:
            return 'nvim-macos-x86_64.tar.gz'
    return None


def _fetch_release_metadata() -> dict[str, object]:
    with urllib.request.urlopen(NEOVIM_RELEASE_API_URL, timeout=NEOVIM_DOWNLOAD_TIMEOUT_S) as response:
        return json.loads(response.read().decode('utf-8'))


def _select_release_asset(release: dict[str, object], asset_name: str) -> dict[str, object]:
    for asset in tuple(release.get('assets') or ()):
        if not isinstance(asset, dict):
            continue
        if str(asset.get('name') or '') == asset_name:
            url = str(asset.get('browser_download_url') or '').strip()
            digest = _normalize_sha256_digest(asset.get('digest'))
            if not url:
                raise RuntimeError(f'Neovim asset {asset_name} has no download URL')
            if not digest:
                raise RuntimeError(f'Neovim asset {asset_name} has no sha256 digest')
            return {
                'name': asset_name,
                'url': url,
                'sha256': digest,
            }
    raise RuntimeError(f'Neovim release asset not found: {asset_name}')


def _normalize_sha256_digest(value: object) -> str:
    text = str(value or '').strip()
    if text.startswith('sha256:'):
        text = text.split(':', 1)[1]
    if len(text) == 64 and all(ch in '0123456789abcdefABCDEF' for ch in text):
        return text.lower()
    return ''


def _download_and_activate_nvim(
    paths: dict[str, Path],
    asset: dict[str, object],
    release: dict[str, object],
) -> dict[str, object]:
    paths['downloads'].mkdir(parents=True, exist_ok=True)
    paths['versions'].mkdir(parents=True, exist_ok=True)
    archive_path = paths['downloads'] / str(asset['name'])
    _download_file(str(asset['url']), archive_path)
    actual = _sha256_file(archive_path)
    expected = str(asset['sha256'])
    if actual != expected:
        raise RuntimeError(f'Neovim asset sha256 mismatch: expected {expected}, got {actual}')
    version_name = str(release.get('tag_name') or 'stable').strip() or 'stable'
    extract_root = paths['versions'] / _safe_version_dir(version_name, str(asset['name']))
    tmp_root = Path(tempfile.mkdtemp(prefix='ccb-nvim-', dir=str(paths['versions'])))
    try:
        with tarfile.open(archive_path, 'r:gz') as archive:
            _safe_extract_tar(archive, tmp_root)
        binary = _find_extracted_nvim(tmp_root)
        if binary is None:
            raise RuntimeError('downloaded Neovim archive did not contain bin/nvim')
        if extract_root.exists():
            shutil.rmtree(extract_root)
        tmp_root.rename(extract_root)
        binary = _find_extracted_nvim(extract_root)
        assert binary is not None
    except Exception:
        shutil.rmtree(tmp_root, ignore_errors=True)
        raise
    _activate_managed_nvim(paths, binary)
    payload = {
        'status': 'ok',
        'binary': str(paths['managed_nvim']),
        'managed_neovim_target': str(binary),
        'managed_neovim_version': version_name,
        'managed_neovim_asset': str(asset['name']),
        'managed_neovim_sha256': actual,
    }
    _write_manifest(paths, {**_read_manifest(paths), **payload})
    return payload


def _download_file(url: str, destination: Path, *, timeout_s: float = NEOVIM_DOWNLOAD_TIMEOUT_S) -> None:
    with urllib.request.urlopen(url, timeout=timeout_s) as response:
        data = response.read()
    destination.write_bytes(data)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def _safe_extract_tar(archive: tarfile.TarFile, destination: Path) -> None:
    root = destination.resolve()
    for member in archive.getmembers():
        member_path = (destination / member.name).resolve()
        if not _is_within(root, member_path):
            raise RuntimeError(f'unsafe Neovim archive member: {member.name}')
        if member.issym() or member.islnk():
            link_target = Path(member.linkname)
            if link_target.is_absolute():
                raise RuntimeError(f'unsafe Neovim archive link: {member.name}')
            resolved = ((destination / member.name).parent / member.linkname).resolve()
            if not _is_within(root, resolved):
                raise RuntimeError(f'unsafe Neovim archive link: {member.name}')
    archive.extractall(destination)


def _is_within(root: Path, candidate: Path) -> bool:
    try:
        candidate.relative_to(root)
        return True
    except ValueError:
        return False


def _find_extracted_nvim(root: Path) -> Path | None:
    candidates = sorted(root.glob('*/bin/nvim')) + sorted(root.glob('**/bin/nvim'))
    for candidate in candidates:
        if candidate.is_file():
            candidate.chmod(candidate.stat().st_mode | 0o755)
            return candidate
    return None


def _activate_managed_nvim(paths: dict[str, Path], binary: Path) -> None:
    target = paths['managed_nvim']
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = target.with_name(f'.{target.name}.tmp')
    if tmp.exists() or tmp.is_symlink():
        tmp.unlink()
    try:
        tmp.symlink_to(binary)
    except OSError:
        tmp.write_text(
            '#!/usr/bin/env sh\n'
            f'exec {_shell_quote(str(binary))} "$@"\n',
            encoding='utf-8',
        )
        tmp.chmod(0o755)
    tmp.replace(target)


def _safe_version_dir(version: str, asset_name: str) -> str:
    text = f'{version}-{asset_name}'.replace('/', '-')
    return ''.join(ch if ch.isalnum() or ch in {'.', '_', '-'} else '-' for ch in text)


def _is_managed_binary(paths: dict[str, Path], nvim: Path) -> bool:
    try:
        return nvim.resolve() == paths['managed_nvim'].resolve()
    except Exception:
        return str(nvim) == str(paths['managed_nvim'])


def _write_lazyvim_profile(paths: dict[str, Path]) -> None:
    init_lua = paths['config_nvim'] / 'init.lua'
    if not init_lua.exists() or _is_managed_lazyvim_init(init_lua):
        init_lua.write_text(_managed_lazyvim_init_text(), encoding='utf-8')
    plugins = paths['config_nvim'] / 'lua' / 'plugins'
    plugins.mkdir(parents=True, exist_ok=True)
    terminal_compat = plugins / 'ccb-terminal-compat.lua'
    if not terminal_compat.exists() or _is_managed_lazyvim_init(terminal_compat):
        terminal_compat.write_text(_managed_lazyvim_terminal_compat_text(), encoding='utf-8')
    keep = plugins / '.keep'
    if not keep.exists():
        keep.write_text('', encoding='utf-8')
    paths['marker'].write_text('managed_by=ccb\n', encoding='utf-8')


def _is_managed_lazyvim_init(path: Path) -> bool:
    try:
        return path.read_text(encoding='utf-8').startswith('-- Managed by CCB.')
    except Exception:
        return False


def _managed_lazyvim_init_text() -> str:
    return (
        '-- Managed by CCB. This profile is isolated from ~/.config/nvim.\n'
        'vim.g.mapleader = " "\n'
        'vim.g.maplocalleader = " "\n'
        'vim.g.have_nerd_font = false\n'
        'local function ccb_terminal_compat()\n'
        '  vim.opt.fillchars = { foldopen = "-", foldclose = "+", fold = " ", foldsep = " ", diff = "/", eob = " " }\n'
        'end\n'
        'ccb_terminal_compat()\n'
        'vim.api.nvim_create_autocmd("User", { pattern = { "VeryLazy", "LazyDone" }, callback = ccb_terminal_compat })\n'
        'local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"\n'
        'vim.opt.rtp:prepend(lazypath)\n'
        'local ok, lazy = pcall(require, "lazy")\n'
        'if not ok then\n'
        '  vim.api.nvim_err_writeln("CCB LazyVim profile is not provisioned: " .. tostring(lazy))\n'
        '  return\n'
        'end\n'
        'lazy.setup({ { "LazyVim/LazyVim", import = "lazyvim.plugins" }, { import = "plugins" } })\n'
    )


def _managed_lazyvim_terminal_compat_text() -> str:
    return (
        '-- Managed by CCB. Terminal compatibility for the isolated LazyVim profile.\n'
        'local icon_style = vim.env.CCB_LAZYVIM_ICON_STYLE or "ascii"\n'
        'local ascii_icons = {\n'
        '  misc = { dots = "..." },\n'
        '  ft = { octo = "GH ", gh = "GH ", ["markdown.gh"] = "GH " },\n'
        '  dap = {\n'
        '    Stopped = { "> ", "DiagnosticWarn", "DapStoppedLine" },\n'
        '    Breakpoint = "B ",\n'
        '    BreakpointCondition = "? ",\n'
        '    BreakpointRejected = "! ",\n'
        '    LogPoint = ".>",\n'
        '  },\n'
        '  diagnostics = { Error = "E ", Warn = "W ", Hint = "H ", Info = "I " },\n'
        '  git = { added = "+ ", modified = "~ ", removed = "- " },\n'
        '  kinds = {\n'
        '    Array = "A ", Boolean = "B ", Class = "C ", Codeium = "AI ", Color = "C ",\n'
        '    Control = "Ctrl ", Collapsed = "> ", Constant = "Const ", Constructor = "Ctor ",\n'
        '    Copilot = "AI ", Enum = "Enum ", EnumMember = "Enum ", Event = "Ev ",\n'
        '    Field = "Fld ", File = "File ", Folder = "Dir ", Function = "Fn ",\n'
        '    Interface = "Iface ", Key = "Key ", Keyword = "Kw ", Method = "Meth ",\n'
        '    Module = "Mod ", Namespace = "Ns ", Null = "Null ", Number = "Num ",\n'
        '    Object = "Obj ", Operator = "Op ", Package = "Pkg ", Property = "Prop ",\n'
        '    Reference = "Ref ", Snippet = "Snip ", String = "Str ", Struct = "Struct ",\n'
        '    Supermaven = "AI ", TabNine = "AI ", Text = "Txt ", TypeParameter = "T ",\n'
        '    Unit = "Unit ", Value = "Val ", Variable = "Var ",\n'
        '  },\n'
        '}\n'
        '\n'
        'local function apply_ascii_options()\n'
        '  vim.g.have_nerd_font = icon_style == "glyph"\n'
        '  if icon_style ~= "glyph" then\n'
        '    vim.opt.fillchars = { foldopen = "-", foldclose = "+", fold = " ", foldsep = " ", diff = "/", eob = " " }\n'
        '  end\n'
        'end\n'
        '\n'
        'return {\n'
        '  {\n'
        '    "LazyVim/LazyVim",\n'
        '    opts = icon_style == "glyph" and {} or { icons = ascii_icons },\n'
        '    init = apply_ascii_options,\n'
        '  },\n'
        '  {\n'
        '    "folke/snacks.nvim",\n'
        '    opts = function(_, opts)\n'
        '      if icon_style == "glyph" then return opts end\n'
        '      opts = opts or {}\n'
        '      opts.dashboard = opts.dashboard or {}\n'
        '      opts.dashboard.preset = opts.dashboard.preset or {}\n'
        '      opts.dashboard.preset.keys = opts.dashboard.preset.keys or {}\n'
        '      for _, item in ipairs(opts.dashboard.preset.keys) do\n'
        '        item.icon = ""\n'
        '      end\n'
        '      return opts\n'
        '    end,\n'
        '  },\n'
        '  {\n'
        '    "nvim-mini/mini.icons",\n'
        '    opts = function(_, opts)\n'
        '      opts = opts or {}\n'
        '      opts.style = icon_style\n'
        '      if icon_style ~= "glyph" then\n'
        '        opts.file = vim.tbl_deep_extend("force", opts.file or {}, {\n'
        '          [".keep"] = { glyph = "K", hl = "MiniIconsGrey" },\n'
        '          ["devcontainer.json"] = { glyph = "D", hl = "MiniIconsAzure" },\n'
        '        })\n'
        '        opts.filetype = vim.tbl_deep_extend("force", opts.filetype or {}, {\n'
        '          dotenv = { glyph = "E", hl = "MiniIconsYellow" },\n'
        '        })\n'
        '      end\n'
        '      return opts\n'
        '    end,\n'
        '  },\n'
        '  {\n'
        '    "akinsho/bufferline.nvim",\n'
        '    opts = function(_, opts)\n'
        '      if icon_style == "glyph" then return opts end\n'
        '      opts = opts or {}\n'
        '      opts.options = opts.options or {}\n'
        '      opts.options.show_buffer_icons = false\n'
        '      opts.options.show_buffer_close_icons = false\n'
        '      opts.options.show_close_icon = false\n'
        '      opts.options.get_element_icon = function() return "" end\n'
        '      return opts\n'
        '    end,\n'
        '  },\n'
        '  {\n'
        '    "nvim-lualine/lualine.nvim",\n'
        '    opts = function(_, opts)\n'
        '      if icon_style == "glyph" then return opts end\n'
        '      opts = opts or {}\n'
        '      opts.sections = opts.sections or {}\n'
        '      opts.sections.lualine_z = {}\n'
        '      return opts\n'
        '    end,\n'
        '  },\n'
        '}\n'
    )


def _write_wrapper(paths: dict[str, Path], nvim: Path) -> None:
    wrapper = paths['wrapper']
    wrapper.write_text(
        '#!/usr/bin/env sh\n'
        'set -eu\n'
        f'export XDG_CONFIG_HOME={_shell_quote(str(paths["profile"] / "config"))}\n'
        f'export XDG_DATA_HOME={_shell_quote(str(paths["data"]))}\n'
        f'export XDG_STATE_HOME={_shell_quote(str(paths["state"]))}\n'
        f'export XDG_CACHE_HOME={_shell_quote(str(paths["cache"]))}\n'
        'export NVIM_APPNAME=nvim\n'
        'export COLORTERM="${COLORTERM:-truecolor}"\n'
        f'exec {_shell_quote(str(nvim))} "$@"\n',
        encoding='utf-8',
    )
    wrapper.chmod(0o755)


def _sync_lazyvim_profile(paths: dict[str, Path]) -> dict[str, object]:
    wrapper = paths['wrapper']
    if not wrapper.is_file():
        return {'status': 'failed', 'reason': 'ccb-nvim wrapper is missing'}
    bootstrap = _ensure_lazy_nvim(paths)
    if bootstrap.get('status') != 'ok':
        return {
            'status': 'failed',
            'reason': bootstrap.get('reason') or 'lazy.nvim bootstrap failed',
            'error': bootstrap.get('error'),
            'repaired': bool(bootstrap.get('repaired')),
        }
    first = _run_lazyvim_sync(paths)
    health = _check_lazyvim_health(paths) if first.get('status') == 'ok' else {'status': 'failed'}
    if first.get('status') == 'ok' and health.get('status') == 'ok':
        return {
            **first,
            'health_status': 'ok',
            'repaired': bool(bootstrap.get('repaired')),
        }
    _reset_lazyvim_plugin_dir(paths)
    bootstrap = _ensure_lazy_nvim(paths)
    if bootstrap.get('status') != 'ok':
        return {
            'status': 'failed',
            'reason': bootstrap.get('reason') or 'lazy.nvim repair failed',
            'error': bootstrap.get('error'),
            'repaired': True,
        }
    second = _run_lazyvim_sync(paths)
    if second.get('status') != 'ok':
        return {**second, 'repaired': True}
    health = _check_lazyvim_health(paths)
    if health.get('status') != 'ok':
        return {
            'status': 'failed',
            'reason': health.get('reason') or 'LazyVim health check failed',
            'error': health.get('error'),
            'health_status': health.get('status'),
            'health_error': health.get('error'),
            'repaired': True,
        }
    return {
        **second,
        'health_status': 'ok',
        'repaired': True,
    }


def _ensure_lazy_nvim(paths: dict[str, Path]) -> dict[str, object]:
    lazy_path = _lazy_nvim_path(paths)
    repaired = False
    if lazy_path.exists() and not _lazy_nvim_looks_usable(paths):
        _safe_rmtree(lazy_path, root=paths['root'])
        repaired = True
    if _lazy_nvim_looks_usable(paths):
        return {'status': 'ok', 'repaired': repaired}
    lazy_path.parent.mkdir(parents=True, exist_ok=True)
    git_result = _clone_lazy_nvim_with_git(paths, lazy_path)
    if git_result.get('status') != 'ok':
        if lazy_path.exists():
            repaired = True
        _safe_rmtree(lazy_path, root=paths['root'])
        tarball_result = _install_lazy_nvim_from_tarball(paths, lazy_path)
        if tarball_result.get('status') != 'ok':
            return {
                'status': 'failed',
                'reason': tarball_result.get('reason') or git_result.get('reason') or 'lazy.nvim bootstrap failed',
                'error': _join_errors(git_result.get('error'), tarball_result.get('error')),
                'repaired': repaired,
            }
    if not _lazy_nvim_looks_usable(paths):
        _safe_rmtree(lazy_path, root=paths['root'])
        return {
            'status': 'failed',
            'reason': 'lazy.nvim bootstrap did not install required Lua files',
            'repaired': repaired,
        }
    return {'status': 'ok', 'repaired': repaired}


def _clone_lazy_nvim_with_git(paths: dict[str, Path], lazy_path: Path) -> dict[str, object]:
    try:
        completed = subprocess.run(
            [
                'git',
                'clone',
                '--filter=blob:none',
                '--branch=stable',
                'https://github.com/folke/lazy.nvim.git',
                str(lazy_path),
            ],
            cwd=str(paths['root']),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=_lazyvim_bootstrap_timeout_s(),
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return {
            'status': 'failed',
            'reason': 'lazy.nvim git bootstrap timed out',
            'error': _process_output_text(exc.stdout, exc.stderr),
        }
    except Exception as exc:
        return {
            'status': 'failed',
            'reason': 'lazy.nvim git bootstrap failed',
            'error': f'{type(exc).__name__}: {exc}',
        }
    if completed.returncode != 0:
        return {
            'status': 'failed',
            'reason': f'lazy.nvim git bootstrap exited with {completed.returncode}',
            'error': _short_process_text((completed.stdout or '') + '\n' + (completed.stderr or '')),
        }
    return {'status': 'ok'}


def _install_lazy_nvim_from_tarball(paths: dict[str, Path], lazy_path: Path) -> dict[str, object]:
    archive_path = paths['downloads'] / 'lazy.nvim-main.tar.gz'
    tmp_root = Path(tempfile.mkdtemp(prefix='ccb-lazy-nvim-', dir=str(paths['root'])))
    try:
        paths['downloads'].mkdir(parents=True, exist_ok=True)
        download_error = _download_first_available(LAZY_NVIM_TARBALL_URLS, archive_path)
        if download_error is not None:
            raise RuntimeError(download_error)
        with tarfile.open(archive_path, 'r:gz') as archive:
            _safe_extract_tar(archive, tmp_root)
        extracted = _find_lazy_nvim_extract_root(tmp_root)
        if extracted is None:
            raise RuntimeError('lazy.nvim tarball did not contain lua/lazy/init.lua')
        if lazy_path.exists():
            _safe_rmtree(lazy_path, root=paths['root'])
        lazy_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(extracted), str(lazy_path))
    except Exception as exc:
        _safe_rmtree(lazy_path, root=paths['root'])
        return {
            'status': 'failed',
            'reason': 'lazy.nvim tarball bootstrap failed',
            'error': f'{type(exc).__name__}: {exc}',
        }
    finally:
        shutil.rmtree(tmp_root, ignore_errors=True)
    return {'status': 'ok'}


def _download_first_available(urls: tuple[str, ...], destination: Path) -> str | None:
    errors: list[str] = []
    for url in urls:
        try:
            _download_file(url, destination, timeout_s=_lazyvim_bootstrap_timeout_s())
            return None
        except Exception as exc:
            errors.append(f'{url}: {type(exc).__name__}: {exc}')
    return '\n'.join(errors)


def _find_lazy_nvim_extract_root(root: Path) -> Path | None:
    for candidate in sorted(root.iterdir()):
        if candidate.is_dir() and (candidate / 'lua' / 'lazy' / 'init.lua').is_file():
            return candidate
    for candidate in sorted(root.glob('**/lua/lazy/init.lua')):
        parent = candidate.parents[2]
        if parent.is_dir():
            return parent
    return None


def _lazy_nvim_looks_usable(paths: dict[str, Path]) -> bool:
    return (_lazy_nvim_path(paths) / 'lua' / 'lazy' / 'init.lua').is_file()


def _lazy_nvim_path(paths: dict[str, Path]) -> Path:
    return paths['data'] / 'nvim' / 'lazy' / 'lazy.nvim'


def _lazyvim_plugin_path(paths: dict[str, Path]) -> Path:
    return paths['data'] / 'nvim' / 'lazy' / 'LazyVim'


def _reset_lazyvim_plugin_dir(paths: dict[str, Path]) -> None:
    _safe_rmtree(paths['data'] / 'nvim' / 'lazy', root=paths['root'])


def _safe_rmtree(path: Path, *, root: Path) -> None:
    try:
        resolved_path = path.resolve()
        resolved_root = root.resolve()
        resolved_path.relative_to(resolved_root)
    except Exception:
        return
    shutil.rmtree(resolved_path, ignore_errors=True)


def _run_lazyvim_sync(paths: dict[str, Path]) -> dict[str, object]:
    wrapper = paths['wrapper']
    try:
        completed = subprocess.run(
            [str(wrapper), '--headless', '+Lazy! sync', '+qa'],
            cwd=str(paths['root']),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=_lazyvim_sync_timeout_s(),
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return {
            'status': 'failed',
            'reason': 'LazyVim sync timed out',
            'error': _process_output_text(exc.stdout, exc.stderr),
        }
    except Exception as exc:
        return {
            'status': 'failed',
            'reason': 'LazyVim sync failed',
            'error': f'{type(exc).__name__}: {exc}',
        }
    if completed.returncode == 0:
        return {'status': 'ok'}
    return {
        'status': 'failed',
        'reason': f'LazyVim sync exited with {completed.returncode}',
        'error': _short_process_text((completed.stdout or '') + '\n' + (completed.stderr or '')),
    }


def _check_lazyvim_health(paths: dict[str, Path]) -> dict[str, object]:
    wrapper = paths['wrapper']
    if not wrapper.is_file():
        return {'status': 'failed', 'reason': 'ccb-nvim wrapper is missing'}
    script = (
        'local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"; '
        'vim.opt.rtp:prepend(lazypath); '
        'local ok_lazy, lazy_err = pcall(require, "lazy"); '
        'if not ok_lazy then error("lazy.nvim not loadable: " .. tostring(lazy_err)) end; '
        'local lazyvim_init = vim.fn.stdpath("data") .. "/lazy/LazyVim/lua/lazyvim/init.lua"; '
        'if vim.fn.filereadable(lazyvim_init) ~= 1 then error("LazyVim plugin files missing: " .. lazyvim_init) end; '
        'print("ccb_lazyvim_health=ok")'
    )
    try:
        completed = subprocess.run(
            [str(wrapper), '--clean', '--headless', '+lua ' + script, '+qa'],
            cwd=str(paths['root']),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=min(_lazyvim_sync_timeout_s(), 30.0),
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return {
            'status': 'failed',
            'reason': 'LazyVim health check timed out',
            'error': _process_output_text(exc.stdout, exc.stderr),
        }
    except Exception as exc:
        return {
            'status': 'failed',
            'reason': 'LazyVim health check failed',
            'error': f'{type(exc).__name__}: {exc}',
        }
    if completed.returncode == 0 and (_lazyvim_plugin_path(paths) / 'lua' / 'lazyvim' / 'init.lua').is_file():
        return {'status': 'ok'}
    return {
        'status': 'failed',
        'reason': 'LazyVim health check failed',
        'error': _short_process_text((completed.stdout or '') + '\n' + (completed.stderr or '')),
    }


def _write_bin_link(paths: dict[str, Path]) -> None:
    link = paths['bin_link']
    link.parent.mkdir(parents=True, exist_ok=True)
    try:
        if link.is_symlink() or link.exists():
            link.unlink()
        link.symlink_to(paths['wrapper'])
    except Exception:
        shutil.copy2(paths['wrapper'], link)
        link.chmod(0o755)


def _shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\"'\"'") + "'"


def _read_manifest(paths: dict[str, Path]) -> dict[str, object]:
    try:
        return json.loads(paths['manifest'].read_text(encoding='utf-8'))
    except Exception:
        return {}


def _write_manifest(paths: dict[str, Path], payload: dict[str, object]) -> None:
    paths['manifest'].parent.mkdir(parents=True, exist_ok=True)
    paths['manifest'].write_text(json.dumps(payload, indent=2, sort_keys=True) + '\n', encoding='utf-8')


def _status_paths(paths: dict[str, Path]) -> dict[str, object]:
    return {
        'root': str(paths['root']),
        'config_home': str(paths['profile'] / 'config'),
        'data_home': str(paths['data']),
        'state_home': str(paths['state']),
        'cache_home': str(paths['cache']),
    }


def _print_status(status: dict[str, object], stdout: TextIO) -> None:
    print(f"neovim_status: {status.get('status')}", file=stdout)
    for key in (
        'reason',
        'binary',
        'wrapper',
        'bin_link',
        'managed_neovim_target',
        'lazyvim_profile',
        'lazyvim_sync_status',
        'lazyvim_sync_error',
        'lazyvim_health_status',
        'lazyvim_health_error',
        'root',
        'config_home',
        'data_home',
        'state_home',
        'cache_home',
    ):
        value = status.get(key)
        if value:
            print(f'{key}: {value}', file=stdout)


def _print_help(stdout: TextIO) -> None:
    print('usage: ccb tools <doctor|install|update> neovim', file=stdout)


def _install_required() -> bool:
    return str(os.environ.get('CCB_INSTALL_NEOVIM') or '').strip() == '1'


def _lazyvim_profile_enabled() -> bool:
    return str(os.environ.get('CCB_LAZYVIM_PROFILE') or '').strip().lower() not in {'0', 'false', 'off'}


def _lazyvim_sync_timeout_s() -> float:
    raw = str(os.environ.get('CCB_LAZYVIM_SYNC_TIMEOUT_S') or '').strip()
    if not raw:
        return LAZYVIM_SYNC_TIMEOUT_S
    try:
        return max(0.0, float(raw))
    except ValueError:
        return LAZYVIM_SYNC_TIMEOUT_S


def _lazyvim_bootstrap_timeout_s() -> float:
    raw = str(os.environ.get('CCB_LAZYVIM_BOOTSTRAP_TIMEOUT_S') or '').strip()
    if not raw:
        return LAZYVIM_BOOTSTRAP_TIMEOUT_S
    try:
        return max(0.0, float(raw))
    except ValueError:
        return LAZYVIM_BOOTSTRAP_TIMEOUT_S


def _short_process_text(value: object, *, limit: int = 2000) -> str:
    text = str(value or '').strip()
    if len(text) <= limit:
        return text
    return text[:limit] + '...'


def _process_output_text(*values: object) -> str:
    parts = [_decode_process_output(value) for value in values]
    return _short_process_text('\n'.join(part for part in parts if part))


def _decode_process_output(value: object) -> str:
    if value is None:
        return ''
    if isinstance(value, bytes):
        return value.decode('utf-8', errors='replace')
    return str(value)


def _join_errors(*values: object) -> str:
    parts = [str(value).strip() for value in values if str(value or '').strip()]
    return '\n'.join(parts)


__all__ = ['cmd_tools', 'neovim_status', 'provision_neovim']
