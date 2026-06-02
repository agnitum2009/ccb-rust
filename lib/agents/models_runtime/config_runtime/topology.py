from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any

from agents.models_runtime.layout import LayoutNode, parse_layout_spec, prune_layout

from ..names import AgentValidationError, normalize_agent_name


_WINDOW_NAME_RE = re.compile(r'^[A-Za-z][A-Za-z0-9_-]*$')
SIDEBAR_MODE_EVERY_WINDOW = 'every_window'
SIDEBAR_MODE_OFF = 'off'
DEFAULT_SIDEBAR_VIEW_TIPS = (
    'C-b d  detach',
    'C-b h/j/k/l pane',
    'C-b H/J/K/L resize',
    'C-b o  next pane',
    'C-b z  zoom',
    'C-b w  tree',
    'C-b n/p next/prev',
    'C-b 0-9 jump win',
    'C-b [  copy mode',
    'copy: PgUp/PgDn',
    'copy: v select',
    'copy: y yank',
    'copy: q exit',
    'C-b ]  paste',
    'C-b c  new win',
    'C-b ,  rename',
    'C-b ?  keys',
)


@dataclass(frozen=True)
class SidebarSpec:
    mode: str = SIDEBAR_MODE_EVERY_WINDOW
    width: str | int = '15%'
    bottom_height: int = 20

    def __post_init__(self) -> None:
        mode = str(self.mode or '').strip()
        if mode not in {SIDEBAR_MODE_EVERY_WINDOW, SIDEBAR_MODE_OFF}:
            raise AgentValidationError('ui.sidebar.mode must be every_window or off')
        object.__setattr__(self, 'mode', mode)
        object.__setattr__(self, 'width', normalize_sidebar_width(self.width))
        try:
            bottom_height = int(self.bottom_height)
        except Exception as exc:
            raise AgentValidationError('ui.sidebar.bottom_height must be a non-negative integer') from exc
        if bottom_height < 0:
            raise AgentValidationError('ui.sidebar.bottom_height must be a non-negative integer')
        object.__setattr__(self, 'bottom_height', bottom_height)

    def to_record(self) -> dict[str, object]:
        return {
            'mode': self.mode,
            'width': self.width,
            'bottom_height': self.bottom_height,
        }


@dataclass(frozen=True)
class SidebarViewSpec:
    agents_height: str | int = '50%'
    comms_height: str | int = '15%'
    tips_height: str | int = '35%'
    comms_limit: int = 5
    comms_compact: bool = True
    tips_enabled: bool = True
    tips: tuple[str, ...] = DEFAULT_SIDEBAR_VIEW_TIPS

    def __post_init__(self) -> None:
        object.__setattr__(
            self,
            'agents_height',
            normalize_sidebar_view_height(
                self.agents_height,
                field_name='ui.sidebar.view.agents_height',
            ),
        )
        object.__setattr__(
            self,
            'comms_height',
            normalize_sidebar_view_height(
                self.comms_height,
                field_name='ui.sidebar.view.comms_height',
            ),
        )
        object.__setattr__(
            self,
            'tips_height',
            normalize_sidebar_view_height(
                self.tips_height,
                field_name='ui.sidebar.view.tips_height',
            ),
        )
        try:
            comms_limit = int(self.comms_limit)
        except Exception as exc:
            raise AgentValidationError('ui.sidebar.view.comms_limit must be a positive integer') from exc
        if comms_limit <= 0:
            raise AgentValidationError('ui.sidebar.view.comms_limit must be a positive integer')
        object.__setattr__(self, 'comms_limit', comms_limit)
        if not isinstance(self.comms_compact, bool):
            raise AgentValidationError('ui.sidebar.view.comms_compact must be a boolean')
        if not isinstance(self.tips_enabled, bool):
            raise AgentValidationError('ui.sidebar.view.tips_enabled must be a boolean')
        object.__setattr__(self, 'comms_compact', self.comms_compact)
        object.__setattr__(self, 'tips_enabled', self.tips_enabled)
        tips = tuple(_normalize_tip(item) for item in self.tips)
        object.__setattr__(self, 'tips', tips or DEFAULT_SIDEBAR_VIEW_TIPS)

    def to_record(self) -> dict[str, object]:
        return {
            'agents_height': self.agents_height,
            'comms_height': self.comms_height,
            'tips_height': self.tips_height,
            'comms_limit': self.comms_limit,
            'comms_compact': self.comms_compact,
            'tips_enabled': self.tips_enabled,
            'tips': list(self.tips),
        }


