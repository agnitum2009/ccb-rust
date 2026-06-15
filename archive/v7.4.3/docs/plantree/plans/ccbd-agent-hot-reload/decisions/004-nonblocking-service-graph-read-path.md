# Non-Blocking Service Graph Read Path

Date: 2026-05-29

## Context

Reload requires handlers to use the current config-bound services after a graph
swap. A naive handler wrapper could protect graph reads with a global mutex,
but sidebar refresh, project-view, ping, submit, and focus requests would then
compete with reload and with each other.

## Decision

Handler reads of the current service graph must be non-blocking in the steady
state, or must be proven by metrics not to contend. Reload may build and
publish a new graph under an exclusive mutation path, but ordinary request
handlers use the last fully-published graph.

Old graphs are retained only while in-flight requests still reference them and
must have observable bounded retention.

## Consequences

This prevents dynamic reload from making normal request CPU worse through lock
contention. It also forces graph lifetime and retained-count metrics to exist
before mutating reload is released.

Related topics:

- [execution-plan.md](../topics/execution-plan.md)
- [performance-baseline-and-gates.md](../topics/performance-baseline-and-gates.md)
