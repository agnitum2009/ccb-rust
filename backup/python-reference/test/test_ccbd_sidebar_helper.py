from __future__ import annotations

from pathlib import Path

from ccbd.services.project_namespace_runtime.sidebar_helper import (
    SIDEBAR_ENV_PATH,
    missing_sidebar_respawn_args,
    resolve_sidebar_helper,
    sidebar_respawn_args,
)


def test_resolve_sidebar_helper_prefers_explicit_env_path(tmp_path: Path) -> None:
    helper = tmp_path / 'custom-sidebar'
    helper.write_text('#!/bin/sh\n', encoding='utf-8')
    helper.chmod(0o755)

    resolution = resolve_sidebar_helper(
        env={SIDEBAR_ENV_PATH: str(helper)},
        which=lambda name: None,
        script_root=tmp_path / 'repo',
    )

    assert resolution.path == str(helper)
    assert resolution.source == SIDEBAR_ENV_PATH


def test_resolve_sidebar_helper_finds_repository_bin(tmp_path: Path) -> None:
    helper = tmp_path / 'repo' / 'bin' / 'ccb-agent-sidebar'
    helper.parent.mkdir(parents=True)
    helper.write_text('#!/bin/sh\n', encoding='utf-8')
    helper.chmod(0o755)

    resolution = resolve_sidebar_helper(
        env={},
        which=lambda name: None,
        script_root=tmp_path / 'repo',
    )

    assert resolution.path == str(helper)
    assert resolution.source == 'script_root_bin'


def test_resolve_sidebar_helper_uses_path_as_last_discovery_source(tmp_path: Path) -> None:
    resolution = resolve_sidebar_helper(
        env={},
        which=lambda name: '/usr/local/bin/ccb-agent-sidebar',
        script_root=tmp_path / 'repo',
    )

    assert resolution.path == '/usr/local/bin/ccb-agent-sidebar'
    assert resolution.source == 'PATH'


def test_sidebar_respawn_args_replaces_symbolic_binary_with_resolved_path(tmp_path: Path) -> None:
    helper = tmp_path / 'ccb-agent-sidebar'
    helper.write_text('#!/bin/sh\n', encoding='utf-8')
    helper.chmod(0o755)

    args = sidebar_respawn_args(
        ('ccb-agent-sidebar', '--pane-window', 'main'),
        env={SIDEBAR_ENV_PATH: str(helper)},
        which=lambda name: None,
        script_root=tmp_path / 'repo',
    )

    assert args == (str(helper), '--pane-window', 'main')


def test_sidebar_respawn_args_falls_back_to_visible_keepalive_message(tmp_path: Path) -> None:
    args = sidebar_respawn_args(
        ('ccb-agent-sidebar', '--pane-window', 'main'),
        env={},
        which=lambda name: None,
        script_root=tmp_path / 'repo',
    )

    assert args[:2] == ('sh', '-lc')
    assert 'CCB sidebar helper unavailable' in args[2]
    assert 'while :; do sleep 3600; done' in args[2]
    assert missing_sidebar_respawn_args()[0] == 'sh'
