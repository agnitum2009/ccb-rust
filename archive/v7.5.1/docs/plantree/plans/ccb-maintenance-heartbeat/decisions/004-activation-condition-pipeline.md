# Activation Condition Pipeline

Date: 2026-06-10

## Context

The heartbeat can activate `self` for more than one reason. A runtime state
check can find risk or unknown progress, while a scheduled follow-up can become
due after `self` asks for a later recheck.

Future CCB features may also need schedule-driven activation for other
configured agents. If heartbeat directly calls `ask --silence self` from each
condition branch, the design will be hard to reuse and hard to gate.

## Decision

Separate activation conditions from activation dispatch.

Condition producers observe runtime state or time and produce a normalized
`ActivationIntent`. V1 intentionally uses one intent structure instead of
separate condition and activation-record objects. The dispatcher validates
target authority, duplicate suppression, cadence, payload size, delivery mode,
and v1 target scope before sending any CCB message.

V1 implements this internally for heartbeat diagnostics only:

- state-check conditions can activate the configured assessor;
- due follow-up conditions can activate the same assessor after a fresh
  re-snapshot;
- condition and record fields stay collapsed into one `ActivationIntent`
  structure until a second producer or public scheduled-task surface requires a
  split;
- delivery uses `ask --silence`;
- public arbitrary-agent scheduled tasks remain deferred.

## Consequences

- `self` activation is the first use of a general activation pipeline, not a
  hard-coded special case.
- Future activation conditions can reuse the dispatcher without inheriting
  heartbeat-specific logic.
- Future target agents can be added only after explicit policy and command
  design.
- V1 remains narrow enough to validate heartbeat behavior safely.
