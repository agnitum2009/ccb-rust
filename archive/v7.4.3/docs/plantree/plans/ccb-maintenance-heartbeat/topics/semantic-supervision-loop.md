# Semantic Supervision Loop

Date: 2026-06-10

## Boundary

The heartbeat is a generic CCB feature with a configurable semantic assessor.
The default assessor is `ccb_self`.

CCB owns:

- independent heartbeat runner and external tick entrypoint;
- effective-config heartbeat enablement and assessor selection;
- normal `ccb` project-start integration that ensures the runner when enabled;
- project-scoped heartbeat enablement and cadence policy;
- next-run state and locking;
- scheduled `ActivationIntent` state and dispatch validation;
- ccbd and communication snapshot collection;
- programmatic agent-health diagnosis;
- dispatch of risk, unknown, and unhealthy results to the configured assessor;
- validation of any schedule update or repair command;
- diagnostics records and user-visible status.

The semantic assessor owns:

- the running-supervision skill or equivalent assessor contract;
- semantic interpretation of CCB evidence;
- structured advice about health, confidence, suspected failure domain, and
  next useful cadence.
- optional skill-triggered requests to invoke or reschedule the independent
  heartbeat runner through sanctioned CCB commands.

The assessor must not own:

- keeper lifecycle;
- ccbd generation or lease authority;
- configured-agent runtime authority;
- direct file writes to schedule or authority records;
- the independent runner process lifetime;
- raw tmux mutation;
- project-wide shutdown or destructive repair.

## Why Programmatic Checks Are Not Enough

Programmatic checks can answer whether a queue is empty, a pane exists, a job
has a heartbeat timeout, or a mailbox summary is missing. They cannot reliably
answer all execution-quality questions.

Semantic supervision is useful for cases such as:

- an agent says a task is done but no expected evidence, tests, or artifacts are
  present;
- a reply exists but appears to be a refusal, prompt confusion, or partial
  answer rather than the requested result;
- a callback chain is technically terminal but the parent task cannot resume
  from the result;
- logs show repeated provider/tool errors that are not yet expressed as a
  single terminal CCB failure;
- an agent is idle but the current plan or user request still implies unfinished
  work;
- diagnostics are degraded enough that CCB cannot decide whether the state is
  healthy.

The design should use cheap programmatic filters first and reserve semantic
assessor provider work for risk, unhealthy, or ambiguous states.

## Proposed Tick Flow

1. An external scheduler, user command, or sanctioned assessor request
   invokes the independent heartbeat runner for one project-scoped maintenance
   tick.
2. The tick acquires a heartbeat lock that is independent from keeper and ccbd
   lifecycle locks, then reads CCB-owned schedule policy.
3. If heartbeat is disabled, too early, or another maintenance tick is active,
   it exits.
4. CCB collects a bounded snapshot from existing diagnostics and communication
   surfaces: `ps`, queue summaries, pending inbox/reply counts, active job
   ages, recent terminal failures, fault rules, degraded diagnostics, and
   relevant heartbeat timeout evidence.
5. If the snapshot is clearly idle and healthy, CCB records `last_ok`, advances
   `next_run_at` using the normal interval, and exits without waking
   the assessor.
6. If the snapshot has risk, is unhealthy, or cannot be classified with enough
   confidence, CCB sends a bounded diagnostic package to the configured
   assessor, default `ccb_self`. In v1 this dispatch uses `ask --silence` so
   the heartbeat runner does not wait for the provider result.
7. The assessor analyzes trace, artifacts, logs, and current plan/task context
   as needed, then returns structured advice. In v1 the advice is report-only
   or asks for user input; mutating repair is deferred.
8. CCB validates any schedule recommendation, persists the accepted next
   cadence and diagnostics, and surfaces user-visible status when needed. The
   provider turn ends; there is no provider-side infinite loop.

## Independent Runner And Trigger Surfaces

The runner should be independently invokable. Candidate surfaces:

- user/manual: `ccb maintenance tick`, `ccb maintenance status`,
  `ccb maintenance schedule --after <duration>`, `ccb maintenance enable`, and
  `ccb maintenance disable`;
- scheduler: cron, systemd timer, launchd, Windows Task Scheduler, or a future
  CCB installer-managed timer calls the same one-shot tick surface;
- assessor: a running-supervision skill or recovery skill may request
  `schedule-next`, `run-now`, or `enable/disable` through the same control-plane
  surface.

The important rule is that the assessor can request a heartbeat action but
should not be the heartbeat process. The independent runner validates policy,
deduplicates active ticks, writes the schedule state, invokes CCB diagnostics,
and decides whether a provider turn is needed.

## Config And Startup

Heartbeat enablement belongs in effective `ccb.config`, not in role memory and
not in provider state. A planned shape can be refined before implementation,
but the config needs at least:

- enabled/disabled;
- assessor target, defaulting to `ccb_self` or `self` when configured;
- normal interval;
- minimum interval;
- unknown-streak cap;
- whether risk/unknown/unhealthy states should escalate by `ask --silence`.

