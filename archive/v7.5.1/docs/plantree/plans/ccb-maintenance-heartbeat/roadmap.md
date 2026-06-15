# CCB Maintenance Heartbeat Roadmap

Date: 2026-06-10

## Done

- Accepted the boundary correction: heartbeat scheduling, wake policy, and
  next-run state belong to CCB rather than to the `ccb_self` Role Pack.
- Promoted the external heartbeat idea from the general ideas inbox into this
  CCB-level planning root.
- Accepted the direction that the heartbeat should be an independent CCB
  program/helper that `ccb_self` can trigger or reschedule through a sanctioned
  skill or control-plane command.
- Accepted the user's correction that the heartbeat is generic CCB
  infrastructure: it periodically diagnoses all configured agents from ccbd and
  communication evidence, then escalates risk, unknown, or unhealthy states to
  the configured semantic assessor, defaulting to `ccb_self`.
- Accepted reviewer1's architecture feedback: avoid hard-coding `ccb_self`,
  prefer a hybrid `ccb maintenance ...` one-shot surface over ccbd/keeper
  internal ticks, keep the heartbeat lock independent from keeper/ccbd locks,
  and constrain v1 assessor actions to report-only advice.
- Accepted the startup/config direction: heartbeat enablement belongs in
  effective `ccb.config`; normal `ccb` project startup should ensure the
  independent runner when heartbeat is enabled and the configured assessor
  exists; ambiguous unfinished work is escalated to `self` with
  `ask --silence` so the runner does not block waiting for semantic analysis.
- Accepted the abstraction direction: activation conditions are separate from
  activation dispatch. Heartbeat state checks and scheduled follow-ups are the
  first condition producers; v1 uses one `ActivationIntent` structure and only
  activates `self`, while future conditions may target other configured agents.
- Accepted reviewer1's design review: clarify maintenance heartbeat namespace
  versus existing job heartbeat evidence, avoid long-lived runner lifecycle
  ambiguity in v1, collapse condition/record data into `ActivationIntent`,
  require a snapshot-field/read-path map, and expand integration tests.
- Accepted worker1's implementation analysis: land contracts, config parsing,
  namespace/store, and read-only status before any tick dispatch; keep
  `enable/disable` mutation, startup runner lifecycle, and internal
  `ask --silence` sender identity as explicit implementation decisions.
- Landed the first safe implementation slice: `[maintenance.heartbeat]`
  config parsing/defaults, `.ccb/ccbd/maintenance-heartbeat/` path helpers,
  schedule/status data models, read-only `ccb maintenance status`,
  diagnostics bundle inclusion, and reserved non-mutating parser entries for
  `tick|schedule|enable|disable`. Verified with targeted pytest, py_compile,
  `git diff --check`, and isolated external `ccb_test` smoke tests from
  `/home/bfly/yunwei/test_ccb2`.
- Landed the one-shot tick snapshot slice: `ccb maintenance tick` now runs only
  when heartbeat is enabled, reads bounded `project_view` evidence from the
  mounted daemon with local `ps` fallback, classifies `healthy|concern|failing|unknown`,
  writes only `maintenance-heartbeat/status.json` and `schedule.json`, shortens
  non-healthy cadence to `min_interval_s`, and still does not dispatch
  `ask --silence`, repair, or start providers.
- Landed the activation/schedule/startup ensure slice: non-healthy due ticks
  create a bounded `ActivationIntent` audit record, validate the configured
  assessor, suppress active or recent duplicate maintenance activations,
  submit one `ask --silence` through the mounted daemon dispatcher with
  `from_actor=maintenance-heartbeat`, then exit. `ccb maintenance schedule`
  writes the next CCB-owned follow-up time with `min_interval_s` enforcement.
  `ccb` startup now performs a non-fatal due-tick ensure when heartbeat is
  enabled, `startup_ensure=true`, and the assessor exists.
- Completed v1 verification: full `python -m pytest -q` passed with
  `2518 passed, 2 skipped`, `git diff --check` passed, and isolated
  `/home/bfly/yunwei/ccb_source/ccb_test` smoke tests from
  `/home/bfly/yunwei/test_ccb2` covered disabled status/tick/schedule plus an
  enabled temporary project schedule/too-early/force-no-dispatch flow.
- Landed post-review hardening for the next release: Codex unusable-pane
  detection now uses line-level terminal marker matching instead of broad
  substring matching; pure `[maintenance.heartbeat]` config diffs are
  classified and published as `maintenance_change` without tmux namespace
  mutation, runtime mount/unload, or agent pane restart; v1
  `escalation_policy` is documented as status-only. Verified with full
  `python -m pytest -q` (`2523 passed, 2 skipped`), reload/namespace targeted
  tests (`114 passed`), focused maintenance/Codex/reload tests, `git diff
  --check`, `py_compile`, and isolated `ccb_test --diagnose`, `config
  validate`, and `maintenance status` from `/home/bfly/yunwei/test_ccb2`.
