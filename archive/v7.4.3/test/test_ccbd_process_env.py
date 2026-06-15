from __future__ import annotations

import os
from pathlib import Path

from ccbd.daemon_process import _ccbd_env, _prepend_tool_paths


def test_ccbd_env_prefers_current_worktree_tools(monkeypatch) -> None:
    monkeypatch.setenv('PATH', os.pathsep.join(['/usr/bin', '/bin']))
    monkeypatch.setenv('PYTHONPATH', '/stable/ccb/lib:/other')

    env = _ccbd_env(keeper_pid=123)
    script_root = Path(__file__).resolve().parents[1]
    lib_root = script_root / 'lib'
    parts = env['PATH'].split(os.pathsep)

    assert parts[:2] == [str(script_root / 'bin'), str(script_root)]
    assert env['PYTHONPATH'] == str(lib_root)
    assert env['CCB_KEEPER_PID'] == '123'


def test_prepend_tool_paths_deduplicates_existing_entries(tmp_path: Path) -> None:
    root = tmp_path / 'repo'
    (root / 'bin').mkdir(parents=True)
    env = {'PATH': os.pathsep.join([str(root), '/usr/bin', str(root / 'bin')])}

    _prepend_tool_paths(env, root)

    assert env['PATH'].split(os.pathsep) == [str(root / 'bin'), str(root), '/usr/bin']
