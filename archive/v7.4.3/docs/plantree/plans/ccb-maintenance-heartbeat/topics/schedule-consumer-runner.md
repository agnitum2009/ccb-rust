# Schedule Consumer Runner

Date: 2026-06-11

Role: implementation plan
Status: landed
Read when: changing maintenance heartbeat startup, runner lifecycle, schedule
consumption, or real `ccb_test` validation.
Related:
[../decisions/005-project-scoped-schedule-consumer.md](../decisions/005-project-scoped-schedule-consumer.md),
[semantic-supervision-loop.md](semantic-supervision-loop.md),
[implementation-slices.md](implementation-slices.md)

## Purpose

Close the gap between persisted heartbeat schedule state and actual delayed
execution. The landed v1 can write `next_run_at`, and `ccb_self` can update it
through `ccb maintenance schedule`, but nothing consumes the schedule after
startup unless a user manually runs `ccb maintenance tick`.

## Validation Finding

On 2026-06-11, isolated source validation in
`/home/bfly/yunwei/test_ccb2` showed:

- `ccb_test maintenance tick --force` advanced `last_tick_at`, wrote
  `schedule.json`, and submitted a silent diagnostic activation to `ccb_self`.
- `ccb_self` completed the activation and changed `next_run_at` to a later
  follow-up time.
- After the new `next_run_at` passed, `last_tick_at` did not advance
  automatically.
- `ccbd` was mounted and healthy, queues were empty, and no
  `maintenance-heartbeat` or `maintenance tick` process was running.

Conclusion: the one-shot runner and assessor scheduling path work; the missing
piece is an automatic CCB-owned schedule consumer.

## Target Behavior

When heartbeat is enabled for a project:

1. Normal `ccb` project startup ensures exactly one heartbeat schedule
   consumer for that project.
2. The consumer reads effective config and the current heartbeat schedule.
3. If the schedule is in the future, it waits without waking providers.
4. When the schedule is due, it invokes the existing one-shot tick service.
5. The one-shot tick collects a fresh CCB snapshot, updates heartbeat status
   and schedule, and may submit one bounded silent assessor activation.
6. If `ccb_self` schedules a later follow-up, the same consumer observes that
   update and runs the next due tick.
7. When heartbeat is disabled, the project is killed, or the project authority
   changes, the consumer exits cleanly.

Healthy idle projects should not wake `ccb_self`. "Idle exit" means the tick
cycle exits without a provider activation; the lightweight schedule consumer
may keep sleeping so it can consume the next schedule without relying on an
external host timer.

## Physical Shape

Preferred first implementation:

- add an internal command or service path such as `ccb maintenance runner`
  or `ccb maintenance watch`;
- start it from the existing startup ensure path when heartbeat is enabled;
- keep the public documented surface as `ccb maintenance status|tick|schedule`
  until the runner is stable;
- record runner state under `.ccb/ccbd/maintenance-heartbeat/`;
- use the existing heartbeat lock before each tick so manual ticks and the
  consumer cannot race.

Avoid for this slice:

- provider-side loops inside `ccb_self`;
- direct self-to-self asks for delayed diagnosis;
- automatic host OS scheduler installation;
- moving health classification or activation dispatch out of the one-shot tick;
- making ccbd's internal daemon heartbeat loop the semantic maintenance
  scheduler.

## State And Files

Add a runner state record:

- path: `.ccb/ccbd/maintenance-heartbeat/runner.json`
- fields: `schema_version`, `record_type`, `project_id`, `runner_id`, `pid`,
  `started_at`, `last_seen_at`, `last_wake_at`, `last_tick_at`,
  `last_tick_status`, `observed_next_run_at`, `sleep_until`, `exit_reason`,
  and `source` such as `startup_ensure`, `manual`, or `test`.

Optional later evidence:

- bounded `runner-events.jsonl` for start, sleep, due, tick result, stale
  runner replacement, and clean exit events.

`status.json`, `schedule.json`, `lock.json`, and `activations.jsonl` keep their
current meanings. The runner must not write health status directly; it only
updates runner state and invokes the existing tick path.

## Lifecycle Rules

- Singleton: only one live consumer per project id and project anchor.
- Startup: if enabled and `startup_ensure=true`, `ccb start` ensures the
  consumer after the daemon is mounted and the assessor is present.
- Stale state: if `runner.json` points to a dead pid or mismatched project id,
  startup may replace it and record the replacement.
- Shutdown: `ccb kill` must stop or cause clean exit for the consumer. Shutdown
  must not block on a long sleep; the consumer needs a short wake cap or a
  project-lifecycle check.
- Config change: when heartbeat is disabled or the assessor becomes invalid,
  the consumer exits or reports degraded state. It must not edit
  `.ccb/ccb.config`.
- Crash tolerance: a crashed consumer leaves only diagnostics residue. The
  next `ccb start` or manual ensure can replace it.

## Consumption Algorithm

1. Load effective heartbeat config and runner state.
2. Exit if disabled, project id mismatches, or project lifecycle is no longer
   running.
3. Load `schedule.json`.
4. If no schedule exists, run one normal due tick so the tick can materialize
   initial schedule/status state.
5. If `next_run_at` is in the future, sleep until it is due, capped by a short
   maximum such as 30 seconds so shutdown/config changes are observed.
6. If `next_run_at` is due, invoke the same internal service as
   `ccb maintenance tick` without bypassing schedule, lock, duplicate
   suppression, or dispatch policy.
7. After the tick, loop back to observe the newly written schedule.
8. On repeated internal errors, back off to the configured normal interval or a
   bounded degraded interval and surface `needs_user=true` through status.

The consumer should not use `--force` for normal scheduled work. `--force`
remains a user/manual diagnostic override.

## Degraded Behavior

