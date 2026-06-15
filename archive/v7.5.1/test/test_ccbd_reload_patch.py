from __future__ import annotations

from pathlib import Path
from types import SimpleNamespace

import pytest

from agents.config_loader import load_project_config
from ccbd.app import CcbdApp
from ccbd.reload_patch import build_namespace_patch_plan
from ccbd.reload_plan import build_reload_dry_run_plan
from cli.context import CliContext
from cli.models import ParsedReloadCommand
from cli.parser import CliParser
from cli.services.reload import reload_config
from project.resolver import bootstrap_project
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


def test_namespace_patch_plan_add_agent_append_preserves_existing_agents(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )
    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'add_agent'
    assert patch['status'] == 'planned'
    assert patch['mutation_enabled'] is False
    assert patch['apply_deferred'] is True
    assert patch['scope']['verified'] is True
    assert patch['preserved_agents'] == ['agent1', 'agent2']
    assert patch['blocked_operations'] == []
    assert patch['steps'] == [
        {
            'action': 'create_agent_pane',
            'window': 'main',
            'agent': 'agent3',
            'role': 'agent',
            'slot_key': 'agent3',
            'managed_by': 'ccbd',
            'anchor_agent': 'agent2',
            'reason': 'new agent appended to existing managed window',
        }
    ]


def test_namespace_patch_plan_add_agent_trailing_vertical_append(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-trailing', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-trailing',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, agent2:claude, agent3:codex'),
    )

    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'add_agent'
    assert patch['status'] == 'planned'
    assert patch['blocked_operations'] == []
    assert patch['steps'] == [
        {
            'action': 'create_agent_pane',
            'window': 'main',
            'agent': 'agent3',
            'role': 'agent',
            'slot_key': 'agent3',
            'managed_by': 'ccbd',
            'anchor_agent': 'agent2',
            'reason': 'new agent appended to existing managed window',
        }
    ]


@pytest.mark.parametrize(
    'layout',
        [
            'agent1:codex; agent2:claude, agent3:codex',
        ],
    )
def test_namespace_patch_plan_blocks_add_agent_layouts_that_do_not_expand_last_pane(
    tmp_path: Path,
    layout: str,
) -> None:
    current = _load_config(tmp_path / 'current-add-agent-layout', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-add-agent-layout',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', layout),
    )

    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    assert plan['plan_class'] == 'add_agent'
    assert plan['namespace_patch_plan']['status'] == 'blocked'
    assert {'add_agent'} == {item['op'] for item in plan['namespace_patch_plan']['blocked_operations']}
    assert plan['namespace_patch_plan']['steps'] == []


def test_namespace_patch_plan_add_window_creates_window_sidebar_and_agent_panes(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-window', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-window',
        """version = 2
entry_window = "main"

[windows]
main = "agent1:codex, agent2:claude"
review = "agent3:codex"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
""",
    )
    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'add_window'
    assert patch['status'] == 'planned'
    assert patch['preserved_agents'] == ['agent1', 'agent2']
    assert [step['action'] for step in patch['steps']] == [
        'create_window',
        'create_sidebar_pane',
        'create_agent_pane',
    ]
    assert patch['steps'][0]['window'] == 'review'
    assert {step['managed_by'] for step in patch['steps']} == {'ccbd'}
    assert patch['steps'][1]['slot_key'] == 'sidebar:review'
    assert patch['steps'][2]['agent'] == 'agent3'


def test_namespace_patch_plan_add_tool_window_creates_sidebar_and_tool_pane(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-tool-window', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-tool-window',
        BASE_CONFIG
        + """
[tool_windows.neovim]
command = "ccb-nvim"
label = "neovim"
""",
    )
    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'add_tool_window'
    assert patch['status'] == 'planned'
    assert patch['preserved_agents'] == ['agent1', 'agent2']
    assert [step['action'] for step in patch['steps']] == [
        'create_window',
        'create_sidebar_pane',
        'create_tool_pane',
    ]
    assert patch['steps'][0]['window'] == 'neovim'
    assert patch['steps'][1]['slot_key'] == 'sidebar:neovim'
    assert patch['steps'][2]['slot_key'] == 'tool:neovim'