@dataclass(frozen=True)
class WindowSpec:
    name: str
    order: int
    layout_spec: str
    agent_names: tuple[str, ...]

    def __post_init__(self) -> None:
        name = validate_window_name(self.name)
        try:
            order = int(self.order)
        except Exception as exc:
            raise AgentValidationError('window order must be an integer') from exc
        layout_spec = str(self.layout_spec or '').strip()
        if not layout_spec:
            raise AgentValidationError(f'windows.{name} layout cannot be empty')
        agent_names = tuple(normalize_agent_name(item) for item in self.agent_names)
        if not agent_names:
            raise AgentValidationError(f'windows.{name} must contain at least one agent')
        if len(set(agent_names)) != len(agent_names):
            raise AgentValidationError(f'windows.{name} cannot contain duplicate agents')
        object.__setattr__(self, 'name', name)
        object.__setattr__(self, 'order', order)
        object.__setattr__(self, 'layout_spec', layout_spec)
        object.__setattr__(self, 'agent_names', agent_names)

    def to_record(self) -> dict[str, object]:
        return {
            'name': self.name,
            'order': self.order,
            'layout_spec': self.layout_spec,
            'agent_names': list(self.agent_names),
        }


@dataclass(frozen=True)
class ToolWindowSpec:
    name: str
    order: int
    command: str
    label: str | None = None
    show_in_sidebar: bool = True

    def __post_init__(self) -> None:
        name = validate_window_name(self.name)
        try:
            order = int(self.order)
        except Exception as exc:
            raise AgentValidationError('tool window order must be an integer') from exc
        command = str(self.command or '').strip()
        if not command:
            raise AgentValidationError(f'tool_windows.{name}.command cannot be empty')
        label = str(self.label or '').strip() or name
        if not isinstance(self.show_in_sidebar, bool):
            raise AgentValidationError(f'tool_windows.{name}.show_in_sidebar must be a boolean')
        object.__setattr__(self, 'name', name)
        object.__setattr__(self, 'order', order)
        object.__setattr__(self, 'command', command)
        object.__setattr__(self, 'label', label)
        object.__setattr__(self, 'show_in_sidebar', self.show_in_sidebar)

    def to_record(self) -> dict[str, object]:
        return {
            'name': self.name,
            'order': self.order,
            'command': self.command,
            'label': self.label,
            'show_in_sidebar': self.show_in_sidebar,
        }


def default_sidebar_spec() -> SidebarSpec:
    return SidebarSpec()


def default_sidebar_view_spec() -> SidebarViewSpec:
    return SidebarViewSpec()


def validate_window_name(value: str) -> str:
    name = str(value or '').strip()
    if not _WINDOW_NAME_RE.fullmatch(name):
        raise AgentValidationError(
            f'invalid window name {name!r}; expected ^[A-Za-z][A-Za-z0-9_-]*$'
        )
    return name


def normalize_sidebar_width(value: str | int) -> str | int:
    if isinstance(value, bool):
        raise AgentValidationError('ui.sidebar.width must be a positive integer or percentage string')
    if isinstance(value, int):
        if value <= 0:
            raise AgentValidationError('ui.sidebar.width must be positive')
        return value
    text = str(value or '').strip()
    if text.endswith('%'):
        number = text[:-1].strip()
        if not number.isdigit() or int(number) <= 0:
            raise AgentValidationError('ui.sidebar.width percentage must be positive')
        return f'{int(number)}%'
    if text.isdigit() and int(text) > 0:
        return int(text)
    raise AgentValidationError('ui.sidebar.width must be a positive integer or percentage string')


def normalize_sidebar_view_height(
    value: str | int,
    *,
    field_name: str = 'ui.sidebar.view.agents_height',
) -> str | int:
    if isinstance(value, bool):
        raise AgentValidationError(f'{field_name} must be a positive integer or percentage string')
    if isinstance(value, int):
        if value <= 0:
            raise AgentValidationError(f'{field_name} must be positive')
        return value
    text = str(value or '').strip()
    if text.endswith('%'):
        number = text[:-1].strip()
        if not number.isdigit() or int(number) <= 0 or int(number) >= 100:
            raise AgentValidationError(f'{field_name} percentage must be between 1% and 99%')
        return f'{int(number)}%'
    if text.isdigit() and int(text) > 0:
        return int(text)
    raise AgentValidationError(f'{field_name} must be a positive integer or percentage string')


def _normalize_tip(value: object) -> str:
    text = str(value or '').strip()
    if not text:
        raise AgentValidationError('ui.sidebar.view.tips cannot contain empty strings')
    return text


def normalize_windows(
    windows: tuple[WindowSpec, ...] | None,
    *,
    layout_spec: str,
    default_agents: tuple[str, ...],
) -> tuple[WindowSpec, ...]:
    if windows:
        return _validate_windows(windows)
    return (legacy_main_window(layout_spec=layout_spec, default_agents=default_agents),)


