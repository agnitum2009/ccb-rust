# Execution Plan

Date: 2026-05-29

## Summary

Dynamic agent load/unload/replace must be delivered as a sequence of small
control-plane changes. The unsafe version is a single large `reload` handler
that reparses config, swaps objects, mutates tmux, and updates lifecycle in one
patch. The safe version first creates a measurable current-service boundary,
then a dry-run diff engine, then bounded mutation paths.

## Phase 0: Baseline And Instrumentation

Goal: know the current resource cost before reload work changes behavior.

Deliverables:

- Add metrics for heartbeat duration, project-view build duration, handler
  latency, reload duration, tmux command count, `capture-pane` count, and RSS.
- Expose the metrics through `ping` or diagnostics without adding a heavy read
  path.
- Add focused tests that metrics are updated without changing command behavior.
- Record a manual `test_ccb2` baseline with current release behavior.

Exit criteria:

- A no-op idle project has stable heartbeat and project-view timings.
- Metrics show whether CPU cost is dominated by heartbeat, project-view,
  tmux/capture-pane, dispatcher scans, or handler lock contention.

Rollback:

- Metrics must be removable or ignorable without changing runtime authority.

## Phase 1: Service Graph Boundary

Goal: make config-bound services replaceable as a bundle.

Deliverables:

- Introduce `CcbdServiceGraph` or equivalent bundle containing config,
  config identity, registry, runtime supervisor, runtime supervision,
  completion tracker, dispatcher, project view, project focus, and ping payload
  dependencies.
- Add one builder used by startup and future reload.
- Keep persistent stores, path layout, project namespace controller, mount
  manager, ownership guard, socket server, execution service, snapshot writer,
  and lifecycle generation outside the graph.
- Add graph version and created-at metadata for diagnostics.

Exit criteria:

- Startup behavior is identical when bootstrapped through the graph builder.
- Unit tests prove the graph can be built twice from the same config without
  writing runtime authority.

Rollback:

- Revert to direct `app.*` service fields because no reload mutation uses the
  graph yet.

## Phase 2: Non-Blocking Handler Routing

Goal: prevent stale handler captures after reload without adding request-path
lock contention.

Deliverables:

- Register stable handler wrappers once.
- Each wrapper resolves the current service graph at request time.
- The steady-state read path must not acquire a contended mutex.
- Mutating reload may acquire an exclusive publish lock, but ordinary submit,
  project-view, ping, queue, and focus requests should use the last fully
  published graph.

Exit criteria:

- Tests replace the graph and prove `submit`, `project_view`, `ping`, and
  focus handlers use the new graph.
- Startup registers stable wrappers once; wrappers read the current graph once
  per request without reparsing config or rebuilding the graph.
- `service_graph_retained_count` is explicitly scoped as published graph count
  until true old-graph in-flight retention is implemented in a later mutating
  reload phase.
- Handler latency does not regress beyond the gate in
  [performance-baseline-and-gates.md](performance-baseline-and-gates.md).

Rollback:

- Keep wrapper registration but point wrappers at the original graph.

## Phase 3: Dry-Run Reload

Goal: compute the reload plan without mutating daemon, tmux, runtime, or
lifecycle state.

Deliverables:

- Add `project_reload_config` dry-run service.
- Add CLI `ccb reload --dry-run`.
- Load and validate `.ccb/ccb.config`.
- Build old/new topology plans and classify the diff.
- Return planned operations, blocked operations, affected agents/windows, and
  estimated mutation class.

Exit criteria:

- Invalid config returns structured errors and leaves all state untouched.
- No-op reload reports no changes.
- Add, unload, replace, move, and view-only cases are classified.
- Phase 3 implementation status:
  - `project_reload_config` rejects non-dry-run requests before updating reload
    metrics.
  - `ccb reload --dry-run` calls the mounted daemon and does not bootstrap or
    write a missing `.ccb/ccb.config`.
  - returned payloads include old/new config signatures, `plan_class`,
    `safe_to_apply=false`, `future_safe_to_apply`, operations, reasons,
    warnings, and errors.
  - classification is conservative: existing agent spec changes are reported as
    `replace_agent`; presentation-only identity-preserving diffs are reported
    as `view_only_change`; Phase 3 does not split metadata-only agent fields
    from runtime-relevant replacement fields.
  - metrics `last_reload_duration_s`, `last_reload_plan_class`, and
    `last_reload_error` are updated only after a dry-run handler invocation.
  - dry-run does not publish a service graph, mutate tmux, write lifecycle,
    lease, namespace, start-policy, restore, or agent runtime authority, and
    does not install a config watcher.

