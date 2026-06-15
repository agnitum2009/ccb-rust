# Phase 6 Additive Apply Design

Date: 2026-05-29

## Scope

Phase 6a was design-only. It did not enable non-dry-run `ccb reload`, call
tmux, mount providers, write runtime authority, update lifecycle/lease state, or
publish a service graph.

Phase 6b enables only these mutating classes through the explicit reload path:

- `view_only_change`;
- append-only `add_agent`;
- `add_window`.

`remove_agent`, `replace_agent`, `move_agent`, and arbitrary `layout_change`
remain rejected until later phases.

## Current API Findings

At the Phase 6a checkpoint, `project_reload_config` still rejected non-dry-run
before loading the new config. That guard was kept until the additive
transaction path existed and was wired behind the Phase 6b user-path gate.

`build_ccbd_service_graph()` is the right way to build a new config-bound
bundle. `publish_ccbd_service_graph()` is the only publish step and must be the
last in-memory graph mutation after namespace, runtime, lease, and lifecycle
state are consistent.

`ensure_project_namespace()` is not safe as the Phase 6b additive entrypoint.
For explicit window topology it calls `topology_recreate_reason()`, and missing
windows, missing agent panes, or missing sidebar panes become recreate reasons.
That is correct for cold start but too broad for hot reload.

The reusable tmux primitives are lower-level:

- `create_window()` for a new managed window in the current project session;
- `window_root_pane()` for the new window root pane;
- `split_pane()` for sidebar and agent leaves;
- `apply_ccb_pane_identity()` for `@ccb_project_id`, role, slot, window,
  namespace epoch, and `managed_by=ccbd` options;
- `existing_topology_agent_panes()` and a new preservation snapshot helper for
  pre/post pane evidence.

The reusable runtime path is `run_start_flow()` with
`namespace_agent_panes={new_agent: pane_id}` and
`cleanup_tmux_orphans=False`. It already passes assigned panes into
`start_agent_runtime()`, which launches or reuses provider runtime, relabels the
project namespace pane, and writes runtime authority through `RuntimeService`.
Phase 6b should call it only for newly-added agents, not for preserved agents.

`CcbdStartPolicyStore` should be read, not updated, by reload. Startup owns
persisting policy; reload should inherit `recovery_restore` and
`auto_permission` when present.

`MountManager.mark_mounted()` is the existing mounted lease write path, but
reload should not call it directly because it is a startup-style write unless
all current holder, generation, and `started_at` fields are replayed exactly.
Phase 6b needs a narrow wrapper that asserts the current lease holder matches
`app.pid` and `app.daemon_instance_id`, then rewrites only the
signature/heartbeat fields.

Lifecycle signature update can reuse the existing lifecycle model by loading the
current mounted lifecycle and saving `with_updates(config_signature=...,
namespace_epoch=...)`. Phase 6b should add a named helper so reload does not
call the startup-only `_mark_lifecycle_mounted()` path directly.

## Required Narrow APIs

Phase 6b added these before opening non-dry-run reload:

- `ProjectNamespaceController.apply_additive_patch(plan, old_topology,
  new_topology, timeout_s=...) -> NamespacePatchApplyResult`.
- `NamespacePatchApplyResult` fields:
  `created_windows`, `created_panes`, `agent_panes`, `sidebar_panes`,
  `preserved_before`, `preserved_after`, `partial`, `rollback_actions`, and
  `diagnostics`.
- `snapshot_preserved_agent_panes(controller, context, agents) -> dict[str,
  str]`.
- `assert_preserved_agent_panes(before, after)` to fail before publish if any
  preserved agent has a changed or missing pane id.
- `update_current_lease_config_signature(app, new_signature)`.
- `update_mounted_lifecycle_config_signature(app, new_signature,
  namespace_epoch)`.
- `build_reload_service_graph(app, new_config, version=...)`, a reload wrapper
  around `build_ccbd_service_graph()` that reuses startup dependencies and does
  not publish.
- `run_additive_agent_mounts(app, graph, namespace, agent_panes, agent_names)`,
  which calls `run_start_flow()` or a small supervisor wrapper only for new
  agents, with `cleanup_tmux_orphans=False`,
  `interactive_tmux_layout=True`, and explicit `namespace_agent_panes`.

## Add Agent Append

The planner/apply boundary accepts append-only changes only when the old window
agent list is a prefix of the new list and the new layout tree expands the last
existing agent leaf. Phase 6b implements this by:

1. Loading the current namespace and verifying project id, tmux socket path,
   session name, namespace epoch, window, role, slot key, and
   `managed_by=ccbd`.
