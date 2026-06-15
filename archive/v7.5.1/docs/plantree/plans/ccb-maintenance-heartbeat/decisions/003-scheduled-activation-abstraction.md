# Scheduled Activation Abstraction

Date: 2026-06-10

## Context

Heartbeat has two activation conditions:

- runtime state checks can find risk, unknown progress, or unhealthy execution;
- scheduled follow-ups can become due after `self` requested a later
  diagnostic pass.

Both conditions need to send bounded information to an agent without turning
the heartbeat runner into a provider loop. The first target is usually
`self`/`ccb_self`, but the same scheduling capability may later need to send
tasks to other configured agents.

## Decision

Model heartbeat dispatch as a reusable CCB scheduled-activation abstraction.
In v1 the internal data shape is a single `ActivationIntent` that describes the
condition, trigger, target agent, delivery mode, deduplication key, schedule,
reason, and bounded payload or artifact reference.

Heartbeat state checks create `maintenance_diagnostic` activations for the
configured assessor. Delayed rechecks create `scheduled_followup` activations
that the runner validates against a fresh snapshot before dispatch. Future
schedule features may reuse the same envelope for bounded tasks to other
configured agents, after separate user-facing policy and command design.

In v1, activation delivery compiles to `ask --silence` for maintenance
diagnostics. The runner submits once and does not wait for a reply.

## Consequences

- Heartbeat avoids a hard-coded "send to self" implementation.
- Follow-up scheduling and state-triggered diagnostics share deduplication,
  minimum interval, payload-size, target-authority, and repeat-cap rules.
- V1 avoids over-splitting condition and record data; the split can happen
  later when another producer proves it is needed.
- Future schedule-to-agent features can reuse the same internal shape without
  inheriting heartbeat-specific semantics.
- Public arbitrary-agent scheduled tasks remain deferred until command syntax,
  permissions, payload policy, and result-dependency semantics are explicitly
  designed.
