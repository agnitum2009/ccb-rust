from __future__ import annotations

import argparse
from pathlib import Path
from typing import TextIO

from agents.models import parse_layout_spec
from project.discovery import find_nearest_project_anchor
from rolepacks import (
    RolePackError,
    add_role_to_project_config,
    install_role,
    list_builtin_roles,
    load_installed_role,
    role_status,
    update_role,
)


def cmd_roles(
    argv: list[str],
    *,
    script_root: Path,
    cwd: Path,
    stdout: TextIO,
    stderr: TextIO,
) -> int:
    parser = _build_parser()
    try:
        args = parser.parse_args(argv)
    except SystemExit as exc:
        return int(exc.code or 2)
    try:
        if args.command == 'list':
            return _cmd_list(args, script_root=script_root, stdout=stdout)
        if args.command == 'show':
            return _cmd_show(args, script_root=script_root, stdout=stdout)
        if args.command == 'install':
            return _cmd_install(args, script_root=script_root, stdout=stdout)
        if args.command == 'update':
            return _cmd_update(args, script_root=script_root, stdout=stdout)
        if args.command == 'doctor':
            return _cmd_doctor(args, script_root=script_root, stdout=stdout)
        if args.command == 'add':
            return _cmd_add(args, script_root=script_root, cwd=cwd, stdout=stdout)
    except RolePackError as exc:
        print(f'roles_status: failed\nerror: {exc}', file=stderr)
        return 1
    parser.print_help(file=stderr)
    return 2


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog='ccb roles', add_help=True)
    sub = parser.add_subparsers(dest='command')
    sub.add_parser('list')
    show = sub.add_parser('show')
    show.add_argument('role_id')
    install = sub.add_parser('install')
    install.add_argument('role_id')
    install.add_argument('--skip-tools', action='store_true', default=False)
    update = sub.add_parser('update')
    update.add_argument('role_id')
    update.add_argument('--skip-tools', action='store_true', default=False)
    doctor = sub.add_parser('doctor')
    doctor.add_argument('role_id')
    add = sub.add_parser('add')
    add.add_argument('role_spec')
    add.add_argument('--agent', default=None)
    add.add_argument('--provider', default=None)
    add.add_argument('--window', default=None)
    return parser


def _cmd_list(args, *, script_root: Path, stdout: TextIO) -> int:
    roles = list_builtin_roles(script_root=script_root)
    print('roles_status: ok', file=stdout)
    for role in roles:
        print(
            f'role: id={role.id} name={role.name} version={role.version} '
            f'providers={",".join(role.providers)}',
            file=stdout,
        )
    return 0


def _cmd_show(args, *, script_root: Path, stdout: TextIO) -> int:
    role = load_installed_role(args.role_id)
    if role is None:
        role = next((item for item in list_builtin_roles(script_root=script_root) if item.id == args.role_id), None)
    if role is None:
        raise RolePackError(f'unknown role: {args.role_id}')
    print('roles_status: ok', file=stdout)
    for key, value in role.to_summary().items():
        print(f'{key}: {value}', file=stdout)
    return 0


def _cmd_install(args, *, script_root: Path, stdout: TextIO) -> int:
    payload = install_role(args.role_id, script_root=script_root, with_tools=not bool(args.skip_tools))
    _print_payload(payload, stdout=stdout)
    return 0


def _cmd_update(args, *, script_root: Path, stdout: TextIO) -> int:
    payload = update_role(args.role_id, script_root=script_root, with_tools=not bool(args.skip_tools))
    _print_payload(payload, stdout=stdout)
    return 0


def _cmd_doctor(args, *, script_root: Path, stdout: TextIO) -> int:
    payload = role_status(args.role_id, script_root=script_root, include_tools=True)
    exists = bool(payload.get('builtin') or payload.get('installed'))
    tools_failed = payload.get('tools_status') == 'failed'
    print('roles_status: ok' if exists and not tools_failed else 'roles_status: missing' if not exists else 'roles_status: degraded', file=stdout)
    _print_payload(payload, stdout=stdout)
    return 0 if exists and not tools_failed else 1


def _cmd_add(args, *, script_root: Path, cwd: Path, stdout: TextIO) -> int:
    project_root = find_nearest_project_anchor(cwd)
    if project_root is None:
        raise RolePackError('cannot find a project .ccb anchor for roles add')
    role_id, provider_from_spec = _parse_add_role_spec(args.role_spec)
    payload = add_role_to_project_config(
        project_root=project_root,
        role_id=role_id,
        agent_name=args.agent,
        provider=args.provider or provider_from_spec,
        window_name=args.window,
        script_root=script_root,
    )
    for key, value in payload.items():
        if value != '':
            print(f'{key}: {value}', file=stdout)
    return 0


def _print_payload(payload: dict[str, object], *, stdout: TextIO) -> None:
    tools = payload.get('tools')
    for key, value in payload.items():
        if key == 'tools':
            continue
        print(f'{key}: {value}', file=stdout)
    if isinstance(tools, tuple):
        for item in tools:
            if not isinstance(item, dict):
                continue
            print(
                'tool: '
                f'id={item.get("tool_id")} '
                f'action={item.get("action")} '
                f'status={item.get("status")} '
                f'required={str(bool(item.get("required"))).lower()}',
                file=stdout,
            )
            stdout_text = str(item.get('stdout') or '').strip()
            stderr_text = str(item.get('stderr') or '').strip()
            if stdout_text:
                print(f'tool_{item.get("tool_id")}_stdout: {_one_line(stdout_text)}', file=stdout)
            if stderr_text:
                print(f'tool_{item.get("tool_id")}_stderr: {_one_line(stderr_text)}', file=stdout)


def _one_line(text: str) -> str:
    return ' | '.join(line.strip() for line in text.splitlines() if line.strip())


def _parse_add_role_spec(value: str) -> tuple[str, str | None]:
    text = str(value or '').strip()
    if not text:
        raise RolePackError('roles add requires a role spec, for example ccb.archi:codex')
    try:
        node = parse_layout_spec(text)
    except Exception as exc:
        raise RolePackError(f'invalid role spec {text!r}; expected role.id or role.id:provider') from exc
    if node.kind != 'leaf' or node.leaf is None:
        raise RolePackError(f'invalid role spec {text!r}; expected a single role leaf')
    if node.leaf.workspace_mode:
        raise RolePackError('roles add role spec does not accept workspace mode; configure workspace separately')
    return node.leaf.name, node.leaf.provider


__all__ = ['cmd_roles']
