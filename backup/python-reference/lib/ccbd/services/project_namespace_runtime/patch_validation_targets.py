from __future__ import annotations

from ccbd.reload_additive_agents import window_agent_names


def removed_agent_targets(old_topology, new_topology) -> set[tuple[str, str]]:
    new_agents = _topology_agent_pairs(new_topology)
    return _topology_agent_pairs(old_topology) - new_agents


def _topology_agent_pairs(topology) -> set[tuple[str, str]]:
    return {
        (str(window.name), str(agent_name))
        for window in tuple(getattr(topology, 'windows', ()) or ())
        for agent_name in window_agent_names(window)
    }


__all__ = ['removed_agent_targets']
