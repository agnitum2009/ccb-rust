# CCB Maintenance Heartbeat Plan

Date: 2026-06-10

## Purpose

Plan a generic CCB-owned maintenance heartbeat for independently diagnosing
configured-agent health, communication state, and task execution risk.

The heartbeat runner is a relatively independent CCB process or helper. It can
read `ccbd` diagnostics and CCB communication state, run periodic
agent-health checks, and exit when the project is healthy and idle. When it
finds risk, cannot decide, or diagnoses an unhealthy state, it sends a bounded
diagnostic package to a semantic assessor. The default assessor is `ccb_self`,
but the heartbeat target must remain configurable rather than hard-coded.

Heartbeat enablement is configured from effective `ccb.config`. When heartbeat
is enabled and the configured assessor exists, normal `ccb` project startup
should ensure the independent heartbeat runner is started or scheduled for that
project. The runner remains outside provider context and outside ccbd/keeper
lifecycle authority.

The feature belongs to CCB, not to the `agentroles.ccb_self` Role Pack. CCB
owns the runner, scheduler, project policy, wakeup state, locks, diagnostics
snapshot, dispatch to the assessor, schedule persistence, and next-run cadence.
The assessor owns semantic interpretation of the evidence and returns bounded
advice.

The heartbeat runner should be an independent CCB program or helper, not a
provider-side loop inside an agent conversation. `ccb_self` may trigger or
reschedule that runner through a sanctioned skill or CCB control-plane command,
but the runner owns scheduling, locking, state writes, and tick execution.

The dispatch piece should be abstracted as CCB scheduled activation. Heartbeat
uses it to activate `self` for diagnostics, but the same abstraction should be
usable later for schedule-driven tasks sent to other configured agents.

## File Map

- [roadmap.md](roadmap.md): planning sequence and current status.
- [open-questions.md](open-questions.md): unresolved product, safety, and
  implementation questions.
- [topics/semantic-supervision-loop.md](topics/semantic-supervision-loop.md):
  proposed wake, assess, and reschedule model.
- [topics/activation-model.md](topics/activation-model.md): generic
  activation condition, `ActivationIntent`, dispatcher, target-agent, and v1
  self-only scope.
- [topics/v1-slice-and-tests.md](topics/v1-slice-and-tests.md): first
  implementation slice and focused test matrix.
- [topics/snapshot-and-contract-gates.md](topics/snapshot-and-contract-gates.md):
  snapshot field/read-path mapping, namespace separation, and contract updates
  required before runtime implementation.
- [topics/implementation-slices.md](topics/implementation-slices.md): code
  entrypoint inventory, PR sequence, first safe slice, test mapping, and
  implementation blockers.
- [topics/schedule-consumer-runner.md](topics/schedule-consumer-runner.md):
  next slice for consuming persisted `schedule.json` automatically after
  startup instead of relying on manual `maintenance tick`.
- [topics/active-anomaly-and-hook-attribution.md](topics/active-anomaly-and-hook-attribution.md):
  incident-driven refinement for default active-anomaly escalation when provider
  hooks, protocol logs, pane state, and CCB control-plane state disagree.
- [topics/ask-runtime-health-mechanism.md](topics/ask-runtime-health-mechanism.md):
  current ask job running/fault detection chain and identified supervision
  gaps.
- [topics/self-first-health-supervision.md](topics/self-first-health-supervision.md):
  self-first supervision design that keeps ccbd conservative while making
  `ccb_self` the semantic diagnosis and bounded-repair fallback.
- [decisions/001-independent-runner-default-self-escalation.md](decisions/001-independent-runner-default-self-escalation.md):
  decision to use an independent CCB runner with configurable assessor
  escalation, defaulting to `ccb_self`.
- [decisions/002-config-enabled-startup-and-silence-escalation.md](decisions/002-config-enabled-startup-and-silence-escalation.md):
  decision to enable heartbeat from `ccb.config`, start it with normal `ccb`
  project startup when an assessor is present, and use `ask --silence` for
  non-blocking diagnostic activation.
- [decisions/003-scheduled-activation-abstraction.md](decisions/003-scheduled-activation-abstraction.md):
  decision to model heartbeat-to-self dispatch as a reusable scheduled
  activation envelope that can later target other agents.
- [decisions/004-activation-condition-pipeline.md](decisions/004-activation-condition-pipeline.md):
  decision to separate activation conditions from activation dispatch so future
  triggers and targets can reuse the same pipeline.
- [decisions/005-project-scoped-schedule-consumer.md](decisions/005-project-scoped-schedule-consumer.md):
  decision to add a CCB-owned project-scoped schedule consumer helper that
  consumes heartbeat schedules and invokes the existing one-shot tick.

## Related Sources

- [../ccb-self-role/README.md](../ccb-self-role/README.md)
- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../../../ccbd-lifecycle-stability-plan.md](../../../ccbd-lifecycle-stability-plan.md)
- [../../../ccbd-diagnostics-contract.md](../../../ccbd-diagnostics-contract.md)
- [../../../managed-provider-completion-reliability-plan.md](../../../managed-provider-completion-reliability-plan.md)

## Scope

In scope:

- A project-scoped external maintenance tick owned by CCB.
- An independent heartbeat runner that can be invoked by user commands,
  scheduler hooks, or a sanctioned maintenance skill.
- Programmatic CCB runtime and communication snapshots used as evidence.
- Configurable semantic-assessor escalation, defaulting to `ccb_self`.
- A `ccb_self` running-supervision skill as the first semantic assessor.
- Assessor-side real pane observation for ambiguous execution progress:
  heartbeat passes target references, while `ccb_self` starts with read-only
  `tmux capture-pane` style bottom/current text capture and activity sampling,
  using bounded screenshot artifacts only as fallback.
- A controlled schedule update surface for the next heartbeat time.
- `ccb.config` heartbeat enablement and assessor selection.
- Startup integration so normal `ccb` project startup ensures the independent
  runner when heartbeat is enabled and the assessor exists.
- `ask --silence` escalation to the default assessor for unfinished work whose
  runtime progress cannot be determined programmatically.
- A reusable scheduled-activation envelope for sending bounded tasks to a
  target agent by schedule or by runtime-state trigger.
- A generic activation-condition pipeline, with v1 limited to heartbeat
  conditions that activate `self` / `ccb_self`.
- Idle exit behavior so the heartbeat does not keep provider context alive.
- Conservative ambiguity handling that can temporarily shorten the next
  heartbeat interval without running destructive repair.
- v1 escalation to the assessor for risk, unknown, or unhealthy states without
  automatic mutating repair.

Out of scope:

- Making any assessor a daemon lifecycle, keeper, or runtime supervision
  authority.
- A provider-side infinite loop inside any assessor conversation.
- Direct schedule-file writes from assessor provider code.
- Replacing ccbd's normal configured-agent supervision.
- Adding a ccbd/keeper internal periodic tick that cannot survive the process
  it is meant to diagnose.
- Fully autonomous project-wide shutdown, force cleanup, restart-all, or broad
  repair.
- Continuing the original business task as `ccb_self` after another agent
  fails.
