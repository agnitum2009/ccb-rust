from __future__ import annotations

from ccbd.reload_additive_agents import append_agent_windows, window_agent_names, window_map
from terminal_runtime.tmux_identity import apply_ccb_pane_identity

from .backend import split_pane


def append_agent_panes(
    controller,
    backend,
    *,
    old_topology,
    new_topology,
    existing_agent_panes: dict[str, str],
    namespace_epoch: int,
    created_panes: list[str],
    timeout_s: float | None,
) -> dict[str, str]:
    append_windows = append_agent_windows(old_topology, new_topology) or {}
    old_windows = window_map(old_topology)
    new_windows = window_map(new_topology)
    agent_panes: dict[str, str] = {}
    for window_name, appended_agents in append_windows.items():
        agent_panes.update(
            _append_window_agent_panes(
                controller,
                backend,
                window_name=window_name,
                old_window=old_windows[window_name],
                new_window=new_windows[window_name],
                appended_agents=appended_agents,
                existing_agent_panes=existing_agent_panes,
                namespace_epoch=namespace_epoch,
                created_panes=created_panes,
                timeout_s=timeout_s,
            )
        )
    return agent_panes


def _append_window_agent_panes(
    controller,
    backend,
    *,
    window_name: str,
    old_window,
    new_window,
    appended_agents,
    existing_agent_panes: dict[str, str],
    namespace_epoch: int,
    created_panes: list[str],
    timeout_s: float | None,
) -> dict[str, str]:
    old_agents = window_agent_names(old_window)
    if not old_agents:
        raise RuntimeError(f'cannot append agents to empty existing window {window_name!r}')
    target = _anchor_pane(existing_agent_panes, old_agents[-1])
    style_index_by_agent = {agent: index for index, agent in enumerate(window_agent_names(new_window))}
    agent_panes: dict[str, str] = {}
    for appended in appended_agents:
        target = _append_single_agent_pane(
            controller,
            backend,
            appended=appended,
            target=target,
            window_name=window_name,
            order_index=style_index_by_agent.get(appended.agent),
            namespace_epoch=namespace_epoch,
            created_panes=created_panes,
            timeout_s=timeout_s,
        )
        agent_panes[appended.agent] = target
    return agent_panes


def _anchor_pane(existing_agent_panes: dict[str, str], anchor_agent: str) -> str:
    pane_id = existing_agent_panes.get(anchor_agent)
    if not pane_id:
        raise RuntimeError(f'anchor pane missing for preserved agent {anchor_agent!r}')
    return pane_id


def _append_single_agent_pane(
    controller,
    backend,
    *,
    appended,
    target: str,
    window_name: str,
    order_index: int | None,
    namespace_epoch: int,
    created_panes: list[str],
    timeout_s: float | None,
) -> str:
    pane_id = split_pane(
        backend,
        target=target,
        direction=appended.direction,
        percent=50,
        project_root=controller._layout.project_root,
        timeout_s=timeout_s,
    )
    _append_unique(created_panes, pane_id)
    apply_ccb_pane_identity(
        backend,
        pane_id,
        title=appended.agent,
        agent_label=appended.agent,
        project_id=controller._project_id,
        order_index=order_index,
        role='agent',
        slot_key=appended.agent,
        window_name=window_name,
        namespace_epoch=namespace_epoch,
        managed_by='ccbd',
    )
    return pane_id


def _append_unique(values: list[str], value: str) -> None:
    if value and value not in values:
        values.append(value)


__all__ = ['append_agent_panes']
