# Performance Baseline And Gates

Date: 2026-05-29

## Position

Dynamic reload should have near-zero steady-state cost. The reload transaction
can spend CPU and memory when the user explicitly runs it, but normal submit,
ping, sidebar project-view refresh, and heartbeat paths must not become heavier.

The current daemon already has user-reported high CPU and memory use, so the
first implementation phase must measure the baseline before mutating reload is
enabled.

## Metrics To Add

Control-plane metrics:

- `last_handler_latency_s` by operation family.
- `last_reload_duration_s`.
- `last_reload_plan_class`.
- `last_reload_error`.

Maintenance metrics:

- `last_heartbeat_duration_s`.
- Per-step heartbeat timings for health monitor, runtime supervision,
  dispatcher runtime views, dispatcher tick, completion polling, and job
  heartbeat.
- Number of agents inspected per heartbeat.
- Number of runtime authority writes per heartbeat.

Project view metrics:

- `last_project_view_build_duration_s`.
- `last_project_view_response_duration_s`.
- Cache hit/miss count.
- Tmux command count per build.
- `capture-pane` count per build.
- Store scan count per build.

Process metrics:

- RSS and virtual memory.
- File descriptor count.
- Thread count.
- Pending maintenance ticks.

## Baseline Runs

Use `/home/bfly/yunwei/test_ccb2` for manual runtime checks.

Run at least:

- Idle project with current agent count.
- One busy Codex/Claude manual pane.
- Sidebar open on every managed window.
- Repeated window/pane focus changes.
- One long-running job plus sidebar refresh.
- Config edit followed by dry-run reload.

Record:

- 60-second idle CPU and RSS.
- 60-second sidebar-open CPU and RSS.
- p50/p95 project-view build time.
- p50/p95 heartbeat duration.
- p50/p95 handler latency for `ping`, `project_view`, and `submit`.
- tmux command count per sidebar refresh.

## Gates Before Mutating Reload

Do not expose mutating reload until:

- Handler current-graph reads are non-blocking or proven not to contend under
  sidebar refresh plus submit traffic.
- No steady-state path reparses `.ccb/ccb.config` for reload purposes.
- No steady-state path rebuilds the service graph.
- `project_view` cache behavior is unchanged or improved.
- Heartbeat per-step timings are visible.
- Tmux/capture-pane counts are visible for project-view builds.
- Old service graph retention is bounded and observable.

Phase 1 and later should add service-graph version, retained count, and graph
read latency only after a real service graph boundary exists. Phase 0 must not
publish placeholder graph metrics.

## Regression Gates

After each phase:

- Idle RSS must not grow continuously over a 5-minute run.
- Service graph retained count must return to the configured bound after
  in-flight requests complete.
- A no-op reload dry-run must not trigger tmux commands.
- A rejected reload must not trigger namespace mutation.
- A successful additive reload must not increase sidebar steady-state refresh
  work beyond the new configured-agent count.

## Design Implications

Avoid:

- Per-request mutexes around the entire service graph.
- Config file polling in heartbeat.
- Namespace patch chains that require replay on every project-view build.
- Unbounded pending unload/replace queues.
- Keeping old graphs alive after all requests have left the old version.

Prefer:

- Atomic or RCU-style current graph publication.
- Explicit reload commands over file watching.
- Bounded old graph retention.
- Diff dry-run before mutation.
- Metrics-driven release gates.
