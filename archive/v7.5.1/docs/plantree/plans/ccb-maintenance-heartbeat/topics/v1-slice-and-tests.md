# V1 Slice And Tests

Date: 2026-06-10

## V1 Goal

Ship the smallest useful CCB maintenance heartbeat that is independently
invokable, diagnoses configured-agent health from ccbd and communication
evidence, exits when healthy/idle, and escalates risk, unknown, or unhealthy
states to the configured semantic assessor, defaulting to `ccb_self`.

V1 must not perform automatic mutating repair.

## Included

- `ccb maintenance tick`: one-shot runner entrypoint with project discovery and
  optional explicit project root.
- `ccb maintenance status`: read current schedule and last tick state.
- `ccb maintenance schedule --after <duration> --reason <text>`: validated
  schedule update.
- `ccb maintenance enable` / `disable`: reserved user-facing policy commands
  only; v1 returns `not_implemented` and keeps `.ccb/ccb.config` as the
  enablement authority.
- Effective `ccb.config` heartbeat enablement and configured assessor lookup.
- Normal `ccb` startup ensures the independent runner when heartbeat is enabled
  and the configured assessor exists by refreshing schedule state and running
  or arranging a one-shot due tick. V1 does not add a long-lived supervised
  runner process unless a startup lifecycle contract is added first.
- Independent heartbeat lock under the heartbeat namespace, with stale-lock
  detection separate from keeper and ccbd lifecycle locks.
- Schedule state under a dedicated namespace such as
  `.ccb/ccbd/maintenance-heartbeat/schedule.json`.
- Programmatic snapshot from existing ccbd diagnostics and CCB communication
  state.
- Healthy idle detection that records `last_ok`, schedules the next normal
  interval, and does not wake the assessor.
- Escalation to the configured assessor for risk, unknown, or unhealthy states,
  using `ask --silence`, a bounded diagnostic package, and a dedup key.
- V1 validates and reports `escalation_policy = "report_only" | "ask_user"`,
  but the field is status-only. It does not branch dispatch behavior in v1;
  all non-healthy activations use one bounded silent ask to the configured
  assessor.
- Assessor-requested delayed follow-up through a validated schedule command,
  not through direct self-to-self ask or a provider-side loop.
- A minimal internal `ActivationIntent` envelope for heartbeat diagnostic
  dispatch, with fields that can later support scheduled tasks to other agents.
- A minimal activation-condition model where heartbeat state checks create
  `ActivationIntent`-style activation records and dispatch only to the
  configured assessor.

## Deferred From V1

- Automatic repair actions: `clear`, `restart`, `repair`, `kill`, force
  cleanup, restart-all, or window-level restart.
- Autonomous assessor enable/disable of heartbeat policy.
- Automatic external scheduler installation.
- Multi-assessor arbitration.
- General user-facing scheduled tasks to arbitrary agents.
- Non-heartbeat activation condition producers.
- Rich `ccb doctor` or sidebar UI integration beyond a minimal status summary.
- Provider-side loops or long-lived assessor turns.
- Direct self-to-self ask chains for delayed diagnosis.
- Policy-specific dispatch behavior for `escalation_policy`.
- Hot reload paths that mutate tmux namespace or restart agents for
  maintenance-only config changes.

## Contract Update Gate

Before implementing files, config fields, startup behavior, or status fields:

- update [../../../../ccb-config-layout-contract.md](../../../../ccb-config-layout-contract.md)
  with heartbeat config fields, defaults, validation, and reload behavior;
- update [../../../../ccbd-diagnostics-contract.md](../../../../ccbd-diagnostics-contract.md)
  with `.ccb/ccbd/maintenance-heartbeat/`, schedule schema, lock/status files,
  support-bundle inclusion, and the distinction between daemon lease heartbeat,
  subject/job heartbeat evidence, and maintenance heartbeat scheduling;