Normal `ccb` project startup should ensure the independent heartbeat runner
when heartbeat is enabled and the configured assessor exists. "Ensure" means
refresh schedule state and run a one-shot due tick or arrange a supported
one-shot schedule for the project without making it keeper, ccbd, or provider
runtime authority. V1 should not introduce a long-lived supervised runner
process unless the startup contract first defines who owns its lifecycle.
Runner launch failures should be diagnostics and status evidence unless the
user explicitly requires maintenance heartbeat as a hard startup gate.

If heartbeat is enabled but no assessor exists, startup should report a config
diagnostic and the runner should fall back to programmatic checks only.

## Escalation To Self

The runner escalates to the configured assessor when it sees unfinished work and
cannot determine whether the work is still running correctly. The diagnostic
package should include the job, target agent, mailbox state, active/pending
reply state, callback state, recent trace/artifact references, and the exact
reason programmatic classification was inconclusive.

V1 dispatch uses `ask --silence <assessor>` so the runner can submit the
diagnostic activation and exit. It must not poll, watch, or wait for the
assessor result. The assessor's running-supervision skill can then inspect
deeper evidence and, through sanctioned CCB commands, schedule the next
heartbeat or request user action.

The runner should not capture pane contents or screenshots on every tick. When
progress is ambiguous, it should pass enough target references for the assessor
to request real pane observation through sanctioned read-only tools. The default
`ccb_self` assessor should prefer a pane-text-first evidence ladder for
ambiguous execution-quality questions: pane metadata, bottom/current
`tmux capture-pane` text, recent scrollback, short activity sampling, and then
a bounded CCB-owned pane screenshot or equivalent visual artifact only when
text is insufficient.

## Delayed Follow-Up

Because escalation uses `ask --silence`, the runner will not receive a normal
reply from the assessor. If `self` decides that the right action is "check
again later", it must not try to ask itself directly or keep a provider-side
loop alive. Instead, `self` should register a CCB-owned follow-up through a
sanctioned schedule command, for example:

- `ccb maintenance schedule --after <duration> --reason <text>`
- a future structured equivalent that includes `followup_id`,
  `diagnostic_fingerprint`, target job/agent, and assessor target

The control plane validates the request, writes heartbeat schedule/follow-up
state, and enforces minimum interval, duplicate suppression, unknown caps, and
backoff. When the next tick is due, the independent runner collects a fresh
snapshot first:

- if the task is now healthy, completed, or no longer ambiguous, the runner
  resolves the follow-up without waking `self`;
- if the same unfinished or ambiguous condition remains, the runner sends a new
  bounded `ask --silence` diagnostic activation to `self`;
- if repeated follow-ups hit the configured cap, the runner stops shortening the
  interval and surfaces `needs_user=true`.

This keeps "ask self later" as a CCB scheduling concern rather than a nested
provider conversation or a self-recursive ask chain.

## Scheduled Activation Abstraction

Heartbeat should use a generic CCB activation dispatcher rather than a
heartbeat-specific "send to self" path. The v1 abstraction is a bounded
`ActivationIntent` that can be created by runtime diagnosis, explicit schedule,
user command, or a sanctioned agent request.

Conceptual fields:

- `activation_id`
- `condition_kind`
- `trigger_kind`: `state_check`, `scheduled_followup`, `user_request`, or
  future project-defined trigger kinds
- `target_agent`
- `delivery_mode`: v1 defaults to `ask_silence`
- `payload_kind`: `maintenance_diagnostic`, `scheduled_task`, or future
  bounded task type
- `dedup_key` or `diagnostic_fingerprint`
- `not_before` and optional expiration
- `reason`
- bounded payload or artifact reference
- policy gates such as minimum interval, target availability, and max repeats

Heartbeat's state-check path creates a `maintenance_diagnostic`
`ActivationIntent` for the configured assessor when it cannot classify
progress. The delayed follow-up path creates a `scheduled_followup`
`ActivationIntent` that the runner rechecks before dispatch. A future scheduler
can use the same envelope to send a scheduled task to another configured agent,
as long as it passes the same target authority, deduplication, payload-size,
and policy gates.

This abstraction should not bypass normal CCB message/job authority. It should
compile down to supported CCB delivery surfaces such as `ask --silence` in v1,
with callback or result-dependent delivery left for a later explicit design.

## Assessor Command Surface

For v1, the default `ccb_self` assessor should have a narrow sanctioned surface:

- `status`: allowed as read-only.
- `schedule-next`: allowed when the requested delay satisfies the minimum
  interval and includes a reason.
- `schedule-follow-up`: allowed when tied to the current diagnostic fingerprint
  or target job/agent and when it satisfies duplicate and cap rules.
- `schedule-activation`: future generalized form for scheduling a bounded
  activation to a configured target agent after policy validation.
- `run-now`: restricted; use only for explicit user requests or high-confidence
  concern/failing results after CCB validation.
- `enable` / `disable`: user policy only in v1; the assessor may recommend
  these actions but must not apply them autonomously.

Forbidden in v1:

