from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from types import SimpleNamespace

import pytest

from agents.config_loader import load_project_config
from ccbd.app import CcbdApp
from ccbd.reload_plan import build_reload_dry_run_plan
from ccbd.services.project_namespace import ProjectNamespaceController
from ccbd.services.project_namespace_runtime import (
    assert_preserved_agent_panes,
    build_namespace_topology_plan,
    snapshot_preserved_agent_panes,
)
from ccbd.services.project_namespace_state import ProjectNamespaceState, ProjectNamespaceStateStore
from storage.paths import PathLayout


BASE_CONFIG = """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
"""


ADD_WINDOW_CONFIG = """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
review = "agent3:codex, agent4:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
"""


ADD_TOOL_WINDOW_CONFIG = BASE_CONFIG + """
[tool_windows.neovim]
command = "ccb-nvim"
label = "neovim"
"""


@dataclass
class _PatchFakeBackend:
    socket_path: str | None = None
    sessions: dict[str, list[dict[str, object]]] = field(default_factory=dict)
    pane_options: dict[str, dict[str, str]] = field(default_factory=dict)
    pane_titles: dict[str, str] = field(default_factory=dict)
    split_calls: list[tuple[str, str, int]] = field(default_factory=list)
    tmux_calls: list[tuple[str, ...]] = field(default_factory=list)
    respawn_calls: list[tuple[str, str]] = field(default_factory=list)
    pane_counter: int = 0
    window_counter: int = 0

    def add_window(self, session_name: str, window_name: str) -> str:
        pane_id = self._alloc_pane()
        self.window_counter += 1
        self.sessions.setdefault(session_name, []).append(
            {
                'id': f'@{self.window_counter}',
                'name': window_name,
                'panes': [pane_id],
            }
        )
        return pane_id

    def split_pane(self, parent_pane_id: str, direction: str, percent: int, cmd=None, cwd=None) -> str:
        del cmd, cwd
        self.split_calls.append((parent_pane_id, direction, percent))
        for windows in self.sessions.values():
            for record in windows:
                panes = record['panes']
                if parent_pane_id in panes:
                    pane_id = self._alloc_pane()
                    panes.append(pane_id)
                    return pane_id
        raise RuntimeError(f'pane not found: {parent_pane_id}')

    def list_panes_by_user_options(self, expected: dict[str, str]) -> list[str]:
        matches = []
        for pane_id, options in self.pane_options.items():
            if all(str(options.get(key, '') or '') == str(value) for key, value in expected.items()):
                matches.append(pane_id)
        return matches

    def respawn_pane(self, pane_id: str, *, cmd: str, cwd: str | None = None, remain_on_exit: bool = True) -> None:
        del cwd, remain_on_exit
        self.respawn_calls.append((pane_id, cmd))

    def kill_pane(self, pane_id: str) -> None:
        self.tmux_calls.append(('kill-pane', '-t', pane_id))
        for windows in self.sessions.values():
            for record in windows:
                panes = record['panes']
                if pane_id in panes:
                    panes.remove(pane_id)
        self.pane_options.pop(pane_id, None)

    def set_pane_title(self, pane_id: str, title: str) -> None:
        self.pane_titles[pane_id] = title

    def set_pane_user_option(self, pane_id: str, name: str, value: str) -> None:
        self.pane_options.setdefault(pane_id, {})[name] = value

    def set_pane_style(self, pane_id: str, *, border_style=None, active_border_style=None) -> None:
        if border_style:
            self.set_pane_user_option(pane_id, 'pane-border-style', border_style)
        if active_border_style:
            self.set_pane_user_option(pane_id, 'pane-active-border-style', active_border_style)

    def _tmux_run(self, args: list[str], *, check=False, capture=False, input_bytes=None, timeout=None):
        del check, capture, input_bytes, timeout
        self.tmux_calls.append(tuple(args))
        if args[:2] == ['has-session', '-t']:
            return SimpleNamespace(returncode=0 if args[2] in self.sessions else 1, stdout='', stderr='')
        if len(args) >= 7 and args[:2] == ['new-window', '-d']:
            session_name = args[args.index('-t') + 1]
            window_name = args[args.index('-n') + 1]
            self.add_window(session_name, window_name)
            return SimpleNamespace(returncode=0, stdout='', stderr='')
        if len(args) >= 4 and args[:2] == ['list-windows', '-t']:
            session_name = args[2]
            rows = []
            for record in self.sessions.get(session_name, []):
                rows.append(f"{record['id']}\t{record['name']}\t0")
            return SimpleNamespace(returncode=0, stdout='\n'.join(rows), stderr='')
        if len(args) >= 4 and args[:2] == ['list-panes', '-t']:
            target = args[2]
            session_name, _, window_ref = target.partition(':')
            record = self._window(session_name, window_ref)
            panes = list(record['panes']) if record is not None else []
            return SimpleNamespace(returncode=0, stdout='\n'.join(str(item) for item in panes), stderr='')
        if len(args) >= 3 and args[:2] == ['kill-window', '-t']:
            session_name, _, window_ref = args[2].partition(':')
            windows = self.sessions.get(session_name, [])
            removed_panes = [
                pane
                for record in windows
                if record['name'] == window_ref or record['id'] == window_ref
                for pane in record['panes']
            ]
            self.sessions[session_name] = [
                record for record in windows if record['name'] != window_ref and record['id'] != window_ref
            ]
            for pane in removed_panes:
                self.pane_options.pop(str(pane), None)
            return SimpleNamespace(returncode=0, stdout='', stderr='')
        raise AssertionError(f'unexpected tmux command in additive patch test: {args}')

    def _window(self, session_name: str, window_ref: str) -> dict[str, object] | None:
        for record in self.sessions.get(session_name, []):
            if record['name'] == window_ref or record['id'] == window_ref:
                return record
        return None

    def _alloc_pane(self) -> str:
        self.pane_counter += 1
        return f'%{self.pane_counter}'