- Accepted the self-supervision refinement: ambiguous execution-quality
  diagnosis should use real CCB-owned pane observation, starting with
  `tmux capture-pane` bottom/current text capture and short activity sampling.
  Screenshot or equivalent visual artifacts are fallback evidence when text is
  insufficient. Heartbeat passes target references; the `ccb_self` assessor
  requests pane evidence through sanctioned read-only tools.
- Verified the schedule-consumption gap in `/home/bfly/yunwei/test_ccb2`:
  manual `maintenance tick --force` updates status, submits `ccb_self`, and
  lets `ccb_self` reschedule a follow-up, but a due `next_run_at` is not
  consumed automatically without a background schedule consumer.
- Landed the project-scoped schedule consumer runner: startup ensure now starts
  or reuses one detached runner, `maintenance status` reports `runner.json`,
  the runner invokes the existing one-shot tick when schedules are due, and
  `ccb kill` best-effort signals it. Verified with targeted tests, full pytest,
  and isolated `/home/bfly/yunwei/test_ccb2` validation showing automatic
  `last_tick_at` advancement without manual `maintenance tick`.
- Recorded the v7.4.1 manual-test incident where Claude `Stop` hook attribution
  reused a stale `CCB_REQ_ID` after an interrupted request and scheduled task.
  The targeted fix now binds Claude hook completion to the current transcript
  turn, and project-view activity recognizes Claude scheduled-task and
  shell-running pane markers as provider work.
- Clarified that Codex is not on the same `Stop` hook completion path. Codex
  completion authority is the managed protocol/session event stream, so its
  supervision concerns are delivery-anchor absence, stale or rebound session
  binding, and pane/control-plane conflicts rather than stale hook `CCB_REQ_ID`
  reuse.
- Accepted the no-branching product constraint: do not add
  `mode = "aggressive"` or any equivalent heartbeat behavior mode. Active
  anomaly detection is part of the single default heartbeat classification path,
  with cadence, escalation target, and policy gates controlling noise and
  authority.
- Documented the current ask running/fault detection chain: dispatcher job
  state is authoritative for queued/running/terminal status; provider execution
  adapters and completion detectors produce terminal decisions; job heartbeat
  is a long no-progress terminal fuse; project-view and maintenance heartbeat
  are diagnostic/activation layers rather than completion authorities.
- Accepted the self-first supervision direction: ccbd should not become a
  fine-grained semantic judge. It should emit coarse, read-only suspicion
  envelopes for gray-zone evidence and activate `ccb_self`, which performs
  deeper pane/log/config/comms diagnosis and bounded repair through sanctioned
  CCB commands.
- Completed and reviewed the first suspicion-envelope slice in the default maintenance
  heartbeat classifier. It now emits a structured `suspicion_envelope` when
  provider/pane evidence says an agent is active but CCB has no active job or
  active communication for that agent, and when active/pending activity evidence
  is degraded. The envelope includes `condition_kind`, `control_state`,
  `provider_state`, `pane_ref`, `evidence_refs`, `confidence`, and
  `allowed_actions`; heartbeat activation messages instruct `ccb_self` to use
  only explicitly allowed actions.
- `archi`, `coworker`, and `reviewer1` reviewed Step 1 with no blockers.
  Follow-up suggestions were applied: added active-comms suppression coverage,
  top-level diagnostic `allowed_actions`, degraded-evidence classifier coverage,
  active degraded-evidence coverage, and unknown-streak E2E coverage.
- Validated the reviewed first suspicion-envelope slice with
  `python -m pytest -q test/test_maintenance_heartbeat.py` (`32 passed`) and
  adjacent hook/project-view coverage:
  `python -m pytest -q test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_ccbd_project_view.py test/test_maintenance_heartbeat.py`
  (`98 passed`). `py_compile` and `git diff --check` passed.
- Completed and reviewed Step 2: safe provider execution runtime state is now
  exposed as bounded `provider_runtime` snapshots on project-view agent rows.
  The classifier uses that evidence for `provider_runtime_without_control_job`
  and delayed `provider_delivery_pending_anchor` suspicion envelopes. This
  slice was reviewed by `archi`, `coworker`, and `reviewer1` with no blockers.
  Follow-up hardening was applied: dedup normalizes volatile provider-runtime
  timing/progress fields, project-view only attaches a runtime snapshot when
  it matches the current job id, and multiple orphan runtime snapshots are
  surfaced as a conflict summary rather than silently last-write-wins.