2. Taking `preserved_before` for `namespace_patch_plan.preserved_agents`.
   This list is only the input set for the preservation gate, not a promise
   that panes were already reused.
3. Finding the anchor pane from the previous managed agent in the same window.
4. Splitting exactly one new pane per appended agent from the anchor/previous
   new pane, then applying CCB pane identity to the new pane.
5. Returning `agent_panes` only for new agents and `preserved_after` for the
   preservation gate.

Implemented narrow API: a pure rightmost-leaf append proof computes the split
direction for each appended leaf without parsing the whole new layout into a
full-window materialization that would touch old panes. Layouts that add an
agent name but require changing any old pane outside that last-leaf expansion
remain blocked.

## Add Window

`add_window` should not call `ensure_project_namespace(topology_plan=new_plan)`,
because that path can recreate the whole namespace.

The narrow patcher should:

1. Verify the current session identity from namespace state.
2. Call `create_window()` with `session_name=current.tmux_session_name`,
   the new window name, and project root. This is project/session scoped.
3. Resolve the new window root pane with `window_root_pane()`.
4. If sidebar is enabled, split the root pane, respawn the sidebar command, and
   apply sidebar CCB identity for the new window only.
5. Materialize only the new window's agent layout under the new window user
   root, applying CCB identity to each new agent pane.
6. Refresh project tmux UI/sidebar width for the project session only.
7. Persist namespace state with the new topology signature and the same
   namespace epoch unless a later decision explicitly advances epoch for
   additive patches. The chosen rule must be consistent with focus requests that
   validate namespace epoch.

This path creates no windows or panes for existing windows and must not call
`kill_server`, `kill_window`, `reflow_workspace`, `force_recreate_namespace`, or
`topology_recreate_reason()` as an apply gate.

## Transaction Order

For accepted non-dry-run reload, Phase 6b should execute in this order:

1. Acquire a reload/app maintenance lock so heartbeat does not concurrently
   mount/reconcile through the old desired set while the transaction is active.
2. Read current graph, load new config once, build the dry-run plan, and reject
   unless the plan class is `view_only_change`, append-only `add_agent`, or
   `add_window` with an unblocked namespace patch plan.
3. Build the new service graph but do not publish it.
4. Snapshot preserved agent pane ids and existing runtime authority for
   `preserved_agents`.
5. Apply the additive namespace patch and collect new pane ids. For
   `view_only_change`, skip tmux namespace mutation.
6. Mount/start only new agents through the new graph runtime service and the
   current start policy, using the new pane ids from the patcher.
7. Verify every new configured agent has runtime authority and every preserved
   agent has unchanged pane id/runtime authority evidence.
8. Update lease config signature for the current daemon holder.
9. Update mounted lifecycle config signature and namespace epoch.
10. Publish the new service graph.
11. Invalidate project view cache and refresh managed sidebar panes in the
    project session.
12. Return a structured apply summary.

The graph publish step is intentionally after all durable state required by
keeper and handlers is consistent. If any earlier step fails, handlers continue
reading the old graph.

The final signature handoff needs dedicated Phase 6b race coverage. The keeper
process checks the mounted daemon by calling `ping('ccbd')` and comparing the
daemon-reported config signature with the on-disk config. A reload implementation
must therefore prevent a keeper-visible window where disk config is new,
`ping('ccbd')` still reports the old graph signature, and the keeper requests
shutdown. The current user path writes a short TTL `reload-handoff.json` before
the CLI submits the non-dry-run RPC, then the daemon overwrites the same record
after the plan is accepted and only when the target config signature differs
from the current daemon signature. Keeper accepts the mismatch only when the
handoff project, target signature, old daemon signature, holder pid, daemon
instance, generation, socket connectivity, liveness, and age all match. Stale,
mismatched, missing, or unreadable handoff records fail closed and keep the
normal drift restart behavior.

## Failure And Diagnostics

Failure before namespace patch:

- return `reload_status=blocked` or `failed`;
- do not publish a graph;
- do not write lease/lifecycle signature;
- no rollback is required because no mutation happened.

Failure during namespace patch:

- do not publish a graph;
- do not write lease/lifecycle signature;
- return created panes/windows and `partial=true`;
- best-effort rollback may kill only panes/windows created by this patch and
  only if their identity options prove `project_id`, session, namespace epoch,
  role, slot, and `managed_by=ccbd`;
- if rollback is not proven, leave created panes as managed residue and report a
  recoverable diagnostic for a later repair path.

Failure during new runtime mount:

- do not publish a graph;
- do not write lease/lifecycle signature;
- preserve any runtime failure records written by the normal mount path for the
  new agent only;
