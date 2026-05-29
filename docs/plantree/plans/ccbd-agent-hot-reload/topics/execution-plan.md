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

Rollback:

- Disable CLI entrypoint; no daemon mutation exists yet.

## Phase 4: Bounded Draining And Retiring

Goal: make unload safe before exposing replacement.

Deliverables:

- Add runtime states or lifecycle markers for `draining`, `retiring`,
  `pending_unload`, and `retired`.
- Stop accepting new jobs for draining agents.
- Keep running work visible until completion, cancellation, timeout, or force.
- Add queue length and age limits for pending unload/replace records.
- Add clear terminal errors when a reload is rejected because a previous drain
  is still active.

Exit criteria:

- Idle unload retires the runtime and removes the managed pane.
- Busy unload waits, then either completes within the configured bound or
  returns a stable timeout/rejected state.
- Pending queues cannot grow unbounded.

Rollback:

- Treat deletion as `unsafe_requires_restart` until drain machinery is enabled.

## Phase 5: Namespace Patch Operations

Goal: mutate only the target CCB-owned tmux surfaces.

Deliverables:

- Add namespace patch operations for add window, add sidebar, add agent pane,
  remove retired agent pane, and refresh sidebar width/UI.
- Every operation must prove project id, socket, session, window, role,
  `slot_key`, and `managed_by=ccbd` before mutation.
- Do not use full namespace recreation for accepted additive/unload operations.
- Keep CCB-owned tmux settings project/session-scoped.

Exit criteria:

- Additive reload preserves old pane ids.
- Retired unload removes only the target agent pane.
- Failed patch does not publish the new graph.

Rollback:

- Reject mutating reload and keep dry-run available.

## Phase 6: Additive Mutating Reload

Goal: expose the first safe mutation.

Deliverables:

- Enable view-only, add-agent, and add-window reload.
- Publish new service graph only after namespace patch and new runtime mount
  succeed.
- Update lifecycle/lease/ping config signature so keeper does not restart the
  hot-loaded daemon.
- Invalidate project view and refresh sidebars.

Exit criteria:

- Busy unrelated agents continue through add-agent/add-window reload.
- Keeper sees the new config as current.
- Manual `test_ccb2` screenshots show unchanged old panes and new mounted
  agents.

Rollback:

- Disable mutating classes and keep dry-run.

## Phase 7: Dynamic Unload

Goal: expose safe unload after bounded drain is proven.

Deliverables:

- Enable deletion from `[windows]` to plan and execute unload.
- Retire runtime authority through explicit authority writes.
- Remove managed pane only after runtime is idle, completed, cancelled, timed
  out, or force-approved.
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
