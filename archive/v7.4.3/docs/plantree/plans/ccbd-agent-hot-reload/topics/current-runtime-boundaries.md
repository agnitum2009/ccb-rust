# Current Runtime Boundaries

Date: 2026-05-28

## Summary

Current `ccbd` is not built around in-process config hot reload. It treats the
loaded config as a startup-time dependency and distributes that object into
long-lived services. When the on-disk config signature changes, the keeper path
currently asks the mounted daemon to stop and then starts a new generation.

That behavior is correct for cold-start safety, but it is the wrong primitive
for "add a new agent without interrupting existing panes."

## Startup-Time Config Injection

`initialize_app()` loads config once, computes identity once, and injects the
same config object into several long-lived services:

- `app.registry = AgentRegistry(app.paths, app.config)`
- `RuntimeSupervisor(..., config=app.config, registry=app.registry, ...)`
- `RuntimeSupervisionLoop(..., config=app.config, registry=app.registry, ...)`
- `CompletionTrackerService(app.config, ...)`
- `JobDispatcher(app.paths, app.config, app.registry, ...)`
- `ProjectViewService(..., config=app.config, registry=app.registry, ...)`
- `ProjectFocusService(..., config=app.config, ...)`

This means a reload cannot safely update only `app.config`. Every config-bound
service either needs an explicit `reload_config()` contract or needs to be
rebuilt from shared persistent stores.

## Keeper Drift Handling

The keeper compares the live daemon's `ping('ccbd')` config signature with the
current on-disk config signature. If they differ, the keeper calls
`request_shutdown(app)` and records a restart attempt.

For hot reload, successful reload must update the live daemon's published
`config_signature`, lifecycle record, and any ping payload source. Otherwise the
keeper will immediately interpret the still-running daemon as stale and restart
it.

## Supervision Desired Set

`RuntimeSupervisionLoop.reconcile_once()` iterates `self._ctx.config.agents`.
The configured-agent set is therefore captured inside the supervision context.
Adding an agent requires replacing or updating this context before heartbeat can
mount or recover the new desired slot.

## Agent Registry Desired Set

`AgentRegistry` stores the config object and uses `self._config.agents` for
`spec_for()`, `list_all()`, and `list_known_agents()`. A newly-added agent is
unknown until registry config is updated or the registry is rebuilt.

The registry already protects runtime-authority fields from accidental
non-authority writes. Hot reload should preserve that safety by keeping existing
runtime records and cache entries for unchanged agents rather than rewriting
their authority.

## Namespace Topology Today

`ensure_project_namespace()` calls `topology_recreate_reason()` when a topology
plan is supplied for a live session. The current reasons include:

- `topology_workspace_changed`
- `topology_window_missing:<window>`
- `topology_agent_panes_changed`
- `topology_sidebar_panes_changed`

The existing topology materializer knows how to create a whole topology from a
fresh or recreated boundary. It does not yet have an additive patch path that
creates only a missing window/sidebar/agent pane while preserving old pane ids.

## Existing Hot-Load Precedent

`[ui.sidebar.view]` is explicitly UI-only in the config contract. `project_view`
reloads that section from disk and can report parse errors while retaining the
daemon's last valid view config. This is a useful precedent, but it does not
change desired agents, provider runtime, pane ownership, or namespace topology.

## Contract Constraints

The startup/supervision contract says:

- `.ccb/ccb.config` defines the desired agent mount set and foreground pane
  layout for the backend.
- `ccbd` continuously keeps configured agents mounted and healthy.
- Pane ids are evidence, not logical identity.
- Runtime authority writes must go through explicit authority paths.
- Unknown `.ccb/agents/*` directories are residue unless present in current
  config.

Hot reload must keep those rules intact. The safe version is therefore not
"mutate files and hope tmux matches"; it is a controlled daemon transaction that
updates desired config, config-bound services, namespace state, and lifecycle
signature together.