- Validated the final reviewed Step 2 state with
  `python -m pytest -q test/test_provider_execution_service_runtime.py test/test_v2_execution_service.py test/test_ccbd_project_view.py test/test_maintenance_heartbeat.py`
  (`160 passed`) and
  `python -m pytest -q test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_provider_activity_artifacts.py test/test_claude_hook_results.py test/test_ccbd_project_view.py test/test_maintenance_heartbeat.py`
  (`113 passed`). `py_compile` and `git diff --check` passed.
- Landed the empty hook-reply guard for hook-driven providers: Claude `Stop`
  hook writes `incomplete` with diagnostics when the assistant reply is empty,
  Claude/Gemini hook readers normalize legacy `completed` + empty-reply events
  to `incomplete`, and Gemini no longer ignores such events until timeout.
  Verified with
  `python -m pytest -q test/test_provider_finish_hook_script.py test/test_provider_hook_transcript.py test/test_claude_hook_results.py test/test_gemini_execution_hook.py test/test_claude_execution_polling.py test/test_v2_execution_service.py`
  (`90 passed`).
- Landed the Codex/protocol empty-reply guard: `task_complete` with no
  boundary reply and no prior assistant-visible reply evidence now terminalizes
  as `incomplete` with `empty_reply`, `empty_provider_reply`, and a readable
  diagnosis instead of normal completion. Existing cases with prior assistant
  chunks still complete normally. Verified with
  `python -m pytest -q test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_completion_orchestration.py test/test_codex_execution_polling.py test/test_v2_execution_service.py`
  (`82 passed`).
- Landed the Antigravity (`agy`) pane-quiet empty-reply guard: a visible
  requested done marker with no extracted assistant reply now terminalizes as
  `incomplete` with `empty_reply`, `empty_provider_reply`, and pane progress
  diagnostics instead of waiting for the long quiet timeout. Verified with
  `python -m pytest -q test/test_agy_execution_polling.py test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_completion_orchestration.py test/test_codex_execution_polling.py test/test_v2_execution_service.py test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_claude_hook_results.py test/test_gemini_execution_hook.py`
  (`106 passed`).
- Completed isolated source-runtime validation from
  `/home/bfly/yunwei/test_ccb2/ccb_empty_reply_real` using
  `/home/bfly/yunwei/ccb_source/ccb_test` with fake completion-family agents.
  `config validate`, startup, `ask`, `pend`, and `watch` succeeded. Empty-reply
  equivalent terminal reasons for `fake-codex`, `fake-claude`,
  `fake-gemini`, and `fake-legacy` all surfaced as `status: incomplete` with
  the expected reason rather than `completed`. `--silence` success stayed
  `completed`, `--silence` abnormal completion stayed diagnosable as
  `incomplete`, standalone `--callback` failed with the expected parent-job
  guard, active-parent callback produced the intentional
  `callback_pending` empty parent reply and then a normal callback
  continuation, and active-parent `--silence` fire-and-forget completed
  normally. Because the fake provider adapter materializes default reply text,
  true empty-string provider replies remain covered by the provider-specific
  pytest suite (`179 passed`).
- Completed dirty-diff review sync after the source-runtime validation:
  `reviewer1`, `archi`, and `coworker` all returned `PASS` with no blockers.
  The only shared plan-tree drift was that
  `topics/ask-runtime-health-mechanism.md` still described provider execution
  runtime state as absent from heartbeat evidence after Slice 2 had already
  added bounded `provider_runtime` snapshots. That topic has been synchronized
  to describe the current bounded-evidence behavior and remaining non-authority
  race/string-bound hardening follow-ups.

## In Progress

- None. The current implementation sequence requested by the user, Step 1 and
  Step 2 with review/test gates, is complete.

## Next

1. Define the `ccb_self` running-supervision skill input/output contract as
   the v1 default assessor implementation, including pane state,
   bottom/current capture references, activity sample references, and optional
   visual fallback evidence, plus a bounded action recommendation schema.
2. Add a public scheduled-activation surface for targets other than the
   configured assessor only after a second use case needs it.
3. Define config-edit policy if `ccb maintenance enable/disable` should ever
   mutate `.ccb/ccb.config` instead of staying config-only.

## V1 Readiness Blockers

1. None for the current code slice.

## Deferred

- Automatic mutating repairs beyond explicit low-risk policy.
- Always-on provider-side self loops.
- Project-wide shutdown or force cleanup from heartbeat logic.
- Multiple maintenance roles with arbitration.
- Automatic host OS scheduler installation; the next slice uses a CCB-owned
  project-scoped schedule consumer helper instead.
- Multi-assessor arbitration beyond the default `ccb_self` assessor.
- Public scheduled activation to arbitrary target agents.
- `ccb maintenance enable/disable` editing `.ccb/ccb.config`; v1 keeps
  enablement as config authority.
- Provider runtime snapshot hardening beyond the current whitelist, including
  maximum string bounds and explicit race-window policy for stale orphan
  snapshots after job terminalization.
