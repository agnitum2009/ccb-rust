# Independent Runner With Default Self Escalation

Date: 2026-06-10

## Context

The maintenance heartbeat must diagnose CCB agent health periodically without
becoming another daemon authority and without relying on a provider-side loop.
Programmatic checks can identify many runtime and communication failures, but
semantic judgment is still needed for ambiguous task execution, weak replies,
stuck callback chains, or degraded diagnostics.

## Decision

The heartbeat is a generic CCB feature implemented as a relatively independent
runner, exposed through a project-aware command family such as
`ccb maintenance ...`.

The runner reads ccbd diagnostics and CCB communication state, performs bounded
programmatic agent-health checks, and exits when the project is healthy and
idle. If it finds risk, diagnoses an unhealthy state, or cannot decide with
enough confidence, it sends a bounded diagnostic package to the configured
semantic assessor.

The default assessor is the project-local `ccb_self` agent when available, but
the target must remain configurable and must not be hard-coded into the
heartbeat engine.

In v1, the assessor may return report-only advice, user-escalation requests,
and validated schedule recommendations. Mutating repair actions such as
`clear`, `restart`, `repair`, `kill`, force cleanup, or restart-all are
deferred until an explicit autonomous repair policy exists.

## Consequences

- Heartbeat scheduling, locking, state writes, and tick execution remain CCB
  responsibilities.
- `ccb_self` supplies semantic supervision, but it is not the heartbeat
  process and does not become keeper, ccbd, lifecycle, or runtime authority.
- The runner must use an independent heartbeat lock namespace and must not
  share keeper or ccbd lifecycle locks.
- The diagnostics snapshot should reuse existing CCB diagnostics and
  communication surfaces instead of inventing a parallel collection path.
- Healthy idle projects do not wake a provider agent.
- Risk, unknown, and unhealthy states wake the assessor unless the assessor is
  busy, missing, degraded, or policy-disabled.
