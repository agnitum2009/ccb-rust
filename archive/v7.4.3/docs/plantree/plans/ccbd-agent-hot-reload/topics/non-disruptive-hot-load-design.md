# Non-Disruptive Hot Load Design

Date: 2026-05-29

## Goal

The user edits `.ccb/ccb.config` to add, unload, or eventually replace an
agent. Unrelated running agents keep their panes, provider sessions, and
in-progress work. The daemon loads the new config, plans the diff, mutates only
accepted targets, and sidebar shows the new desired set.

## Reload Classes

The reload classifier should return one of these classes:

| Class | Meaning | First behavior |
| :--- | :--- | :--- |
| `view_only` | Only `[ui.sidebar.view]` or equivalent presentation fields changed. | Invalidate `project_view`; no namespace/runtime mutation. |
| `additive_agent` | A new agent is added to an existing managed window without changing existing agent specs or ownership. | Add one managed pane and mount the agent. |
| `additive_window` | A new window is added with sidebar and one or more new agents. | Create the new window/sidebar/agent panes. |
| `metadata_future_only` | Non-runtime metadata changes that should apply only to future launches. | Update config-bound read models, mark no active relaunch. |
| `unload_agent` | An existing configured agent is removed from `[windows]`. | Dry-run first; later bounded drain and retire. |
| `replace_agent` | Existing agent provider/workspace/model/key/url/runtime boundary changes. | Dry-run first; later idle replace or bounded pending replace. |
| `unsafe_requires_restart` | Rename, unsupported move, busy cross-window reshuffle, or operation beyond current release gates. | Reject reload with structured reason; do not kill panes. |
| `invalid_config` | New config cannot parse or violates config contract. | Reject reload; preserve old daemon config. |

## Proposed Flow

1. CLI sends `ccb reload --dry-run` or `ccb reload` to the mounted daemon.
2. Handler acquires `app.start_maintenance_lock`.
3. Load and validate `.ccb/ccb.config` from disk.
4. Compute `old_config_identity`, `new_config_identity`, and a structured diff.
5. If dry-run, return the planned operations and mutate nothing.
6. If invalid or unsafe, return a result explaining the blocking fields and
   leave all daemon state unchanged.
7. Build replacement config-bound services using the new config and existing
   persistent stores.
8. Patch the project namespace for accepted operations:
   - create missing managed windows;
   - create missing sidebar panes for new windows;
   - create missing agent panes only for new agents;
   - remove only fully-retired managed agent panes;
   - preserve every unchanged agent pane id;
   - update tmux pane identities for newly-created panes.
9. Mount newly-added agents through the normal start/mount path, inheriting the
   current project start policy for restore and auto-permission behavior.
10. Atomically publish the new config identity to `app.config`,
   `app.config_identity`, lifecycle, lease/ping payload source, supervisor,
   supervision, registry, dispatcher, project view, project focus, and
   completion tracker.
11. Invalidate `project_view` cache and refresh all managed sidebar panes in
    the project session.
12. Return a structured reload summary with changed agents, changed windows,
    skipped unsafe changes, and any mounted agent results.

## Service Rebinding Shape

The preferred implementation is a small `ConfigRuntimeBundle` builder that can
construct every config-bound service from:

- new config;
- existing `PathLayout`;
- existing persistent stores;
- existing project namespace controller;
- existing mount manager and ownership guard;
- existing execution/fault/snapshot services;
- existing lifecycle generation getter.

The reload transaction swaps the bundle into `app` only after validation and
namespace patching succeed. This avoids subtle half-mutated objects where
`app.config` is new but registry, supervisor, and dispatcher still point at the
old desired set.

Handlers must not capture config-bound services permanently. Stable handler
wrappers should read the current graph at request time through a non-blocking
or proven-low-contention path.

## Additive Namespace Patcher

Do not reuse `topology_recreate_reason()` as the hot reload gate. It currently
turns additive missing windows or panes into recreate reasons.

Add a separate patch path, for example:

```text
patch_project_topology_additively(old_plan, new_plan, current_namespace)
```

The patcher should:

- prove the current session/window identity still matches the authoritative
  namespace state;
- list existing managed agent/sidebar panes by CCB tmux user options;
- reject changes that require moving an existing agent to another window;
- reject changed existing `realized_layout` unless the change only adds new
  leaves in a supported append position;
- create missing windows with the same project-scoped tmux UI policy;
- create one sidebar pane per new window when sidebar is enabled;
- create one agent pane per new agent;
- write `@ccb_project_id`, `@ccb_role`, `@ccb_slot`, `@ccb_window`,
  `@ccb_managed_by`, and epoch options for each new pane;
- return `namespace_agent_panes` for both existing and newly-created agents.

Phase 5 establishes the planner half of this patch path:

- `ccbd.reload_patch` produces deferred `create_window`,
  `create_sidebar_pane`, `create_agent_pane`, and view-refresh steps;
- it records required project/session/window/role/slot proofs before any future
  apply can mutate tmux;
- it reports existing agents expected to preserve pane ids;
- it blocks remove, replace, move, and arbitrary layout changes;
- it intentionally does not call tmux, mount providers, write runtime
  authority, or publish a service graph.

Phase 6a documents the apply half in
[phase-6-additive-apply-design.md](phase-6-additive-apply-design.md). The key
constraint is that `preserved_agents` is only the input set for a future
before/after pane-id preservation gate; it is not evidence that reuse already
happened.

## Safety Invariants

- Existing agent pane ids must not change for accepted additive reloads.
- Existing provider processes must not be killed, respawned, or sent input by
  unrelated reload operations.
- Existing runtime authority epochs must not be rewritten unless the agent is
  newly-created, retired, or explicitly replaced.
- A busy agent must continue running while reload adds unrelated agents.
- Draining and pending replacement must have queue and age bounds.
- A failed reload must leave `app.config_identity`, lifecycle, registry,
  namespace state, runtime records, and project view using the old accepted
  config.
- The keeper must not restart the daemon after a successful reload.
- CCB-owned tmux behavior remains project/session-scoped; reload must not rely
  on global tmux config.

## Initial UX

Start explicit:

```bash
ccb reload --dry-run
ccb reload
```

Return examples:

```text
reload_status: ok
added_windows: review
added_agents: agent5
unchanged_agents: agent1, agent2, agent3, agent4
```

For unsafe changes:

```text
reload_status: requires_restart
blocked:
  agent2.workspace_mode changed while agent2 is running
  review layout moved existing agent3
hint: run ccb restart or ccb kill && ccb
```

Do not silently restart or kill panes as part of first-phase hot reload.

Detailed execution sequencing lives in
[execution-plan.md](execution-plan.md). Dynamic unload and replacement semantics
live in [dynamic-unload-and-replace.md](dynamic-unload-and-replace.md).