Rollback:

- Disable CLI entrypoint; no daemon mutation exists yet.

## Phase 4: Bounded Draining And Retiring

Goal: make unload safe before exposing replacement.

Deliverables:

- Add runtime states or lifecycle markers for `draining`, `retiring`,
  `pending_unload`, and `retired`.
- Define the state/predicate boundary needed to stop accepting new jobs for
  draining agents once mutating unload is enabled.
- Keep running work visible until completion, cancellation, timeout, or force.
- Add queue length and age limits for pending unload/replace records.
- Add clear terminal errors when a reload is rejected because a previous drain
  is still active.

Exit criteria:

- Idle drain reaches `idle_ready` / `retiring` without mutating runtime or tmux.
- Busy drain waits, then either reaches `idle_ready` within the configured
  bound or returns a stable timeout/rejected state.
- Pending unload/replace queues cannot grow unbounded.
- Actual new-job rejection, runtime retirement writes, and managed-pane removal
  remain deferred until mutating unload phases wire this state to dispatcher and
  namespace operations.

Phase 4 implementation status:

- Added `ccbd.reload_drain` as pure state machinery with `DrainIntent`,
  `DrainRecord`, `DrainBounds`, `DrainQueue`, `DrainQueueStore`,
  `plan_drain_transition()`, and `retire_record()`.
- Bounds are explicit: `max_pending` caps non-terminal unload/replace records
  across the queue, `timeout_s` caps active draining time, and `max_age_s` caps
  stale intent age before or during drain.
- Busy/idle is an injected predicate over the current `DrainRecord`; the module
  does not import dispatcher, comms, provider execution, tmux, namespace, or
  service-graph publish code.
- `DrainQueueStore` persists only explicit state-machine calls to
  `.ccb/ccbd/reload-drain.json`; heartbeat and request steady state do not scan
  it.
- Phase 3 dry-run plans now include `drain_intents` suggestions for
  `remove_agent` and `replace_agent`, but `safe_to_apply=false` and
  `mutation_enabled=false` remain unchanged.
- At the Phase 4 checkpoint, non-dry-run `project_reload_config` / `ccb reload`
  was still rejected. Phase 4 performed no tmux delete/create, graph publish,
  namespace patch, runtime authority write, config watch, mount, unmount,
  provider start, or provider stop.

Rollback:

- Treat deletion as `unsafe_requires_restart` until drain machinery is enabled.

## Phase 5: Namespace Patch Operations

Goal: introduce namespace patch/additive mutation foundations behind
dry-run-proven plans, without full namespace recreation or unrelated pane churn.

Deliverables:

- Add namespace patch planning records for add window, add sidebar, add agent
  pane, and view refresh.
- Record the project id, socket, session, namespace epoch, window, role,
  `slot_key`, and `managed_by=ccbd` proofs future mutation must satisfy.
- Keep remove/replace/move/layout mutation blocked until later phases.
- Do not use full namespace recreation for accepted additive plans.
- Keep future CCB-owned tmux settings project/session-scoped.

Exit criteria:

- Additive patch planning identifies `create_window`, `create_sidebar_pane`,
  and `create_agent_pane` steps without touching existing panes.
- Planner reports preserved existing agents for additive plans.
- Remove, replace, move, and unsupported layout changes remain blocked for
  non-dry-run mutation.
- Failed or blocked patch planning does not publish the new graph.

Phase 5 implementation status:

- Added `ccbd.reload_patch` as a pure namespace patch planner.
- Dry-run reload payloads now include `namespace_patch_plan` with
  `apply_deferred=true`, `mutation_enabled=false`, scope verification,
  required proofs, preserved agents, planned steps, blocked operations, and
  warnings.
- Supported planned classes are `view_only_change`, `add_agent`, and
  `add_window`; `remove_agent`, `replace_agent`, `move_agent`, and
  `layout_change` remain blocked.
- Add-agent planning is intentionally append-only inside an existing managed
  window; inserting/reordering existing agents is blocked as layout mutation.
- Add-window planning creates only deferred steps for the new window, sidebar,
  and new agent panes; it does not create tmux windows/panes yet.
- At the Phase 5 checkpoint, non-dry-run `project_reload_config` / `ccb reload`
  was still rejected before any socket mutation path. Phase 5 performed no tmux
  calls, provider start/stop, runtime authority writes, lifecycle/lease writes,
  or service-graph publish.

Rollback:

