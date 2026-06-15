# Implementation Slices

Date: 2026-06-10

## Purpose

Translate the current maintenance heartbeat plan into code entrypoints,
implementation sequence, test mapping, and blockers.

This topic records worker1's implementation analysis and the landing evidence
for the first safe slice.

## Code Entrypoints

Config parsing and validation:

- `lib/agents/config_loader_runtime/parsing_runtime/validation.py`
- `lib/agents/config_loader_runtime/common.py`
- `lib/agents/models_runtime/config_runtime/project.py`
- `lib/agents/config_loader_runtime/io_runtime/documents.py`

CLI command routing:

- `lib/cli/parser_runtime/constants.py`
- `lib/cli/parser_runtime/commands.py`
- `lib/cli/models_start.py`
- `lib/cli/parser.py`
- `lib/cli/phase2_runtime/dispatch.py`
- `lib/cli/phase2.py`
- `lib/cli/phase2_services.py`
- `lib/cli/router.py`

Daemon and socket surfaces:

- `lib/ccbd/app_runtime/handlers.py`
- `lib/ccbd/app_runtime/request_guard.py`
- `lib/ccbd/socket_server_runtime/server.py`
- `lib/ccbd/socket_client_runtime/endpoints.py`
- `lib/ccbd/app_runtime/service_graph.py`

Startup and daemon maintenance loop:

- `lib/ccbd/app_runtime/lifecycle.py`
- `lib/ccbd/app_runtime/bootstrap.py`

The existing daemon-internal maintenance worker must not become the heartbeat
long-lived runner in v1.

Diagnostics and project view:

- `lib/cli/services/doctor.py`
- `lib/cli/services/diagnostics_runtime/sources.py`
- `lib/ccbd/handlers/ping_runtime/payloads.py`
- `lib/ccbd/project_view/service.py`

Ask, queue, mailbox, and trace:

- `lib/cli/services/ask_runtime/submission.py`
- `lib/ccbd/handlers/submit.py`
- `lib/ccbd/services/dispatcher_runtime/submission_service.py`
- `lib/ccbd/handlers/`
- `lib/mailbox_kernel/`
- `lib/message_bureau/`

## Contract Priority

P0 before runtime writes:

- [../../../../ccb-config-layout-contract.md](../../../../ccb-config-layout-contract.md):
  define `[maintenance.heartbeat]` or equivalent config fields, defaults,
  units, validation, reload behavior, and whether CLI enable/disable may edit
  config.
- [../../../../ccbd-diagnostics-contract.md](../../../../ccbd-diagnostics-contract.md):
  define `.ccb/ccbd/maintenance-heartbeat/`, schedule/status/lock schema,
  support-bundle inclusion, and separation from daemon lease heartbeat plus
  `.ccb/ccbd/heartbeats/<subject-kind>/` job/subject evidence.

P1 before startup integration or dispatch:

- [../../../../ccbd-startup-supervision-contract.md](../../../../ccbd-startup-supervision-contract.md):
  define startup ensure semantics, optional failure reporting, `ccb kill`
  interaction, and v1's no-long-lived-runner boundary.
- `ActivationIntent` schema and internal sender identity for heartbeat
  `ask --silence` dispatch.

## PR Sequence

1. Contract and config read model.
   Add heartbeat config model and validation only. Do not write runtime state.
   Candidate tests: `test_v2_config_loader.py`,
   `test_v2_phase2_entrypoint.py`.

2. Namespace, store, and read-only status.
   Add heartbeat path layout, schedule/status models, corrupt-state handling,
   diagnostics bundle inclusion, and `ccb maintenance status`. Keep
   `enable/disable` mutation deferred until config authority is decided.
   Candidate tests: `test_maintenance_heartbeat_store.py`,
   `test_v2_diagnostics_bundle.py`, `test_v2_cli_router.py`.

3. One-shot tick snapshot without dispatch.
   Add daemon RPC and service graph integration for bounded snapshots and
   healthy/disabled/too-early/corrupt outcomes. Do not submit asks yet.
   Candidate tests: `test_maintenance_heartbeat_tick.py`,
   `test_ccbd_project_view.py`, message-bureau queue tests.

4. `ActivationIntent` and self dispatch.
   Allow only configured assessor/self target and `delivery_mode=ask_silence`.
   Use daemon dispatcher submit, not shell `ask`. Define sender identity before
   landing. Candidate tests: `test_maintenance_heartbeat_activation.py`,
   dispatcher and ask-service tests.

5. Startup ensure, schedule follow-up, dedup, and caps.
   Startup only refreshes schedule state and runs or arranges one-shot due
   ticks unless a runner lifecycle contract exists. Candidate tests:
   `test_ccbd_start_handler.py`, `test_v2_ccbd_start_flow.py`, source-runtime
   smoke with `/home/bfly/yunwei/ccb_source/ccb_test` from
   `/home/bfly/yunwei/test_ccb2`.

## First Safe Slice

