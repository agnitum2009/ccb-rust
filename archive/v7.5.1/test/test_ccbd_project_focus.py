from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import pytest

from agents.models import AgentSpec, PermissionMode, ProjectConfig, QueuePolicy, RestoreMode, RuntimeMode, ToolWindowSpec, WindowSpec, WorkspaceMode
from ccbd.project_focus import ProjectFocusDependencies, ProjectFocusError, ProjectFocusService
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_state import ProjectNamespaceState, ProjectNamespaceStateStore
from storage.paths import PathLayout


def _spec(name: str, provider: str = 'codex') -> AgentSpec:
    return AgentSpec(
        name=name,
        provider=provider,
        target='.',
        workspace_mode=WorkspaceMode.INPLACE,
        workspace_root=None,
        runtime_mode=RuntimeMode.PANE_BACKED,
        restore_default=RestoreMode.AUTO,
        permission_default=PermissionMode.MANUAL,
        queue_policy=QueuePolicy.SERIAL_PER_AGENT,
    )


def _config() -> ProjectConfig:
    return ProjectConfig(
        version=2,
        default_agents=('agent1', 'agent2'),
        agents={'agent1': _spec('agent1'), 'agent2': _spec('agent2', 'claude')},
        cmd_enabled=False,
        layout_spec='agent1:codex',
        windows=(
            WindowSpec(name='main', order=0, layout_spec='agent1:codex', agent_names=('agent1',)),
            WindowSpec(name='ops', order=1, layout_spec='agent2:claude', agent_names=('agent2',)),
        ),
        entry_window='main',
    )


def _config_with_tool_window() -> ProjectConfig:
    return ProjectConfig(
        version=2,
        default_agents=('agent1', 'agent2'),
        agents={'agent1': _spec('agent1'), 'agent2': _spec('agent2', 'claude')},
        cmd_enabled=False,
        layout_spec='agent1:codex',
        windows=(
            WindowSpec(name='main', order=0, layout_spec='agent1:codex', agent_names=('agent1',)),
            WindowSpec(name='ops', order=1, layout_spec='agent2:claude', agent_names=('agent2',)),
        ),
        tool_windows=(ToolWindowSpec(name='neovim', order=0, command='ccb-nvim'),),
        entry_window='main',
    )


class _FakeTmuxBackend:
    def __init__(self) -> None:
        self.calls: list[list[str]] = []
        self.panes = {
            '%1': {
                '@ccb_project_id': 'proj-1',
                '@ccb_role': 'agent',
                '@ccb_slot': 'agent1',
                '@ccb_window': 'main',
                '@ccb_managed_by': 'ccbd',
            },
            '%2': {
                '@ccb_project_id': 'proj-1',
                '@ccb_role': 'agent',
                '@ccb_slot': 'agent2',
                '@ccb_window': 'ops',
                '@ccb_managed_by': 'ccbd',
            },
            '%3': {
                '@ccb_project_id': 'proj-1',
                '@ccb_role': 'sidebar',
                '@ccb_window': 'main',
                '@ccb_managed_by': 'ccbd',
                'session_name': 'ccb-test',
            },
            '%4': {
                '@ccb_project_id': 'proj-1',
                '@ccb_role': 'sidebar',
                '@ccb_window': 'ops',
                '@ccb_managed_by': 'ccbd',
                'session_name': 'ccb-test',
            },
            '%5': {
                '@ccb_project_id': 'proj-1',
                '@ccb_role': 'sidebar',
                '@ccb_window': 'foreign',
                '@ccb_managed_by': 'ccbd',
                'session_name': 'other-session',
            },
        }
        self.missing_windows: set[str] = set()
        self.missing_panes: set[str] = set()

    def list_panes_by_user_options(self, expected: dict[str, str]) -> list[str]:
        matches = []
        for pane_id, options in self.panes.items():
            if pane_id in self.missing_panes:
                continue
            if all(options.get(name) == value for name, value in expected.items()):
                matches.append(pane_id)
        return matches

    def _tmux_run(self, args: list[str], *, capture=False, check=False, timeout=None):
        del check, timeout
        self.calls.append(list(args))
        if args[:2] == ['select-window', '-t']:
            window = args[2].split(':', 1)[1]
            return SimpleNamespace(returncode=1 if window in self.missing_windows else 0, stdout='', stderr='')
        if args[:2] == ['select-pane', '-t']:
            return SimpleNamespace(returncode=1 if args[2] in self.missing_panes else 0, stdout='', stderr='')
        if args[:3] == ['display-message', '-p', '-t']:
            pane_id = args[3]
            if args[4] == '#{session_name}':
                return SimpleNamespace(
                    returncode=0,
                    stdout=self.panes.get(pane_id, {}).get('session_name', 'ccb-test') + '\n',
                    stderr='',
                )
        if args[:2] == ['send-keys', '-t']:
            return SimpleNamespace(returncode=0, stdout='', stderr='')
        raise AssertionError(f'unexpected tmux args: {args}, capture={capture}')