- leave existing agents untouched;
- return the new pane/runtime diagnostic so the user can retry after fixing the
  provider problem.

Failure during lease/lifecycle signature update:

- do not publish a graph;
- return failure with old graph active;
- if a signature write already happened, roll lease/lifecycle config signatures
  back to the old graph signature before returning failure when holder and
  generation still match;
- new panes/runtime records may exist as residue or partially mounted new
  agents, but they are not part of the published desired graph until retry
  succeeds;
- diagnostics must include whether signature rollback was attempted and
  completed.

Failure after graph publish:

- only project-view/sidebar refresh should remain. A refresh failure should be
  reported as degraded UI refresh, not as reload rollback, because config,
  namespace, runtime, lease, lifecycle, and graph are already consistent.

## Preservation Proof

`namespace_patch_plan.preserved_agents` is the set of agents common to old and
new topology. It is not a claim that panes have already been reused.

Phase 6b must prove preservation by recording:

- `preserved_before`: agent -> pane id from current CCB tmux identity options
  before patch;
- `preserved_after`: agent -> pane id after patch;
- unchanged runtime authority fields for preserved agents, especially pane id,
  runtime refs, provider/session refs, and daemon generation.

Reload must fail before graph publish if a preserved agent is missing in either
snapshot or has a changed pane id.

## Scope Diagnostics

Dry-run and apply diagnostics should distinguish:

- namespace state load failed;
- namespace state missing;
- project id mismatch;
- tmux socket path missing;
- session name missing;
- namespace epoch missing;
- UI not attachable.

The current Phase 5 `_current_namespace(app)` returns `None` for both missing
state and load failure, and `reload_patch` reports a broad scope failure. Phase
6b should return a small diagnostic object instead, while preserving the
existing dry-run payload shape for compatibility.

## Phase 6b Namespace Patch Steps

The first implementation step was not enabling `ccb reload`. It added a fake
backend unit-tested namespace patch apply API for `add_window`:

- input: Phase 5 `namespace_patch_plan`, old/new topology, and current
  namespace;
- output: `NamespacePatchApplyResult`;
- tests: no calls to `kill_server`, `kill_window`, `reflow_workspace`, or
  `ensure_project_namespace`; only project/session-scoped `create_window`,
  `split_pane`, and identity writes for the new window.

After that passes, add append-only `add_agent`, then wire runtime mounts, then
open non-dry-run reload behind the transaction order above.

Implementation status:

- `ProjectNamespaceController.apply_additive_patch(...)` now exists and returns
  `NamespacePatchApplyResult`.
- The implementation supports `add_window` and append-only `add_agent`.
- Append-only `add_agent` requires the new layout tree to expand the last
  existing agent pane. It splits one new managed pane from that anchor and
  writes CCB identity evidence for the new pane only.
- Insert, reorder, move, replace, delete, and arbitrary layout mutations remain
  blocked.
- The fake-backend tests cover new window/sidebar/agent pane creation,
  append-only agent pane creation, `managed_by=ccbd` identity evidence,
  preservation snapshots, patch-plan/topology mismatch, and failure diagnostics.
- At the namespace-patch checkpoint, this API was not wired into
  `project_reload_config`; by itself it still does not mount providers, write
  runtime authority, update lease/lifecycle, publish a graph, or add config
  watching.

## Phase 6b Runtime Mount Helper

`run_additive_agent_mounts(...)` is now the internal runtime handoff for the
next transaction step. It accepts the target service graph, current namespace,
and a successful `NamespacePatchApplyResult`, then calls the existing
`run_start_flow(...)` path only for `patch_result.agent_panes`.

Current status:

- It uses the target graph's `RuntimeSupervisor` state for config, paths,
  runtime service, project id, clock, and start policy. This preserves the
  existing start-flow and `RuntimeService.attach(...)` authority semantics
  instead of inventing a reload-specific runtime writer.
- It passes explicit `namespace_agent_panes`, `cleanup_tmux_orphans=False`,
  `interactive_tmux_layout=True`, `fresh_namespace=False`, and
  `fresh_workspace=False`.
- Tests inject a fake `run_start_flow` so no provider or real tmux backend is
  launched in this phase.
- It rejects attempts to mount preserved agents or agents not present in the
  target graph config.
- It snapshots preserved runtime authority before and after the start flow and
  fails before publish if any preserved record changes.
- It reports `graph_published=false`, `lease_or_lifecycle_written=false`,
  `cleanup_tmux_orphans=false`, and `config_watch_started=false` on success and
  failure.
