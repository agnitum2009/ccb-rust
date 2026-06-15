from __future__ import annotations

from io import StringIO
from pathlib import Path
from types import SimpleNamespace

from cli.phase2 import maybe_handle_phase2


def test_phase2_clear_sends_request_and_renders_summary(monkeypatch, tmp_path: Path) -> None:
    import cli.phase2 as phase2_module

    fake_context = SimpleNamespace(project=SimpleNamespace(project_root=tmp_path, project_id='proj-clear'))
    calls: list[tuple[str, tuple[str, ...]]] = []

    monkeypatch.setattr(phase2_module, '_build_context', lambda command, cwd, out: fake_context)
    monkeypatch.setattr(phase2_module, 'ensure_bootstrap_project_config', lambda project_root: None)

    def _clear_agent_context(context, command):
        calls.append((context.project.project_id, command.agent_names))
        return {
            'status': 'ok',
            'results': [
                {'agent': 'agent1', 'status': 'cleared', 'pane_id': '%1'},
            ],
        }

    monkeypatch.setattr(phase2_module, 'clear_agent_context', _clear_agent_context)

    stdout = StringIO()
    stderr = StringIO()
    code = maybe_handle_phase2(
        ['clear', 'agent1'],
        cwd=tmp_path,
        stdout=stdout,
        stderr=stderr,
    )

    assert code == 0
    assert calls == [('proj-clear', ('agent1',))]
    assert stdout.getvalue() == (
        'clear_status: ok\n'
        'cleared_count: 1\n'
        'skipped_count: 0\n'
        'failed_count: 0\n'
        'clear_agent: agent=agent1 status=cleared pane_id=%1\n'
    )
    assert stderr.getvalue() == ''