def legacy_main_window(*, layout_spec: str, default_agents: tuple[str, ...]) -> WindowSpec:
    layout = parse_layout_spec(layout_spec)
    pruned = prune_layout(layout, include_names=default_agents)
    if pruned is None:
        raise AgentValidationError('legacy layout does not contain any configured agents')
    leaf_names = tuple(normalize_agent_name(leaf.name) for leaf in pruned.iter_leaves())
    return WindowSpec(
        name='main',
        order=0,
        layout_spec=pruned.render(),
        agent_names=leaf_names,
    )


def _validate_windows(windows: tuple[WindowSpec, ...]) -> tuple[WindowSpec, ...]:
    if not windows:
        raise AgentValidationError('at least one window must be configured')
    seen_windows: set[str] = set()
    seen_agents: set[str] = set()
    normalized: list[WindowSpec] = []
    for index, window in enumerate(windows):
        spec = WindowSpec(
            name=window.name,
            order=index,
            layout_spec=window.layout_spec,
            agent_names=window.agent_names,
        )
        if spec.name in seen_windows:
            raise AgentValidationError(f'duplicate window name: {spec.name}')
        seen_windows.add(spec.name)
        duplicates = [name for name in spec.agent_names if name in seen_agents]
        if duplicates:
            raise AgentValidationError(f'duplicate agent across windows: {duplicates[0]}')
        seen_agents.update(spec.agent_names)
        normalized.append(spec)
    return tuple(normalized)


def normalize_tool_windows(tool_windows: tuple[ToolWindowSpec, ...] | None) -> tuple[ToolWindowSpec, ...]:
    normalized: list[ToolWindowSpec] = []
    seen: set[str] = set()
    for index, tool in enumerate(tuple(tool_windows or ())):
        spec = ToolWindowSpec(
            name=tool.name,
            order=index,
            command=tool.command,
            label=tool.label,
            show_in_sidebar=tool.show_in_sidebar,
        )
        if spec.name in seen:
            raise AgentValidationError(f'duplicate tool window name: {spec.name}')
        seen.add(spec.name)
        normalized.append(spec)
    return tuple(normalized)


def validate_entry_window(
    entry_window: str | None,
    *,
    windows: tuple[WindowSpec, ...],
    tool_windows: tuple[ToolWindowSpec, ...] = (),
) -> str:
    if not windows:
        raise AgentValidationError('at least one window must be configured')
    value = str(entry_window or '').strip() or windows[0].name
    value = validate_window_name(value)
    names = {window.name for window in windows}
    names.update(tool.name for tool in tool_windows)
    if value not in names:
        raise AgentValidationError(f'entry_window references unknown window: {value}')
    return value


def validate_tool_windows_do_not_conflict(
    windows: tuple[WindowSpec, ...],
    tool_windows: tuple[ToolWindowSpec, ...],
) -> None:
    agent_window_names = {window.name for window in windows}
    for tool in tool_windows:
        if tool.name in agent_window_names:
            raise AgentValidationError(f'tool window conflicts with agent window: {tool.name}')


def validate_windows_reference_agents(
    windows: tuple[WindowSpec, ...],
    *,
    normalized_agents: dict[str, object],
) -> None:
    configured = set(normalized_agents)
    referenced: list[str] = []
    for window in windows:
        referenced.extend(window.agent_names)
    missing = [name for name in referenced if name not in configured]
    if missing:
        raise AgentValidationError(f'windows reference unknown agents: {missing}')
    unused = [name for name in configured if name not in set(referenced)]
    if unused:
        raise AgentValidationError(f'configured agents missing from windows: {unused}')


def topology_signature_payload(
    *,
    windows: tuple[WindowSpec, ...],
    tool_windows: tuple[ToolWindowSpec, ...],
    entry_window: str,
    sidebar: SidebarSpec,
) -> dict[str, object]:
    return {
        'version': 1,
        'windows': [
            {
                'name': window.name,
                'order': window.order,
                'layout': window.layout_spec,
                'agents': list(window.agent_names),
            }
            for window in windows
        ],
        'tool_windows': [
            {
                'name': tool.name,
                'order': tool.order,
                'command': tool.command,
            }
            for tool in tool_windows
        ],
        'entry_window': entry_window,
        'sidebar': sidebar.to_record(),
    }


def topology_signature(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8')
    return hashlib.sha256(encoded).hexdigest()


__all__ = [
    'DEFAULT_SIDEBAR_VIEW_TIPS',
    'SIDEBAR_MODE_EVERY_WINDOW',
    'SIDEBAR_MODE_OFF',
    'SidebarSpec',
    'SidebarViewSpec',
    'ToolWindowSpec',
    'WindowSpec',
    'default_sidebar_spec',
    'default_sidebar_view_spec',
    'legacy_main_window',
    'normalize_sidebar_width',
    'normalize_sidebar_view_height',
    'normalize_tool_windows',
    'normalize_windows',
    'topology_signature',
    'topology_signature_payload',
    'validate_entry_window',
    'validate_tool_windows_do_not_conflict',
    'validate_window_name',
    'validate_windows_reference_agents',
]
