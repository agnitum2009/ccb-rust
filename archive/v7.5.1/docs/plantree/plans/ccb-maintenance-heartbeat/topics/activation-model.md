# Activation Model

Date: 2026-06-10

## Purpose

Define the generic activation layer behind the maintenance heartbeat.

The first shipped use case is simple: heartbeat conditions activate
`self` / `ccb_self` for semantic diagnostics. The model must still avoid
hard-coding "heartbeat sends to self" so future schedule features can activate
other configured agents through the same policy gates.

## Pipeline

```text
producer -> activation condition -> ActivationIntent -> dispatcher -> target agent
```

Roles:

- Producer: code that observes state or time. V1 producers are heartbeat state
  checks and due follow-ups.
- Activation condition: a bounded predicate that says an activation should be
  considered. It is a conceptual rule, not a separate persisted v1 data
  structure.
- `ActivationIntent`: the normalized handoff from condition evaluation to
  dispatch. V1 intentionally uses one internal structure instead of separate
  condition and record objects.
- Dispatcher: CCB policy layer that validates target authority, deduplication,
  rate limits, delivery mode, payload size, and v1 target scope before sending.
- Target agent: a configured current-graph agent. V1 target is the configured
  assessor, defaulting to `self` / `ccb_self`.

## Activation Conditions

V1 condition kinds:

- `heartbeat_state_check`: the runner sees unfinished work, risk, unhealthy
  state, or unknown progress from ccbd diagnostics and communication state.
- `scheduled_followup_due`: a previously registered follow-up is due and a
  fresh snapshot still shows the condition is unresolved, ambiguous, or
  unhealthy.

Future condition kinds can include:

- scheduled user task;
- stale callback or reply chain;
- repeated provider API failure;
- config/reload attention needed;
- external project event.

The condition evaluator must be side-effect-light. It may propose an
`ActivationIntent`, but it must not directly call `ask`, write target provider
state, or mutate runtime authority.

## ActivationIntent

`ActivationIntent` is the v1 internal structure. It carries both condition
metadata and dispatch request metadata so v1 does not over-engineer a
condition-to-record mapping before a second producer exists.

Conceptual fields:

- `activation_id`
- `status`: `submitted`, `suppressed`, `blocked`, or `failed`
- `condition_kind`
- `trigger_kind`
- `source`
- `observed_at`
- `target_agent`
- `delivery_mode`
- `payload_kind`
- `dedup_key` or `diagnostic_fingerprint`
- `not_before`
- `expires_at`
- `reason`
- `payload_ref`
- `evidence_refs`
- `created_by`
- `repeat_count`

V1 values:

- `target_agent`: configured assessor, default `self` / `ccb_self`
- `delivery_mode`: `ask_silence`
- `payload_kind`: `maintenance_diagnostic`
- `trigger_kind`: `state_check` or `scheduled_followup`
- `created_by` / sender identity: `maintenance-heartbeat`

Implemented v1 storage:

- `.ccb/ccbd/maintenance-heartbeat/activations.jsonl`
- each row records dispatch outcome, not authority
- `status=submitted` includes the submitted CCB `job_id`
- `status=suppressed` records `active_maintenance_job:*`,
  `recent_duplicate:*`, or `dispatch_disabled`
- `status=blocked` records missing configured assessor
- `status=failed` records daemon submit failure

## Dispatcher Policy

The dispatcher owns the last gate before message delivery:

- target must be a current configured daemon-graph agent;
- v1 target must be the configured assessor, default `self` / `ccb_self`;
- delivery must use `ask --silence`;
- duplicate activation must be suppressed by `dedup_key` and active maintenance
  job state;
- activation must honor minimum interval, unknown cap, and repeat limits;
- payload must stay bounded or use CCB artifact storage;
- target busy/missing/degraded cases must degrade to diagnostics rather than
  recursive activation.

This preserves a future route to scheduled tasks for other agents without
shipping arbitrary scheduled agent work in v1.

The shipped dispatcher uses mounted-daemon `submit(MessageEnvelope)`, not shell
`ask`. It sets `message_type=ask`, `silence_on_success=true`,
`delivery_scope=single`, and `from_actor=maintenance-heartbeat`.

## V1 Scope

V1 implements the abstraction internally but exposes only the heartbeat use
case:

- condition producers: heartbeat state check and due follow-up;
- data shape: one `ActivationIntent` structure;
- target: configured assessor only;
- default target: `self` / `ccb_self`;
- delivery: `ask --silence`;
- payload: maintenance diagnostic package;
- result dependency: none; runner submits once and exits.

## Split Later

Only split activation condition and activation record into separate stable
structures after a second producer or a public scheduled-task surface needs the
separation.

## Deferred

- Public CLI for arbitrary scheduled agent tasks.
- Callback/result-dependent scheduled activation.
- Multi-target fanout.
- Activation to non-agent tool windows.
- User-defined activation predicates.