- Reject mutating reload and keep dry-run available.

## Phase 6: Additive Mutating Reload

Goal: expose the first safe mutation.

Phase 6a design-only status:

- Added [phase-6-additive-apply-design.md](phase-6-additive-apply-design.md).
- Confirmed `ensure_project_namespace(topology_plan=...)` is not the hot-reload
  apply API because additive missing windows/panes currently become namespace
  recreate reasons.
- Identified lower-level namespace primitives and the runtime mount path to
  reuse through new narrow APIs.

Phase 6b first-step status:

- Added `ProjectNamespaceController.apply_additive_patch(...)` and
  `NamespacePatchApplyResult`.
- The implementation supports `add_window` and append-only `add_agent`
  namespace patch plans.
- Append-only `add_agent` requires the new layout tree to expand the last
  existing agent pane; merely adding an agent name to the end of a different
  window layout is blocked.
- Insert, reorder, move, replace, delete, and arbitrary layout mutations remain
  blocked.
- Tests use a fake tmux backend and assert the patch path does not call full
  namespace ensure/recreate/reflow/kill paths.
- The API creates only new window/sidebar/agent pane evidence. It does not
  mount providers, write runtime authority, update lease/lifecycle, or publish
  a service graph.
- Added `run_additive_agent_mounts(...)` as the next internal helper. It is not
  wired into `project_reload_config`; tests inject `run_start_flow` and prove
  only namespace-patch-created new agents are mounted through the existing
  runtime authority path.
- Runtime mount helper failures keep graph publish and lease/lifecycle signature
  updates false. If the existing start flow writes a new-agent runtime record
  before failing, that record is explicit new-agent residue and preserved agents
  must remain unchanged.
- Added `publish_additive_reload_transaction(...)` as the signature/publish
  handoff helper. It blocks unless namespace patch is applied and runtime mounts
  are mounted/no-op, then updates the current lease and mounted lifecycle config
  signature before publishing the service graph.
- Lease signature update uses a narrow current-holder API with pid,
  daemon_instance_id, and generation checks. Lifecycle signature update also
  checks mounted phase, owner, daemon instance, and generation.
- Publish remains an internal helper only. If namespace patch, runtime mount,
  signature handoff, or graph publish fails, the current app graph/config remain
  unchanged; signature writes are rolled back to the old config signature on
  post-write failures when the current holder/generation still match.
- Added `run_additive_reload_apply(...)` as the internal end-to-end
  orchestrator. It reads the current graph, builds the dry-run plan, accepts
  `view_only_change`, append-only `add_agent`, `add_window`, and idle
  `remove_agent`, builds the target service graph without publishing it, then
  runs namespace patch, runtime mount/unload, and signature/publish transaction
  in that order.
- The orchestrator runs under the existing app maintenance lock so heartbeat
  reconciliation does not race the staged apply. Ordinary handler graph reads
  still use the already-published graph and are not put behind a request-path
  mutex.
- Stage failures at plan, namespace patch, runtime mount, or publish transaction
  stop the sequence before later stages. Failure diagnostics include created
  pane/window residue and new-agent runtime authority residue when present; the
  old graph/config remain visible unless the final publish succeeds.
- `project_reload_config(dry_run=false)` and plain `ccb reload` now call the
  orchestrator, but only the accepted additive classes may publish.
- `ccb reload --dry-run` remains no-mutation and keeps the same planning
  payload shape.
- `remove_agent` is accepted only for idle/no-outstanding agents and only when
  remaining agent order is preserved. Busy or outstanding targets block before
  namespace mutation. `replace_agent`, `move_agent`, and arbitrary
  `layout_change` still block with no graph publish, no runtime authority
  writes, no lease/lifecycle signature writes, and no tmux reflow.
- CLI non-dry-run output includes apply stage, plan class, graph versions,
  diagnostics, and namespace/runtime residue. Handler diagnostics mark
  `project_view_cache_invalidated=true` after successful publish and
  `sidebar_refresh_signal_sent=false` because no active sidebar refresh signal
  has been added in this patch.
- Plain `ccb reload` writes a short TTL `reload-handoff.json` before submitting
  the RPC, and accepted daemon-side apply with a changed config signature
  overwrites the same record around the transaction. Keeper may use it only to
  tolerate the specific window where disk config has the target signature and
  daemon ping still reports the old graph signature; holder, daemon instance,
  generation, project id, and age must match.