def test_namespace_patch_plan_remove_tool_window_kills_tool_window(tmp_path: Path) -> None:
    current = _load_config(
        tmp_path / 'current-tool-remove',
        BASE_CONFIG
        + """
[tool_windows.neovim]
command = "ccb-nvim"
label = "neovim"
""",
    )
    new = _load_config(tmp_path / 'new-tool-remove', BASE_CONFIG)
    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'remove_tool_window'
    assert patch['status'] == 'planned'
    assert patch['preserved_agents'] == ['agent1', 'agent2']
    assert patch['steps'] == [
        {
            'action': 'kill_tool_window',
            'window': 'neovim',
            'role': 'tool',
            'slot_key': 'tool:neovim',
            'managed_by': 'ccbd',
            'reason': 'managed tool window exists only in current published config',
        }
    ]


def test_namespace_patch_plan_view_only_has_no_tmux_mutation_step(tmp_path: Path) -> None:
    current = _load_config(
        tmp_path / 'current-view',
        BASE_CONFIG
        + """
[ui.sidebar.view]
agents_height = "50%"
comms_height = "15%"
tips_height = "35%"
comms_limit = 4
""",
    )
    new = _load_config(
        tmp_path / 'new-view',
        BASE_CONFIG
        + """
[ui.sidebar.view]
agents_height = "60%"
comms_height = "10%"
tips_height = "30%"
comms_limit = 5
""",
    )

    plan = build_reload_dry_run_plan(current, new)

    assert plan['plan_class'] == 'view_only_change'
    assert plan['namespace_patch_plan']['status'] == 'planned'
    assert plan['namespace_patch_plan']['steps'] == [
        {
            'action': 'refresh_project_view',
            'managed_by': 'ccbd',
            'reason': 'presentation-only config changed; no tmux namespace mutation is required',
        }
    ]


@pytest.mark.parametrize(
    ('new_text', 'expected_class', 'blocked_ops'),
    [
        (BASE_CONFIG.replace('agent2:claude', 'agent2:codex'), 'replace_agent', {'replace_agent'}),
        (
            BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent2:claude, agent1:codex'),
            'layout_change',
            {'layout_change'},
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
            'move_agent',
            {'move_agent'},
        ),
    ],
)
def test_namespace_patch_plan_blocks_non_additive_mutations(
    tmp_path: Path,
    new_text: str,
    expected_class: str,
    blocked_ops: set[str],
) -> None:
    current = _load_config(tmp_path / f'current-{expected_class}', BASE_CONFIG)
    new = _load_config(tmp_path / f'new-{expected_class}', new_text)

    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    assert plan['plan_class'] == expected_class
    assert plan['namespace_patch_plan']['status'] == 'blocked'
    assert blocked_ops <= {item['op'] for item in plan['namespace_patch_plan']['blocked_operations']}
    assert plan['safe_to_apply'] is False
    assert plan['mutation_enabled'] is False


def test_namespace_patch_plan_remove_agent_kills_only_removed_agent_pane(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-remove', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-remove',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex'),
    )

    plan = build_reload_dry_run_plan(
        current,
        new,
        project_id='proj-1',
        current_namespace=_namespace('proj-1'),
    )

    patch = plan['namespace_patch_plan']

    assert plan['plan_class'] == 'remove_agent'
    assert plan['future_safe_to_apply'] is True
    assert patch['status'] == 'planned'
    assert patch['preserved_agents'] == ['agent1']
    assert patch['blocked_operations'] == []
    assert patch['steps'] == [
        {
            'action': 'kill_agent_pane',
            'window': 'main',
            'agent': 'agent2',
            'role': 'agent',
            'slot_key': 'agent2',
            'managed_by': 'ccbd',
            'reason': 'agent exists only in current published config',
        }
    ]


def test_namespace_patch_plan_blocks_additive_when_namespace_scope_unverified(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-no-scope', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-no-scope',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )

    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=None)

    assert plan['namespace_patch_plan']['status'] == 'blocked'
    assert {'namespace_scope'} == {item['op'] for item in plan['namespace_patch_plan']['blocked_operations']}
    assert plan['namespace_patch_plan']['steps'] == []


