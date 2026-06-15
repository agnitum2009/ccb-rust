# Config Enabled Startup And Silence Escalation

Date: 2026-06-10

## Context

The maintenance heartbeat should be available as a CCB runtime feature rather
than a role-only convention. Users need to opt into it from project
configuration, and when it is enabled the project should not depend on a manual
separate command to start health diagnostics.

At the same time, the heartbeat runner must remain separate from provider
conversation loops and must not block while waiting for semantic analysis.

## Decision

Effective `ccb.config` will provide the project-facing heartbeat enablement and
assessor policy. When heartbeat is enabled and the configured assessor exists,
normal `ccb` project startup should ensure the independent heartbeat runner is
started or scheduled for that project.

The runner periodically reads ccbd diagnostics and CCB communication state,
including agent execution status, queues, inboxes, pending replies, callbacks,
and mailbox-summary consistency. If all configured agents are healthy and idle,
the runner records success, schedules the next tick, and exits.

If work is unfinished and the runner cannot determine programmatically whether
the work is still running correctly, or if the runner detects risk or an
unhealthy state, it sends a bounded diagnostic package to the configured
assessor. In v1 the default transport is `ask --silence`, targeting `self` /
`ccb_self` when configured. The runner submits once and exits; it must not poll,
watch, or wait for the assessor reply.

If the assessor decides that a later diagnostic pass is needed, it registers a
CCB-owned follow-up through the sanctioned maintenance schedule surface. The
assessor must not directly ask itself, leave a provider-side loop running, or
write heartbeat schedule files. On the next due tick, the runner takes a fresh
snapshot and re-asks the assessor only if the condition is still unresolved,
ambiguous, or unhealthy.

## Consequences

- `.ccb/ccb.config` grammar and validation must be updated before
  implementation lands.
- Startup reporting must include heartbeat enablement and runner ensure status,
  but optional heartbeat failure should not fail ordinary project startup by
  default.
- `ask --silence` keeps the runner non-blocking and lets `self` activate its
  running-supervision skill in its normal provider context.
- Delayed rechecks become schedule/follow-up state owned by CCB, not nested
  self-recursive provider conversations.
- Duplicate escalation must be deduplicated by tick id, active maintenance job,
  or diagnostic fingerprint so an ambiguous task does not flood `self`.
- If heartbeat is enabled but the assessor is absent or degraded, the runner
  must fall back to programmatic diagnostics and user-visible status rather than
  looping provider activation attempts.