- If start flow writes a new-agent runtime record and then fails, the helper
  reports partial failure with `runtime_authority_written_agents` limited to the
  new agent. That residue remains outside the published desired graph until the
  later lease/lifecycle and graph-publish handoff succeeds.

## Phase 6b Signature Publish Helper

`publish_additive_reload_transaction(...)` is now the internal handoff after
namespace patch and runtime mounts.

Current status:

- It requires `NamespacePatchApplyResult.status == applied`.
- It requires runtime mount status `mounted` or `noop`; partial/failed runtime
  residue blocks publish and is echoed in diagnostics.
- It updates the lease config signature through a narrow
  `MountManager.update_config_signature(...)` API that checks current pid,
  daemon instance id, and generation instead of replaying startup
  `mark_mounted(...)`.
- It updates mounted lifecycle config signature only when lifecycle is mounted
  and owner pid, daemon instance id, and generation still match the current
  daemon.
- It publishes the target service graph only after both signature writes
  succeed. If publish itself fails, it rolls signatures back to the old graph
  signature and leaves the old app graph active.
- Tests cover successful publish, namespace patch failure, runtime mount
  failure with new-agent residue, signature handoff failure with rollback,
  stale lease generation, and publish failure with rollback.

This helper is now used by the explicit user path only through the additive
apply orchestrator. Non-additive plans still block before transaction publish.

## Phase 6b Apply Orchestrator

`run_additive_reload_apply(...)` is the internal orchestration helper used by
`project_reload_config(dry_run=false)` and plain `ccb reload`.

Current status:

- It runs under the existing app maintenance lock, then reads the current
  service graph and current namespace once.
- It builds the same dry-run plan used by `ccb reload --dry-run` and rejects
  every class except `view_only_change`, append-only `add_agent`, and
  `add_window`.
- It builds the target service graph with the same config-bound builder used by
  startup and does not publish that graph until the transaction stage.
- `view_only_change` synthesizes an applied no-op namespace patch and proceeds
  to runtime noop plus signature/publish transaction.
- Additive namespace changes call `ProjectNamespaceController.apply_additive_patch(...)`.
  The helper does not call full namespace ensure/recreate/reflow/kill paths.
- Runtime handoff calls `run_additive_agent_mounts(...)`, so only new
  namespace-patch-created agent panes are mounted through the existing
  start-flow/runtime authority path.
- Final publish calls `publish_additive_reload_transaction(...)`, preserving the
  signature handoff and rollback behavior added earlier in Phase 6b.
- Failure at plan, namespace patch, runtime mount, or publish transaction stops
  before later stages and returns structured diagnostics. Namespace diagnostics
  include created window/pane residue; runtime diagnostics include new-agent
  authority residue. The current app graph/config stay unchanged unless the
  publish transaction returns `published`.

## Phase 6b User Path Gate

Current status:

- `project_reload_config(dry_run=false)` loads `.ccb/ccb.config` once and calls
  `run_additive_reload_apply(...)`.
- Plain `ccb reload` sends `dry_run=false`; `ccb reload --dry-run` remains
  compatible and no-mutation.
- Only `view_only_change`, append-only `add_agent`, and `add_window` can reach
  namespace patch/runtime/publish stages.
- `remove_agent` can reach namespace/runtime/publish stages only for idle
  agents with no outstanding dispatcher work and preserved remaining order.
- `no_change` returns noop without graph publish; busy `remove_agent`,
  `replace_agent`, `move_agent`, and arbitrary `layout_change` return
  blocked/failed structured output and do not publish.
- Successful publish invalidates the current project-view cache so the next
  `project_view` and `ping('ccbd')` read the newly published graph/config.
- Apply writes and clears a bounded keeper handoff record so the pre-publish
  old-signature window is not misclassified as config drift.
- Outside the bounded handoff proof, modern config-signature drift is treated as
  reload pending rather than daemon incompatibility. Keeper and CLI
  compatibility checks keep the mounted daemon alive until explicit reload
  applies or rejects the disk config.
- CLI rendering includes reload stage, graph versions, publish diagnostics, and
  namespace/runtime residue for failed applies.
- Sidebar's refresh control and `r` shortcut now submit `project_restart_panes`
  to respawn each configured agent pane in place, then force a project-view
  refresh. Config changes remain explicit CLI actions through `ccb reload`; the
  sidebar control is not a file watcher or steady-state scan.

The user-path gate still does not add config watching, replacement, arbitrary
existing-pane kill/reflow, or busy-drain unload. Daemon-pushed sidebar refresh
is not signaled yet; diagnostics explicitly report
`sidebar_refresh_signal_sent=false` after successful publish so the remaining
push-refresh work is visible.