- direct writes to heartbeat schedule files;
- direct writes to scheduled `ActivationIntent` state;
- immediate zero-delay self loops;
- direct `ask` from `self` to itself as the delayed follow-up mechanism;
- `clear`, `restart`, `repair`, `kill`, or force cleanup as automatic
  heartbeat actions;
- direct raw tmux mutation.

## Running-Supervision Skill Result

The skill should produce a compact structured result:

- `health`: `healthy`, `concern`, `failing`, or `unknown`
- `confidence`: `high`, `medium`, or `low`
- `domains`: queue, inbox, callback, provider, pane, config, task semantics,
  diagnostics, or unknown
- `evidence`: short references to command outputs, traces, artifacts, or plan
  files
- `pane_state`: optional assessor classification such as `working`,
  `waiting_input`, `stale_prompt`, `provider_update`, `provider_error`,
  `dead_or_blank`, `misframed`, or `unknown`
- `pane_capture`: optional bottom/current prompt capture, recent scrollback, or
  activity-sample artifact references
- `visual_evidence`: optional screenshot or OCR artifact references when text
  capture was insufficient
- `recommended_action`: v1 values are `report_only` or `ask_user`; repair
  values are deferred until an explicit autonomous repair policy exists
- `next_heartbeat_after`: desired delay with reason
- `needs_user`: true when the result is unsafe to repair autonomously

The CCB control plane must validate this result before changing schedule or
running any command.

## Snapshot Contract

The snapshot sent to the assessor should be small, derived from existing CCB
diagnostics and communication state, and prefer references over raw logs. Target
size for v1 is roughly 2 KiB.

Minimum useful fields:

- tick id, timestamp, trigger, project anchor, and configured assessor;
- diagnostic fingerprint or follow-up id when the tick is a delayed recheck;
- ccbd mounted/alive state, generation, degraded diagnostics, and recent
  startup/shutdown failure summaries;
- configured agents with provider, mounted/bound state, runtime health, queue
  depth, pending inbox count, pending reply count, and active job age;
- mailbox summary and communication-ledger consistency status for configured
  agents when available;
- recent terminal failures with job id, target, reason, and timestamp;
- active callbacks or pending replies that could block progress;
- target pane/window references sufficient for the assessor to request
  read-only `tmux capture-pane` style text capture, activity sampling, or
  screenshot fallback through CCB-owned tools;
- whether programmatic evidence conflicts with provider/pane evidence or
  suggests a visual check;
- fault-injection rules when present;
- last heartbeat result and unknown/failing streak counters;
- artifact, trace, or diagnostics references for any included failure.

The runner should not create an unrelated diagnostics collection path when an
existing `doctor`, `ps`, queue, trace, lifecycle, or communication-ledger source
already provides the field.

## Schedule State

Use a dedicated heartbeat namespace under `.ccb/ccbd/`, separate from keeper
and daemon lifecycle authority. Candidate path:

- `.ccb/ccbd/maintenance-heartbeat/schedule.json`

Do not store maintenance heartbeat schedule or lock state under
`.ccb/ccbd/heartbeats/`; that namespace is reserved for non-lease subject/job
heartbeat evidence.

The schedule file may contain `enabled`, `interval_s`, `min_interval_s`,
`next_run_at`, `last_tick_at`, `last_health`, `unknown_streak`,
`failing_streak`, `assessor`, `last_reason`, and a bounded set of active
follow-up records or references.

Heartbeat observations are diagnostics evidence, not lifecycle authority.
Healthy ticks should not spam `lifecycle.jsonl`. Failing or user-actionable
ticks may append a bounded lifecycle event for support visibility, but
`lifecycle.jsonl` must not become the scheduler state store.

The heartbeat lock should live in the same dedicated namespace and must not
reuse keeper or ccbd lifecycle locks.

## Cadence Policy

Suggested first policy:

- `healthy`: use a long normal interval and do not wake the assessor.
- `concern`: send the diagnostic package to the assessor and shorten interval
  moderately if validated.
- `failing`: send the diagnostic package to the assessor, record user-visible
  status, and run only policy-allowed report-only actions in v1.
- `unknown`: send the diagnostic package to the assessor, shorten interval
  briefly, cap repeated unknowns, then escalate to the user or back off.

The "unknown means check sooner" behavior matches the user's intent, but it
needs caps so bad diagnostics or provider confusion cannot create an infinite
self-wakeup loop.

Suggested first cap: after three consecutive `unknown` results, restore the
normal interval and set `needs_user=true`.

## Edge Cases

- Assessor busy: if a recent maintenance task is already queued or running for
  the assessor, skip duplicate wakeup and record the existing task reference.
- Assessor missing: run programmatic checks only; healthy exits normally,
  non-healthy becomes user-visible diagnostics.
- Assessor degraded or provider API failure: do not wake it repeatedly; record
  degraded reason, back off, and escalate after a small capped streak.
- Project idle: idle means ccbd is mounted, no active jobs, no pending
  replies/inboxes, no recent terminal failure inside the configured window, and
  no degraded agent. Idle records `last_ok`, advances the long interval, and
  exits.
- Stale heartbeat lock: detect via lock age and tick id; release only according
  to the heartbeat runner's own stale-lock rule, not keeper or ccbd lock rules.