def test_apply_add_window_creates_only_new_window_sidebar_and_agent_panes(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current', BASE_CONFIG)
    new = _load_config(tmp_path / 'new', ADD_WINDOW_CONFIG)
    project_root = _project(tmp_path / 'repo', BASE_CONFIG)
    layout = PathLayout(project_root)
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(
        layout,
        'proj-1',
        clock=lambda: '2026-05-29T00:00:00Z',
        backend_factory=lambda socket_path=None: backend,
    )
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=controller.load(),
    )

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.created_windows == ('review',)
    assert result.agent_panes == {'agent3': '%4', 'agent4': '%5'}
    assert result.sidebar_panes == {'review': '%3'}
    assert result.preserved_before == {'agent1': '%1', 'agent2': '%2'}
    assert result.preserved_after == {'agent1': '%1', 'agent2': '%2'}
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['runtime_authority_written'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert ('new-window', '-d', '-t', layout.ccbd_tmux_session_name, '-n', 'review') == backend.tmux_calls[1][:6]
    assert all('kill' not in ' '.join(call) for call in backend.tmux_calls)
    assert backend.split_calls == [('%3', 'right', 85), ('%4', 'bottom', 50)]
    assert backend.pane_options['%3']['@ccb_role'] == 'sidebar'
    assert backend.pane_options['%3']['@ccb_slot'] == 'sidebar:review'
    assert backend.pane_options['%3']['@ccb_managed_by'] == 'ccbd'
    assert backend.pane_options['%4']['@ccb_slot'] == 'agent3'
    assert backend.pane_options['%5']['@ccb_slot'] == 'agent4'
    assert {backend.pane_options[pane]['@ccb_window'] for pane in ('%3', '%4', '%5')} == {'review'}
    assert ProjectNamespaceStateStore(layout).load().layout_signature is None


def test_preserved_snapshot_and_assertion_use_fake_identity_data(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-preserved', BASE_CONFIG)
    layout = PathLayout(_project(tmp_path / 'repo-preserved', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    _seed_agent_pane(backend, '%11', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%12', project_id='proj-1', window='main', agent='agent2')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)

    snapshot = snapshot_preserved_agent_panes(
        controller,
        SimpleNamespace(backend=backend),
        topology_plan=build_namespace_topology_plan(current),
        agents=('agent1', 'agent2', 'agent-missing'),
    )

    assert snapshot == {'agent1': '%11', 'agent2': '%12'}
    assert_preserved_agent_panes(snapshot, {'agent1': '%11', 'agent2': '%12'})
    with pytest.raises(RuntimeError, match='changed=agent2'):
        assert_preserved_agent_panes(snapshot, {'agent1': '%11', 'agent2': '%99'})


def test_apply_add_window_failure_returns_partial_without_side_effect_contract(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-fail', BASE_CONFIG)
    new = _load_config(tmp_path / 'new-fail', ADD_WINDOW_CONFIG)
    project_root = _project(tmp_path / 'repo-fail', BASE_CONFIG)
    layout = PathLayout(project_root)

    class _FailingBackend(_PatchFakeBackend):
        def split_pane(self, parent_pane_id: str, direction: str, percent: int, cmd=None, cwd=None) -> str:
            raise RuntimeError('split failed')

    backend = _FailingBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'failed'
    assert result.partial is True
    assert result.created_windows == ('review',)
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['runtime_authority_written'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False
    assert ProjectNamespaceStateStore(layout).load().layout_signature is None


def test_apply_add_window_fails_when_preserved_agent_pane_changes(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-preserve-change', BASE_CONFIG)
    new = _load_config(tmp_path / 'new-preserve-change', ADD_WINDOW_CONFIG)
    project_root = _project(tmp_path / 'repo-preserve-change', BASE_CONFIG)
    layout = PathLayout(project_root)

    class _MovingBackend(_PatchFakeBackend):
        mutate_preserved_on_review_create = False

        def add_window(self, session_name: str, window_name: str) -> str:
            pane_id = super().add_window(session_name, window_name)
            if window_name == 'review' and self.mutate_preserved_on_review_create:
                self.pane_options.pop('%2', None)
                _seed_agent_pane(self, '%99', project_id='proj-1', window='main', agent='agent2')
            return pane_id

    backend = _MovingBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    backend.mutate_preserved_on_review_create = True
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'failed'
    assert result.diagnostics['reason'] == 'preserved_agent_pane_changed'
    assert result.preserved_before == {'agent1': '%1', 'agent2': '%2'}
    assert result.preserved_after == {'agent1': '%1', 'agent2': '%99'}
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['runtime_authority_written'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


def test_apply_append_add_agent_creates_only_new_agent_pane(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-add-agent', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-add-agent',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )
    layout = PathLayout(_project(tmp_path / 'repo-add-agent', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.created_windows == ()
    assert result.created_panes == ('%3',)
    assert result.agent_panes == {'agent3': '%3'}
    assert result.sidebar_panes == {}
    assert result.preserved_before == {'agent1': '%1', 'agent2': '%2'}
    assert result.preserved_after == {'agent1': '%1', 'agent2': '%2'}
    assert backend.split_calls == [('%2', 'right', 50)]
    assert backend.pane_options['%3']['@ccb_project_id'] == 'proj-1'
    assert backend.pane_options['%3']['@ccb_role'] == 'agent'
    assert backend.pane_options['%3']['@ccb_slot'] == 'agent3'
    assert backend.pane_options['%3']['@ccb_window'] == 'main'
    assert backend.pane_options['%3']['@ccb_namespace_epoch'] == '3'
    assert backend.pane_options['%3']['@ccb_managed_by'] == 'ccbd'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['runtime_authority_written'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


def test_apply_remove_agent_kills_only_removed_agent_pane(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-remove-agent', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-remove-agent',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex'),
    )
    layout = PathLayout(_project(tmp_path / 'repo-remove-agent', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_reload_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.removed_agents == {'agent2': '%2'}
    assert result.removed_panes == ('%2',)
    assert result.preserved_before == {'agent1': '%1'}
    assert result.preserved_after == {'agent1': '%1'}
    assert backend.tmux_calls[-1] == ('kill-pane', '-t', '%2')
    assert backend.sessions[layout.ccbd_tmux_session_name][0]['panes'] == ['%1']
    assert '%2' not in backend.pane_options
    assert backend.pane_options['%1']['@ccb_slot'] == 'agent1'


def test_apply_trailing_add_agent_creates_new_agent_pane(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-add-agent-trailing', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-add-agent-trailing',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, agent2:claude, agent3:codex'),
    )
    layout = PathLayout(_project(tmp_path / 'repo-add-agent-trailing', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.created_windows == ()
    assert result.created_panes == ('%3',)
    assert result.agent_panes == {'agent3': '%3'}
    assert backend.split_calls == [('%2', 'bottom', 50)]
    assert backend.pane_options['%3']['@ccb_slot'] == 'agent3'
    assert backend.pane_options['%3']['@ccb_window'] == 'main'


def test_apply_add_tool_window_creates_tool_window_sidebar_and_tool_pane(
    tmp_path: Path,
    monkeypatch,
) -> None:
    current = _load_config(tmp_path / 'current-tool-window', BASE_CONFIG)
    new = _load_config(tmp_path / 'new-tool-window', ADD_TOOL_WINDOW_CONFIG)
    layout = PathLayout(_project(tmp_path / 'repo-tool-window', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.created_windows == ('neovim',)
    assert result.created_panes == ('%3', '%4')
    assert result.sidebar_panes == {'neovim': '%3'}
    assert result.tool_panes == {'neovim': '%4'}
    assert result.agent_panes == {}
    assert backend.respawn_calls[-1] == ('%4', 'ccb-nvim')
    assert backend.pane_options['%3']['@ccb_role'] == 'sidebar'
    assert backend.pane_options['%3']['@ccb_slot'] == 'sidebar:neovim'
    assert backend.pane_options['%4']['@ccb_role'] == 'tool'
    assert backend.pane_options['%4']['@ccb_slot'] == 'tool:neovim'
    assert backend.pane_options['%4']['@ccb_window'] == 'neovim'
    assert result.diagnostics['runtime_authority_written'] is False


def test_apply_remove_tool_window_kills_only_tool_window(
    tmp_path: Path,
    monkeypatch,
) -> None:
    current = _load_config(tmp_path / 'current-tool-remove', ADD_TOOL_WINDOW_CONFIG)
    new = _load_config(tmp_path / 'new-tool-remove', BASE_CONFIG)
    layout = PathLayout(_project(tmp_path / 'repo-tool-remove', ADD_TOOL_WINDOW_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    tool_root = backend.add_window(layout.ccbd_tmux_session_name, 'neovim')
    backend.pane_options[tool_root] = {
        '@ccb_project_id': 'proj-1',
        '@ccb_role': 'tool',
        '@ccb_slot': 'tool:neovim',
        '@ccb_window': 'neovim',
        '@ccb_managed_by': 'ccbd',
    }
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch, allow_kill_window=True)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_reload_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.removed_windows == ('neovim',)
    assert [
        call
        for call in backend.tmux_calls
        if call == ('kill-window', '-t', f'{layout.ccbd_tmux_session_name}:neovim')
    ] == [('kill-window', '-t', f'{layout.ccbd_tmux_session_name}:neovim')]
    assert [record['name'] for record in backend.sessions[layout.ccbd_tmux_session_name]] == ['main']
    assert tool_root not in backend.pane_options
    assert backend.pane_options['%1']['@ccb_slot'] == 'agent1'
    assert backend.pane_options['%2']['@ccb_slot'] == 'agent2'


def test_apply_remove_tool_window_is_idempotent_when_window_already_missing(
    tmp_path: Path,
    monkeypatch,
) -> None:
    current = _load_config(tmp_path / 'current-tool-remove-missing', ADD_TOOL_WINDOW_CONFIG)
    new = _load_config(tmp_path / 'new-tool-remove-missing', BASE_CONFIG)
    layout = PathLayout(_project(tmp_path / 'repo-tool-remove-missing', ADD_TOOL_WINDOW_CONFIG))

    class _MissingWindowBackend(_PatchFakeBackend):
        def _tmux_run(self, args: list[str], *, check=False, capture=False, input_bytes=None, timeout=None):
            if len(args) >= 3 and args[:2] == ['kill-window', '-t']:
                self.tmux_calls.append(tuple(args))
                return SimpleNamespace(returncode=1, stdout='', stderr="can't find window: neovim")
            return super()._tmux_run(
                args,
                check=check,
                capture=capture,
                input_bytes=input_bytes,
                timeout=timeout,
            )

    backend = _MissingWindowBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch, allow_kill_window=True)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_reload_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'applied'
    assert result.removed_windows == ('neovim',)
    assert result.preserved_before == {'agent1': '%1', 'agent2': '%2'}
    assert result.preserved_after == {'agent1': '%1', 'agent2': '%2'}
    assert backend.pane_options['%1']['@ccb_slot'] == 'agent1'
    assert backend.pane_options['%2']['@ccb_slot'] == 'agent2'


def test_apply_append_add_agent_failure_does_not_publish_or_write_authority(tmp_path: Path, monkeypatch) -> None:
    current = _load_config(tmp_path / 'current-add-agent-fail', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-add-agent-fail',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )
    layout = PathLayout(_project(tmp_path / 'repo-add-agent-fail', BASE_CONFIG))

    class _FailingBackend(_PatchFakeBackend):
        def split_pane(self, parent_pane_id: str, direction: str, percent: int, cmd=None, cwd=None) -> str:
            raise RuntimeError('append split failed')

    backend = _FailingBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    backend.sessions[layout.ccbd_tmux_session_name][0]['panes'].append('%2')
    backend.pane_counter = 2
    _seed_agent_pane(backend, '%1', project_id='proj-1', window='main', agent='agent1')
    _seed_agent_pane(backend, '%2', project_id='proj-1', window='main', agent='agent2')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    _forbid_recreate_paths(monkeypatch)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'failed'
    assert result.partial is False
    assert result.diagnostics['reason'] == 'namespace_patch_failed'
    assert result.diagnostics['graph_published'] is False
    assert result.diagnostics['runtime_authority_written'] is False
    assert result.diagnostics['lease_or_lifecycle_written'] is False


@pytest.mark.parametrize(
    ('new_config', 'expected_reason'),
    [
        (
            BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, agent3:codex, agent2:claude'),
            'patch_plan_not_planned',
        ),
        (
            BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex; agent2:claude, agent3:codex'),
            'patch_plan_not_planned',
        ),
        (
            BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent2:claude, agent1:codex, agent3:codex'),
            'patch_plan_not_planned',
        ),
        (
            """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent3:codex"
review = "agent2:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
""",
            'patch_plan_not_planned',
        ),
    ],
)
def test_apply_add_agent_blocks_insert_reorder_move_and_non_last_layouts(
    tmp_path: Path,
    new_config: str,
    expected_reason: str,
) -> None:
    current = _load_config(tmp_path / 'current-non-append', BASE_CONFIG)
    new = _load_config(tmp_path / 'new-non-append', new_config)
    layout = PathLayout(_project(tmp_path / 'repo-non-append', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())

    result = controller.apply_additive_patch(
        patch_plan=plan['namespace_patch_plan'],
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == expected_reason
    assert backend.split_calls == []


def test_apply_additive_patch_requires_step_identity_proofs(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-proof', BASE_CONFIG)
    new = _load_config(tmp_path / 'new-proof', ADD_WINDOW_CONFIG)
    layout = PathLayout(_project(tmp_path / 'repo-proof', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())
    patch_plan = dict(plan['namespace_patch_plan'])
    patch_plan['steps'] = [
        {key: value for key, value in step.items() if key != 'managed_by'}
        for step in patch_plan['steps']
    ]

    result = controller.apply_additive_patch(
        patch_plan=patch_plan,
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'scope_proof_missing'
    assert backend.split_calls == []


def test_apply_additive_patch_rejects_patch_plan_topology_mismatch(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-mismatch', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-mismatch',
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
review = "agent3:codex"
qa = "agent4:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
""",
    )
    layout = PathLayout(_project(tmp_path / 'repo-mismatch', BASE_CONFIG))
    backend = _PatchFakeBackend(socket_path=str(layout.ccbd_tmux_socket_path))
    backend.add_window(layout.ccbd_tmux_session_name, 'main')
    _store_namespace(layout, project_id='proj-1')
    controller = ProjectNamespaceController(layout, 'proj-1', backend_factory=lambda socket_path=None: backend)
    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=controller.load())
    patch_plan = dict(plan['namespace_patch_plan'])
    patch_plan['steps'] = [
        step for step in patch_plan['steps']
        if not (isinstance(step, dict) and step.get('window') == 'qa')
    ]

    result = controller.apply_additive_patch(
        patch_plan=patch_plan,
        old_topology=build_namespace_topology_plan(current),
        new_topology=build_namespace_topology_plan(new),
        timeout_s=0.0,
    )

    assert result.status == 'blocked'
    assert result.diagnostics['reason'] == 'patch_plan_mismatch'
    assert backend.split_calls == []


def test_project_reload_non_dry_run_no_change_noops_after_patch_api(tmp_path: Path) -> None:
    project_root = _project(tmp_path / 'repo-block-no-change', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: '2026-05-29T00:00:00Z', pid=4242)
    old_graph = app.service_graph

    payload = app.socket_server._handlers['project_reload_config']({'dry_run': False})

    assert payload['status'] == 'noop'
    assert payload['stage'] == 'no_op'
    assert payload['plan_class'] == 'no_change'
    assert payload['diagnostics']['graph_published'] is False
    assert app.service_graph is old_graph
    assert app.control_plane_metrics.last_reload_duration_s is not None
    assert app.control_plane_metrics.last_reload_error is None


def _forbid_recreate_paths(monkeypatch, *, allow_kill_window: bool = False) -> None:
    monkeypatch.setattr(
        'ccbd.services.project_namespace_runtime.ensure.ensure_project_namespace',
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('must not call full ensure')),
        raising=False,
    )
    forbidden_backend_mutations = ('kill_server',) if allow_kill_window else ('kill_server', 'kill_window')
    for name in forbidden_backend_mutations:
        monkeypatch.setattr(
            f'ccbd.services.project_namespace_runtime.backend.{name}',
            lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError(f'must not call {name}')),
            raising=False,
        )
    monkeypatch.setattr(
        'ccbd.services.project_namespace_runtime.ensure_state.force_recreate_namespace',
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('must not force recreate namespace')),
        raising=False,
    )
    monkeypatch.setattr(
        'ccbd.services.project_namespace_runtime.reflow.reflow_project_workspace',
        lambda *_args, **_kwargs: (_ for _ in ()).throw(AssertionError('must not reflow workspace')),
        raising=False,
    )


def _seed_agent_pane(backend: _PatchFakeBackend, pane_id: str, *, project_id: str, window: str, agent: str) -> None:
    backend.pane_options[pane_id] = {
        '@ccb_project_id': project_id,
        '@ccb_role': 'agent',
        '@ccb_slot': agent,
        '@ccb_window': window,
        '@ccb_managed_by': 'ccbd',
    }


def _store_namespace(layout: PathLayout, *, project_id: str) -> None:
    ProjectNamespaceStateStore(layout).save(
        ProjectNamespaceState(
            project_id=project_id,
            namespace_epoch=3,
            tmux_socket_path=str(layout.ccbd_tmux_socket_path),
            tmux_session_name=layout.ccbd_tmux_session_name,
            layout_version=3,
            layout_signature=None,
            control_window_name=layout.ccbd_tmux_control_window_name,
            control_window_id='@control',
            workspace_window_name='main',
            workspace_window_id='@main',
            workspace_epoch=1,
            ui_attachable=True,
            last_started_at='2026-05-29T00:00:00Z',
        )
    )


def _project(project_root: Path, config_text: str) -> Path:
    project_root.mkdir(parents=True, exist_ok=True)
    config_path = project_root / '.ccb' / 'ccb.config'
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(config_text, encoding='utf-8')
    return project_root


def _load_config(project_root: Path, config_text: str):
    return load_project_config(_project(project_root, config_text)).config
