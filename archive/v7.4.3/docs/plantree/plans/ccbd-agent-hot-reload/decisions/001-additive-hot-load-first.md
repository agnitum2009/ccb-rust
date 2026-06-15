# Additive Hot Load First

Date: 2026-05-28

## Context

The requested behavior is to load new agents without breaking agents that are
already running. Current startup and namespace code can recreate a topology,
but recreation is too disruptive for this goal.

## Decision

The first supported hot reload class is additive-only:

- add a new agent to an existing managed window;
- add a new managed window with new agents;
- apply view-only sidebar presentation changes.

Changes that delete, rename, move, or replace existing running agents are
classified as `unsafe_requires_restart` and are rejected by the reload command
without killing panes.

## Consequences

This gives a safe implementation boundary and a clear test oracle: unchanged
agent pane ids must remain unchanged. More advanced replacement and deletion
semantics can be added later with explicit pending-restart or pending-removal
states.

Superseding note, 2026-05-29: the broader plan now includes dynamic unload and
replace, but additive load remains the first mutating class to expose. Deletion
and replacement are planned only after dry-run, service-graph routing, bounded
draining, and performance gates are in place.

Related topic:
[non-disruptive-hot-load-design.md](../topics/non-disruptive-hot-load-design.md).