def _service(
    tmp_path: Path,
    backend: _FakeTmuxBackend,
    *,
    epoch: int = 4,
    project_view_service: object | None = None,
    config: ProjectConfig | None = None,
) -> ProjectFocusService:
    project_root = tmp_path / 'repo'
    project_root.mkdir()
    layout = PathLayout(project_root)
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id='proj-1',
            namespace_epoch=epoch,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name='ccb-test',
            layout_version=2,
        )
    )
    controller = ProjectNamespaceController(
        layout,
        'proj-1',
        backend_factory=lambda socket_path=None: backend,
    )
    return ProjectFocusService(
        ProjectFocusDependencies(
            project_id='proj-1',
            config=config or _config(),
            namespace_controller=controller,
            project_view_service=project_view_service,
        )
    )


def test_project_focus_agent_selects_configured_window_and_pane(tmp_path: Path) -> None:
    backend = _FakeTmuxBackend()
    service = _service(tmp_path, backend)

    result = service.focus_agent(agent='agent2', namespace_epoch=4)

    assert result == {
        'focused': True,
        'kind': 'agent',
        'window': 'ops',
        'agent': 'agent2',
        'namespace_epoch': 4,
    }
    assert backend.calls == [
        ['select-window', '-t', 'ccb-test:ops'],
        ['select-pane', '-t', '%2'],
        ['display-message', '-p', '-t', '%3', '#{session_name}'],
        ['display-message', '-p', '-t', '%4', '#{session_name}'],
        ['display-message', '-p', '-t', '%5', '#{session_name}'],
        ['send-keys', '-t', '%3', 'C-l'],
        ['send-keys', '-t', '%4', 'C-l'],
    ]


def test_project_focus_window_focuses_first_configured_agent_when_available(tmp_path: Path) -> None:
    backend = _FakeTmuxBackend()
    service = _service(tmp_path, backend)

    result = service.focus_window(window='main')

    assert result['focused'] is True
    assert result['kind'] == 'window'
    assert result['agent'] == 'agent1'
    assert backend.calls == [
        ['select-window', '-t', 'ccb-test:main'],
        ['select-pane', '-t', '%1'],
        ['display-message', '-p', '-t', '%3', '#{session_name}'],
        ['display-message', '-p', '-t', '%4', '#{session_name}'],
        ['display-message', '-p', '-t', '%5', '#{session_name}'],
        ['send-keys', '-t', '%3', 'C-l'],
        ['send-keys', '-t', '%4', 'C-l'],
    ]


def test_project_focus_tool_window_selects_window_without_agent_pane_lookup(tmp_path: Path) -> None:
    backend = _FakeTmuxBackend()
    service = _service(tmp_path, backend, config=_config_with_tool_window())

    result = service.focus_window(window='neovim')

    assert result['focused'] is True
    assert result['kind'] == 'window'
    assert result['window'] == 'neovim'
    assert result['agent'] is None
    assert backend.calls == [
        ['select-window', '-t', 'ccb-test:neovim'],
        ['display-message', '-p', '-t', '%3', '#{session_name}'],
        ['display-message', '-p', '-t', '%4', '#{session_name}'],
        ['display-message', '-p', '-t', '%5', '#{session_name}'],
        ['send-keys', '-t', '%3', 'C-l'],
        ['send-keys', '-t', '%4', 'C-l'],
    ]


def test_project_focus_success_invalidates_project_view_cache(tmp_path: Path) -> None:
    class _ProjectView:
        invalidated = 0

        def invalidate_cache(self) -> None:
            self.invalidated += 1

    backend = _FakeTmuxBackend()
    project_view = _ProjectView()
    service = _service(tmp_path, backend, project_view_service=project_view)

    service.focus_agent(agent='agent2', namespace_epoch=4)

    assert project_view.invalidated == 1


def test_project_focus_rejects_stale_namespace_epoch(tmp_path: Path) -> None:
    service = _service(tmp_path, _FakeTmuxBackend())

    with pytest.raises(ProjectFocusError, match='stale_view'):
        service.focus_agent(agent='agent1', namespace_epoch=3)


def test_project_focus_rejects_unknown_agent(tmp_path: Path) -> None:
    service = _service(tmp_path, _FakeTmuxBackend())

    with pytest.raises(ProjectFocusError, match='unknown_agent'):
        service.focus_agent(agent='missing')


def test_project_focus_reports_missing_agent_pane(tmp_path: Path) -> None:
    backend = _FakeTmuxBackend()
    backend.missing_panes.add('%2')
    service = _service(tmp_path, backend)

    with pytest.raises(ProjectFocusError, match='target_missing'):
        service.focus_agent(agent='agent2')


def test_project_focus_reports_missing_window(tmp_path: Path) -> None:
    backend = _FakeTmuxBackend()
    backend.missing_windows.add('ops')
    service = _service(tmp_path, backend)

    with pytest.raises(ProjectFocusError, match='target_missing'):
        service.focus_agent(agent='agent2')