Start with PR 1 plus the read-only parts of PR 2:

- contract updates;
- heartbeat config read model;
- `.ccb/ccbd/maintenance-heartbeat/` path helpers;
- schedule/status data model;
- read-only `ccb maintenance status`.

Reason: this fixes authority, naming, and diagnostics boundaries without
introducing automatic wakeups, provider turns, queue writes, or startup side
effects.

Landed on 2026-06-10:

- Updated config, diagnostics, and startup contracts for the maintenance
  heartbeat boundary.
- Added `[maintenance.heartbeat]` parsing/defaults and hybrid overlay support.
- Added `.ccb/ccbd/maintenance-heartbeat/` paths plus schedule/status models
  and corrupt/missing read results.
- Added read-only `ccb maintenance status`; at this initial landing,
  `tick|schedule|enable|disable` returned `not_implemented` and did not mutate
  state.
- Added diagnostics bundle inclusion for present `maintenance-heartbeat/*.json`
  files.

Verification:

- `python -m pytest -q test/test_maintenance_heartbeat.py test/test_v2_config_loader.py test/test_v2_diagnostics_bundle.py test/test_v2_cli_router.py`
  passed with `155 passed`.
- `python -m py_compile lib/agents/models_runtime/config_runtime/maintenance.py lib/maintenance_heartbeat/__init__.py lib/maintenance_heartbeat/models.py lib/maintenance_heartbeat/store.py lib/cli/services/maintenance.py`
  passed.
- `git diff --check` passed.
- From `/home/bfly/yunwei/test_ccb2`, isolated external
  `/home/bfly/yunwei/ccb_source/ccb_test --diagnose` passed and confirmed the
  source checkout was not used as a runtime project.
- From `/home/bfly/yunwei/test_ccb2`, isolated
  `/home/bfly/yunwei/ccb_source/ccb_test maintenance status` exited 0, reported
  missing schedule/status, and did not create
  `.ccb/ccbd/maintenance-heartbeat/`.
- From `/home/bfly/yunwei/test_ccb2`, isolated
  `/home/bfly/yunwei/ccb_source/ccb_test maintenance enable` exited 2 with
  `maintenance_status: not_implemented` and still did not create
  `.ccb/ccbd/maintenance-heartbeat/`.

## One-Shot Tick Snapshot Slice

Landed on 2026-06-10:

- Added a pure heartbeat classifier for `project_view` and local `ps` fallback
  snapshots.
- `ccb maintenance tick` now runs only when `[maintenance.heartbeat]` is
  enabled. Disabled tick reports `tick_status: disabled` and does not write
  status or schedule files.
- Enabled tick reads mounted daemon `project_view` first, falls back to local
  `ps` if the daemon/project-view read is unavailable, classifies
  `healthy|concern|failing|unknown`, and writes only
  `.ccb/ccbd/maintenance-heartbeat/status.json` plus `schedule.json`.
- Healthy tick uses `interval_s`; concern/failing/unknown use `min_interval_s`.
- Non-healthy tick records `recommended_action=assess_later`; it still does
  not dispatch `ask --silence`, run repairs, start providers, or mutate config.

Verification:

- `python -m pytest -q test/test_maintenance_heartbeat.py test/test_v2_config_loader.py test/test_v2_diagnostics_bundle.py test/test_v2_cli_router.py`
  passed with `159 passed`.
- `python -m py_compile lib/maintenance_heartbeat/__init__.py lib/maintenance_heartbeat/models.py lib/maintenance_heartbeat/store.py lib/maintenance_heartbeat/classifier.py lib/cli/services/maintenance.py`
  passed.

## Activation, Schedule, Lock, And Startup Ensure Slice

Landed on 2026-06-10:

- Added `MaintenanceHeartbeatActivation` JSONL audit records at
  `.ccb/ccbd/maintenance-heartbeat/activations.jsonl`.
- Added independent heartbeat file locking through
  `.ccb/ccbd/maintenance-heartbeat/lock.json`.
- Added `maintenance-heartbeat` as an internal non-mailbox CCB sender identity.
- `ccb maintenance tick` now respects persisted `schedule.json`, supports
  `--force` and `--no-dispatch`, and exits as `too_early` when the next run is
  in the future.
- Non-healthy due ticks validate the configured assessor, suppress active or
  recent duplicate maintenance activations, then submit one silent ask through
  daemon `submit` with a bounded diagnostic payload.
- `ccb maintenance schedule --after <duration> [--reason <text>]` persists the
  next CCB-owned heartbeat follow-up, enforcing `min_interval_s`.
- Unknown streaks that reach `unknown_streak_cap` back off to `interval_s` and
  set `needs_user=true`.
- Normal `ccb` startup now performs non-fatal due-tick ensure when heartbeat is
  enabled, `startup_ensure=true`, and the configured assessor exists.
- `enable/disable` remain config-authority in v1 and do not edit
  `.ccb/ccb.config`.

Verification:

- `python -m py_compile lib/maintenance_heartbeat/__init__.py lib/maintenance_heartbeat/models.py lib/maintenance_heartbeat/store.py lib/maintenance_heartbeat/lock.py lib/maintenance_heartbeat/classifier.py lib/cli/services/maintenance.py lib/cli/services/start.py lib/cli/services/start_runtime.py lib/mailbox_runtime/targets.py lib/mailbox_runtime/__init__.py lib/cli/render_runtime/ops_views_basic.py`
  passed.
- `python -m pytest -q test/test_maintenance_heartbeat.py` passed with
  `18 passed`.
- `python -m pytest -q test/test_v2_cli_router.py test/test_v2_diagnostics_bundle.py test/test_v2_start_service.py`
  passed with `70 passed`.
- `python -m pytest -q test/test_v2_config_loader.py test/test_v2_cli_render.py test/test_v2_phase2_entrypoint.py`
  passed with `182 passed`.
- `python -m pytest -q test/test_maintenance_heartbeat.py test/test_v2_config_loader.py test/test_v2_diagnostics_bundle.py test/test_v2_cli_router.py test/test_v2_cli_render.py test/test_v2_phase2_entrypoint.py test/test_v2_start_service.py test/test_v2_layout_plan.py test/test_v2_completion_tracker.py test/test_ccbd_project_view.py test/test_ccbd_project_focus.py test/test_ccbd_namespace_topology_plan.py`
  passed with `341 passed`.
- Full `python -m pytest -q` passed with `2518 passed, 2 skipped`.
- `git diff --check` passed.
- Isolated external `/home/bfly/yunwei/ccb_source/ccb_test --diagnose`,
  `maintenance status`, disabled `maintenance tick`, disabled
  `maintenance schedule`, and enabled temporary-project
  `schedule`/`tick`/`tick --force --no-dispatch` smoke tests passed from
  `/home/bfly/yunwei/test_ccb2` with isolated `HOME` and `CCB_SOURCE_HOME`.

## Schedule Consumer Runner Slice

Landed on 2026-06-11:

- Added `MaintenanceHeartbeatRunner` state under
  `.ccb/ccbd/maintenance-heartbeat/runner.json`.
- Added internal `ccb maintenance runner` as the project-scoped schedule
  consumer. It observes `schedule.json`, sleeps until due times with a bounded
  wake cap, and invokes the existing one-shot `maintenance tick` path.
- `ccb` startup ensure now starts or reuses one detached runner when heartbeat
  is enabled and the assessor is present.
- `ccb kill` best-effort signals the live runner and records stopped/stale
  state without blocking shutdown on runner residue.
- `maintenance status` and render output now expose runner state.
- Startup and diagnostics contracts now define runner lifecycle and
  `runner.json` diagnostics.

Verification:

- `python -m pytest -q test/test_maintenance_heartbeat.py` passed with
  `24 passed`.
- Focused start/kill/render/parser suite passed with `203 passed`.
- Full `python -m pytest -q` passed with `2529 passed, 2 skipped`.
- Isolated `/home/bfly/yunwei/test_ccb2` source validation confirmed startup
  starts the runner, repeated startup reuses it, and a due `next_run_at` is
  consumed without manual `maintenance tick`.

## Test Mapping

- Config enablement and validation: `test_v2_config_loader.py`.
- CLI parse/render: `test_v2_phase2_entrypoint.py`,
  `test_v2_cli_router.py`, `test_v2_cli_render.py`, or a new
  `test_maintenance_heartbeat_cli.py`.
- Schedule, lock, stale lock, corrupt state, and multi-project isolation: new
  `test_maintenance_heartbeat_store.py`.
- Idle/risk/unknown/unhealthy snapshots: new
  `test_maintenance_heartbeat_snapshot.py`.
- `ActivationIntent`, target scope, and `ask --silence` dispatch: new
  `test_maintenance_heartbeat_activation.py`, plus dispatcher/ask-service
  regression tests.
- Startup ensure, nonfatal runner failure, and kill concurrency:
  `test_ccbd_start_handler.py`, `test_v2_ccbd_start_flow.py`,
  `test_ccbd_socket_server_loop.py`.

## Risks And Blockers

- `ccb maintenance enable/disable` cannot safely mutate behavior until config
  authority is decided. If config is authority, these commands either edit
  `.ccb/ccb.config` through an explicit config-editing policy or remain
  disabled/status-only in v1.
- The configured assessor must resolve through effective config/current daemon
  graph, not a hard-coded role name.
- Internal `ask --silence` dispatch needs a CCB system sender identity such as
  `maintenance-heartbeat`; otherwise existing sender validation may reject it
  or misclassify lineage.
- Callback lineage, recent failures, and mailbox consistency need bounded
  snapshot sources before they drive automatic escalation.
- Do not run source runtime validation from `ccb_source`; use
  `/home/bfly/yunwei/ccb_source/ccb_test` from `/home/bfly/yunwei/test_ccb2`.