- update [../../../../ccbd-startup-supervision-contract.md](../../../../ccbd-startup-supervision-contract.md)
  with startup ensure semantics, optional runner failure reporting, and `ccb
  kill` interaction;
- complete [snapshot-and-contract-gates.md](snapshot-and-contract-gates.md)
  with a field-to-read-path map and close any required diagnostics gaps.

## Delivered Test Matrix

- `test_maintenance_heartbeat_paths_use_dedicated_namespace`: heartbeat state
  lives under `.ccb/ccbd/maintenance-heartbeat/`.
- `test_maintenance_heartbeat_store_round_trips_and_reports_missing` and
  `test_maintenance_heartbeat_store_reports_corrupt_files`: schedule, status,
  activation, missing, and corrupt-state read paths are covered.
- `test_maintenance_parser_accepts_status_and_reserves_mutating_actions`,
  `test_phase2_maintenance_enable_disable_are_config_authority`, and
  `test_maintenance_status_rejects_reserved_mutating_actions`: v1
  `enable/disable` stay reserved and config-owned.
- `test_maintenance_status_reads_config_and_missing_state`,
  `test_phase2_maintenance_status_outputs_read_only_status`, and
  `test_render_maintenance_status_includes_config_and_state`: status/render
  surfaces expose config, schedule, status, and last activation state.
- `test_maintenance_tick_disabled_does_not_write_status_or_schedule`,
  `test_maintenance_tick_healthy_project_view_writes_status_and_schedule`,
  `test_maintenance_tick_concern_shortens_next_schedule`, and
  `test_maintenance_tick_falls_back_to_local_ps_when_project_view_unavailable`:
  disabled, healthy, concern, and local-ps fallback paths are covered.
- `test_maintenance_schedule_persists_followup_and_enforces_min_interval`,
  `test_maintenance_tick_exits_when_schedule_is_not_due`, and
  `test_maintenance_tick_force_no_dispatch_bypasses_schedule_without_submit`:
  schedule, too-early, force, and no-dispatch behavior are covered.
- `test_maintenance_tick_suppresses_recent_duplicate_activation` and
  `test_maintenance_tick_reports_lock_busy`: duplicate activation and active
  lock handling are covered.
- `test_reload_plan_classifies_maintenance_only_change`,
  `test_additive_reload_apply_maintenance_change_publishes_without_namespace_or_runtime_mutation`,
  and `test_project_reload_non_dry_run_maintenance_change_publishes_policy`:
  config changes while ccbd is mounted update heartbeat policy through explicit
  `maintenance_change` reload semantics.
- `test_maintenance_status_reports_corrupt_state_as_degraded`: corrupt stored
  state is user-visible and degraded.

## Deferred Test Ideas

- `test_maintenance_tick_stale_lock`: stale heartbeat lock cleanup needs a
  dedicated stale-pid rule before implementation.
- `test_maintenance_assessor_degraded`: repeated degraded-assessor backoff can
  be added after assessor health has a stable dedicated signal.
- `test_maintenance_kill_concurrent`: kill/tick concurrency should be covered
  when heartbeat operations become long-running enough to create real overlap.
- `test_maintenance_diagnostics_read_race`: diagnostics read-race tests can be
  added when the snapshot surface grows beyond the current bounded
  `project_view`/`ps` read path.
- `test_maintenance_followup_conflict`,
  `test_maintenance_activation_condition_due_followup`,
  `test_maintenance_assessor_schedules_followup`,
  `test_maintenance_followup_resolves_without_wakeup`,
  `test_maintenance_followup_reasks_when_still_ambiguous`, and
  `test_maintenance_followup_cap_escalates_user`: deferred until follow-up
  activations become first-class records rather than schedule-only state.
- `test_maintenance_activation_target_scope_v1`: deferred because v1 already
  uses a configured assessor target and no public arbitrary-target scheduler.
- `test_maintenance_enable_disable`: deferred until config editing policy
  allows these commands to mutate `.ccb/ccb.config`.