def test_namespace_patch_plan_requires_namespace_epoch_for_additive_plan(tmp_path: Path) -> None:
    current = _load_config(tmp_path / 'current-no-epoch', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-no-epoch',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )
    namespace = _namespace('proj-1')
    namespace.namespace_epoch = None

    plan = build_reload_dry_run_plan(current, new, project_id='proj-1', current_namespace=namespace)

    assert plan['namespace_patch_plan']['status'] == 'blocked'
    assert plan['namespace_patch_plan']['scope']['verified'] is False
    assert {'namespace_scope'} == {item['op'] for item in plan['namespace_patch_plan']['blocked_operations']}


def test_namespace_patch_planner_does_not_touch_old_runtime_or_tmux(tmp_path: Path, monkeypatch) -> None:
    project_root = _project(tmp_path / 'repo-no-mutation', BASE_CONFIG)
    app = CcbdApp(project_root, clock=lambda: '2026-05-29T00:00:00Z', pid=4242)
    current = _load_config(tmp_path / 'current-app', BASE_CONFIG)
    new = _load_config(
        tmp_path / 'new-app',
        BASE_CONFIG.replace('agent1:codex, agent2:claude', 'agent1:codex, (agent2:claude; agent3:codex)'),
    )
    before_snapshot = _runtime_file_snapshot(project_root)
    before_graph = app.service_graph

    def _fail(*_args, **_kwargs):
        raise AssertionError('namespace patch planning must not mutate tmux, graph, namespace, or runtime authority')

    monkeypatch.setattr(app, 'publish_service_graph', _fail, raising=False)
    monkeypatch.setattr(app.runtime_service, 'mutate_runtime_authority', _fail, raising=False)
    monkeypatch.setattr(app.runtime_service, 'patch_runtime_state', _fail, raising=False)
    for method_name in ('ensure', 'destroy', 'reflow_workspace'):
        monkeypatch.setattr(app.project_namespace, method_name, _fail, raising=False)

    plan = build_namespace_patch_plan(
        current,
        new,
        [{'op': 'add_agent', 'agent': 'agent3', 'window': 'main'}],
        project_id=app.project_id,
        current_namespace=_namespace(app.project_id),
    )

    assert plan['status'] == 'planned'
    assert plan['steps'][0]['action'] == 'create_agent_pane'
    assert app.service_graph is before_graph
    assert _runtime_file_snapshot(project_root) == before_snapshot


def test_project_reload_non_dry_run_no_change_noops_without_publish(tmp_path: Path) -> None:
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
    assert app.control_plane_metrics.last_reload_plan_class == 'no_change'
    assert app.control_plane_metrics.last_reload_error is None


def test_cli_reload_non_dry_run_calls_socket_with_dry_run_false(tmp_path: Path, monkeypatch) -> None:
    parser = CliParser()
    assert parser.parse(['reload']) == ParsedReloadCommand(project=None, dry_run=False)

    project_root = _project(tmp_path / 'repo-cli-reject', BASE_CONFIG)
    command = ParsedReloadCommand(project=None, dry_run=False)
    context = CliContext(
        command=command,
        cwd=project_root,
        project=bootstrap_project(project_root),
        paths=PathLayout(project_root),
    )

    import cli.services.reload as reload_module

    calls: list[bool] = []

    class _Client:
        def project_reload_config(self, *, dry_run: bool) -> dict:
            calls.append(dry_run)
            return {'status': 'blocked', 'dry_run': dry_run}

    monkeypatch.setattr(
        reload_module,
        'connect_current_mounted_daemon',
        lambda _context: SimpleNamespace(client=_Client()),
    )
    payload = reload_config(context, command)

    assert calls == [False]
    assert payload == {'status': 'blocked', 'dry_run': False}


def _namespace(project_id: str):
    return SimpleNamespace(
        project_id=project_id,
        namespace_epoch=7,
        tmux_socket_path='/tmp/ccb-tmux.sock',
        tmux_session_name='ccb-test',
        ui_attachable=True,
    )


def _project(project_root: Path, config_text: str) -> Path:
    project_root.mkdir(parents=True, exist_ok=True)
    config_path = project_root / '.ccb' / 'ccb.config'
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(config_text, encoding='utf-8')
    return project_root


def _load_config(project_root: Path, config_text: str):
    return load_project_config(_project(project_root, config_text)).config


def _runtime_file_snapshot(project_root: Path) -> dict[str, bytes]:
    root = project_root / '.ccb' / 'ccbd'
    if not root.exists():
        return {}
    return {
        str(path.relative_to(root)): path.read_bytes()
        for path in sorted(root.rglob('*'))
        if path.is_file()
    }
