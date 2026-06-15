from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

from ccbd.keeper import KeeperState, KeeperStateStore
from cli.services.daemon_runtime import keeper as keeper_runtime
from storage.paths import PathLayout


class _NoopStartupLock:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb) -> bool:
        return False


def test_spawn_keeper_process_uses_lib_root_keeper_main(tmp_path: Path, monkeypatch) -> None:
    project_root = tmp_path / 'repo'
    paths = PathLayout(project_root)
    context = SimpleNamespace(
        project=SimpleNamespace(project_root=project_root),
        paths=paths,
    )
    popen_calls: list[dict[str, object]] = []

    class _FakePopen:
        def __init__(self, cmd, **kwargs) -> None:
            popen_calls.append({'cmd': cmd, **kwargs})

    monkeypatch.setattr(keeper_runtime.subprocess, 'Popen', _FakePopen)

    keeper_runtime.spawn_keeper_process(context)

    assert len(popen_calls) == 1
    call = popen_calls[0]
    expected_script = Path(keeper_runtime.__file__).resolve().parents[3] / 'ccbd' / 'keeper_main.py'
    assert call['cmd'][1] == str(expected_script)
    assert str(expected_script.parent.parent) in str(call['env']['PYTHONPATH'])


def test_ensure_keeper_started_replaces_state_for_unrelated_live_pid(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo'
    paths = PathLayout(project_root)
    context = SimpleNamespace(
        project=SimpleNamespace(project_id='project-1', project_root=project_root),
        paths=paths,
    )
    KeeperStateStore(paths).save(
        KeeperState(
            project_id='project-1',
            keeper_pid=28,
            started_at='2026-05-23T00:00:00Z',
            last_check_at='2026-05-23T00:00:00Z',
            state='running',
        )
    )
    spawn_calls: list[object] = []

    def _spawn(ctx) -> None:
        spawn_calls.append(ctx)
        KeeperStateStore(paths).save(
            KeeperState(
                project_id='project-1',
                keeper_pid=777,
                started_at='2026-05-23T00:00:01Z',
                last_check_at='2026-05-23T00:00:01Z',
                state='running',
            )
        )

    assert keeper_runtime.ensure_keeper_started(
        context,
        mount_manager_factory=lambda _paths: object(),
        ownership_guard_factory=lambda _paths, _manager: SimpleNamespace(startup_lock=lambda: _NoopStartupLock()),
        process_exists_fn=lambda pid: pid in {28, 777},
        process_cmdline_fn=lambda pid: {
            28: ('[idle_inject/4]',),
            777: ('python3', '/repo/lib/ccbd/keeper_main.py', '--project', str(project_root)),
        }.get(pid, ()),
        spawn_keeper_process_fn=_spawn,
        ready_timeout_s=0.01,
    )

    assert spawn_calls == [context]


def test_ensure_keeper_started_reuses_matching_keeper_state(tmp_path: Path) -> None:
    project_root = tmp_path / 'repo'
    paths = PathLayout(project_root)
    context = SimpleNamespace(
        project=SimpleNamespace(project_id='project-1', project_root=project_root),
        paths=paths,
    )
    KeeperStateStore(paths).save(
        KeeperState(
            project_id='project-1',
            keeper_pid=777,
            started_at='2026-05-23T00:00:00Z',
            last_check_at='2026-05-23T00:00:00Z',
            state='running',
        )
    )
    spawn_calls: list[object] = []

    assert keeper_runtime.ensure_keeper_started(
        context,
        mount_manager_factory=lambda _paths: object(),
        ownership_guard_factory=lambda _paths, _manager: SimpleNamespace(startup_lock=lambda: _NoopStartupLock()),
        process_exists_fn=lambda pid: pid == 777,
        process_cmdline_fn=lambda pid: ('python3', '/repo/lib/ccbd/keeper_main.py', '--project', str(project_root)),
        spawn_keeper_process_fn=lambda ctx: spawn_calls.append(ctx),
        ready_timeout_s=0.01,
    )

    assert spawn_calls == []