- ccbd mounted and healthy: use normal `project_view` snapshot through the
  existing tick path.
- ccbd unmounted or project-view unavailable: let the one-shot tick use its
  existing local `ps` fallback and classify `unknown` or degraded.
- assessor busy: rely on the existing duplicate suppression and active
  maintenance job checks.
- assessor missing or provider failure: do not self-loop; record degraded
  status and back off.
- repeated unknown/failing states: preserve existing `unknown_streak_cap`,
  `min_interval_s`, and `needs_user` policy.

## Implementation Slice

Code surfaces:

- `lib/cli/services/maintenance.py`: runner service, ensure/start helpers, and
  status merge.
- `lib/cli/services/start.py`: replace or extend startup ensure so it ensures
  the consumer.
- `lib/maintenance_heartbeat/models.py`: runner state model.
- `lib/maintenance_heartbeat/store.py`: runner state read/write and stale
  handling helpers.
- `lib/storage/paths_ccbd.py`: runner state path.
- `lib/cli/router.py` and parser files: hidden/internal runner command if
  needed.
- `docs/ccbd-startup-supervision-contract.md` and
  `docs/ccbd-diagnostics-contract.md`: runner lifecycle and diagnostics
  contract updates.

First code slice:

1. Add runner state model/store/status render with no background process.
2. Add an internal foreground runner command that consumes due schedules and
   can be tested with a bounded iteration count.
3. Wire startup ensure to spawn/ensure one detached project runner.
4. Wire shutdown/kill to stop or invalidate the runner safely.
5. Expand isolated `ccb_test` validation from `test_ccb2` to prove a
   `ccb_self` scheduled follow-up is consumed without manual `tick`.

## Landed Implementation

Landed on 2026-06-11:

- Added `.ccb/ccbd/maintenance-heartbeat/runner.json` path, model, store, and
  `maintenance status` rendering.
- Added internal `ccb maintenance runner` action with bounded foreground test
  options and an unbounded project-scoped schedule-consumer loop for startup.
- `ccb` startup now ensures one detached schedule consumer when heartbeat is
  enabled, `startup_ensure=true`, and the configured assessor is present.
- The runner consumes due `schedule.json` state and invokes the existing
  one-shot `maintenance tick` path; it does not classify health, submit asks,
  repair providers, or write heartbeat status directly.
- `ccb kill` now best-effort signals the live runner and records stopped/stale
  runner state without allowing runner residue to block shutdown.
- Startup and diagnostics contracts were updated for runner lifecycle and
  `runner.json` diagnostics.

Verification:

- `python -m py_compile lib/maintenance_heartbeat/models.py
  lib/maintenance_heartbeat/store.py lib/maintenance_heartbeat/__init__.py
  lib/storage/paths_ccbd.py lib/cli/services/maintenance.py
  lib/cli/services/kill.py lib/cli/render_runtime/ops_views_basic.py
  lib/cli/parser_runtime/commands.py`
- `python -m pytest -q test/test_maintenance_heartbeat.py` passed with
  `24 passed`.
- `python -m pytest -q test/test_maintenance_heartbeat.py
  test/test_v2_start_service.py test/test_v2_kill_service.py
  test/test_v2_cli_render.py test/test_v2_phase2_entrypoint.py
  test/test_v2_cli_router.py` passed with `203 passed`.
- Full `python -m pytest -q` passed with `2529 passed, 2 skipped`.
- Isolated real validation from `/home/bfly/yunwei/test_ccb2` with
  `/home/bfly/yunwei/ccb_source/ccb_test` and isolated `HOME` /
  `CCB_SOURCE_HOME` confirmed:
  - startup ensure reported `runner_status=started`;
  - due schedule was consumed without manual `maintenance tick`;
  - `last_tick_at` advanced automatically from `2026-06-11T04:03:33Z` to
    `2026-06-11T04:04:33Z`;
  - a second startup reported `runner_status=already_running` and only one
    runner process remained;
  - the runner later observed a healthy state and advanced to the normal
    interval with `runner_last_tick_status=healthy` and
    `next_run_at=2026-06-11T04:16:33Z`.

## Test Matrix

Unit and service tests:

- disabled heartbeat does not start the consumer;
- missing assessor reports degraded and does not start provider work;
- future `next_run_at` sleeps/skips without writing health status;
- due `next_run_at` invokes one-shot tick once;
- no schedule materializes an initial tick;
- stale/dead runner state is replaced;
- second runner instance exits or reports already running;
- manual `tick` and consumer tick cannot overlap because they share the
  heartbeat lock;
- runner state appears in `ccb maintenance status`;
- heartbeat-only config reload does not restart agent panes but updates runner
  policy;
- `ccb kill` stops the consumer or makes it exit promptly.

Isolated real validation:

- From `/home/bfly/yunwei/test_ccb2`, run source wrapper with isolated
  `HOME` and `CCB_SOURCE_HOME`.
- Start enabled heartbeat with `startup_ensure=true`.
- Verify `maintenance status` reports a live runner.
- Schedule a near follow-up through `ccb maintenance schedule --after 60s`.
- Wait past `next_run_at` without manual `tick`.
- Verify `last_tick_at` advances and a new schedule is written.
- Force a non-healthy condition that activates `ccb_self`; verify `ccb_self`
  can schedule a later follow-up and the consumer consumes it.
- Verify final queues drain and no duplicate runner remains.

## Out Of Scope

- Automatic mutating repair.
- Public scheduled tasks to arbitrary agents.
- Host OS scheduler installation.
- Provider-side loops or long-lived `ccb_self` turns.
- Direct schedule-file writes from assessor code.
- A ccbd-internal semantic maintenance scheduler that cannot diagnose ccbd
  unresponsiveness.