- Outside that handoff proof, a modern mounted daemon whose ping includes a
  config signature is still considered the current project daemon when the
  signature differs from disk config. The mismatch means `reload pending`, not
  `restart incompatible daemon`; keeper and CLI compatibility checks must keep
  it alive so explicit reload can apply the delta without interrupting existing
  agents.
- Sidebar's refresh control and `r` shortcut call
  `project_reload_config(dry_run=false)` and then force a project-view refresh,
  so edited configs can materialize additive tmux panes/windows through the same
  explicit reload path. No background config watcher is added.

Deliverables:

- Enable view-only, add-agent, and add-window reload.
- Publish new service graph only after namespace patch and new runtime mount
  succeed.
- Update lifecycle/lease/ping config signature so keeper does not restart the
  hot-loaded daemon.
- Bound the pre-publish keeper signature race with a fail-closed reload
  handoff record.
- Invalidate project view after publish. Active sidebar refresh signaling is
  still not implemented in this patch and is reported as
  `sidebar_refresh_signal_sent=false`; sidebar user actions trigger reload
  directly instead of waiting for a daemon push.

Exit criteria:

- Busy unrelated agents continue through add-agent/add-window reload.
- Keeper sees the new config as current.
- Manual `test_ccb2` screenshots show unchanged old panes and new mounted
  agents.
- `preserved_agents` is treated only as the pane-preservation gate input; apply
  must prove unchanged pane ids with before/after snapshots before graph
  publish.
- Scope diagnostics distinguish namespace load failure, missing namespace, and
  missing/mismatched project/socket/session/epoch proofs.
- The final config-signature handoff is protected against keeper restart races:
  the keeper must not observe new disk config while daemon ping still reports
  the old graph signature and then request shutdown.
- Stale, mismatched, or wrong-holder handoff records do not suppress the normal
  keeper drift restart path.

Rollback:

- Disable mutating classes and keep dry-run.

## Phase 7: Dynamic Unload

Goal: expose safe unload after bounded drain is proven.

Implementation status:

- Explicit `ccb reload` now accepts the first idle unload slice. Removing an
  agent from `[windows]` plans a `kill_agent_pane` namespace patch step.
- Before killing any pane, the apply path checks dispatcher outstanding work
  and runtime `BUSY` state for every removed agent. If a target is active, the
  reload is blocked at plan stage and existing panes remain untouched.
- For idle targets, namespace patch kills only the target managed pane and
  proves preserved agent panes are unchanged before/after mutation.
- Runtime unload terminates that agent's provider helper manifest when present,
  writes stopped runtime authority through the existing registry path, and
  publishes the target graph through the existing lease/lifecycle signature
  transaction.
- Removing an entire managed window is limited to windows whose agents are all
  removed; replacement, cross-window movement, arbitrary layout reshaping, and
  forced/bounded busy draining remain deferred.

Deliverables:

- Enable deletion from `[windows]` to plan and execute idle unload.
- Retire runtime authority through explicit authority writes.
- Remove managed pane only after runtime is idle/no-outstanding. Completed,
  cancelled, timed out, force-approved, or bounded-draining unload remains a
  follow-up.
- Preserve `.ccb/agents/<agent>` history as residue/audit data, not configured
  authority.

Exit criteria:

- Removing an idle agent unloads it without affecting other panes.
- Removing a busy agent follows the configured draining behavior.
- Project view no longer treats retired agents as configured agents.

Rollback:

- Return deletion to `unsafe_requires_restart`.

## Phase 8: Dynamic Replace

Goal: replace an existing agent route without breaking unrelated panes.

Deliverables:

- Treat provider/workspace/model/key/url changes as replace plans.
- Idle replacement can retire the old runtime and mount the new runtime in the
  same logical slot.
- Busy replacement becomes bounded `pending_replace`.
- Replacement must never rewrite provider session authority as if it were the
  same conversation unless provider-specific resume authority proves it.

Exit criteria:

- Idle replace preserves slot identity but advances runtime authority epoch.
- Busy replace cannot grow unbounded and cannot block future reload forever.
- Codex/Claude session continuity is preserved or explicitly restarted.

Rollback:

- Return replace classes to `unsafe_requires_restart`.

## Phase 9: Optional Movement And Watchers

Goal: handle layout reshaping only after core dynamic lifecycle is stable.

Deliverables:

- Consider idle pane movement within the same project namespace.
- Consider file watching only after explicit reload is reliable.
- Keep busy pane cross-window movement deferred unless there is a proven
  session-preserving tmux operation and rollback path.

Exit criteria:

- Movement has separate tests and does not share first-release reload gates.
