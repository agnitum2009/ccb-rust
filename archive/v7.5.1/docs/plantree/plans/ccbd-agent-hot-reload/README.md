# CCBD Agent Hot Reload Plan

Date: 2026-05-28

## Purpose

Plan true agent dynamic loading for `ccbd`: after `.ccb/ccb.config` changes,
the running daemon should load, unload, and eventually replace agents without
killing, restarting, or rebinding unrelated running agent panes.

The implementation remains phased. Additive load is the first mutating target;
unload and replace are planned behind dry-run diffing, bounded draining, and
performance gates.

General `ccbd` throughput optimization is not the primary feature goal here,
but baseline CPU/memory measurement is an entry gate because the current daemon
already has user-reported high resource use.

## File Map

- [roadmap.md](roadmap.md): implementation sequence and current status.
- [open-questions.md](open-questions.md): unresolved questions only.
- [topics/current-runtime-boundaries.md](topics/current-runtime-boundaries.md):
  current startup, keeper, config, supervision, and namespace constraints.
- [topics/non-disruptive-hot-load-design.md](topics/non-disruptive-hot-load-design.md):
  proposed reload classes, flow, invariants, and failure handling.
- [topics/execution-plan.md](topics/execution-plan.md): phased engineering
  plan, module ownership, exit criteria, and rollback rules.
- [topics/performance-baseline-and-gates.md](topics/performance-baseline-and-gates.md):
  metrics, baselines, and resource gates that must hold before mutating reload.
- [topics/dynamic-unload-and-replace.md](topics/dynamic-unload-and-replace.md):
  draining, retiring, pending replacement, and bounded failure behavior.
- [topics/test-matrix.md](topics/test-matrix.md): automatic and manual test
  coverage for additive reload and unsafe changes.
- [topics/phase-6-additive-apply-design.md](topics/phase-6-additive-apply-design.md):
  Phase 6a transaction order, rollback diagnostics, reusable APIs, and required
  narrow APIs for additive mutating reload.
- [decisions/001-additive-hot-load-first.md](decisions/001-additive-hot-load-first.md):
  decision to support additive changes first and reject disruptive diffs.
- [decisions/002-rebuild-config-bound-services-on-reload.md](decisions/002-rebuild-config-bound-services-on-reload.md):
  decision to rebuild config-bound service objects instead of mutating all
  existing objects in place.
- [decisions/003-explicit-reload-before-watchers.md](decisions/003-explicit-reload-before-watchers.md):
  decision to start with explicit reload and dry-run, not file watching.
- [decisions/004-nonblocking-service-graph-read-path.md](decisions/004-nonblocking-service-graph-read-path.md):
  decision to keep handler graph reads non-blocking in the steady state.
- [decisions/005-bounded-drain-and-pending-replace.md](decisions/005-bounded-drain-and-pending-replace.md):
  decision to bound draining and pending replacement before dynamic unload or
  replace is exposed.

## Related Sources

- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../../../ccbd-project-namespace-lifecycle-plan.md](../../../ccbd-project-namespace-lifecycle-plan.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../ccbd-pane-recovery-continuous-attach-plan.md](../../../ccbd-pane-recovery-continuous-attach-plan.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)
- [../../baseline/storage-and-state.md](../../baseline/storage-and-state.md)
- [../../baseline/test-and-release-gates.md](../../baseline/test-and-release-gates.md)

## Scope

In scope:

- `ccb reload` or equivalent daemon RPC for reloading `.ccb/ccb.config`.
- `ccb reload --dry-run` for diffing and validation before mutation.
- Additive new agent in an existing managed window.
- Additive new managed window with its sidebar and agents.
- Dynamic unload through bounded draining and retiring.
- Dynamic replacement through pending replacement only after unload semantics
  are safe.
- Config-bound daemon service rebinding.
- Handler routing that reads the current service graph after reload.
- Keeper/lifecycle config signature continuity after successful reload.
- Project view/sidebar invalidation after reload.
- CPU/memory metrics and release gates for reload-related changes.

Out of scope:

- General `ccbd` performance slimming.
- Arbitrary reshuffling of existing pane layout while preserving every pane.
- Automatic file watching as the first reload trigger.
- Cross-project or global hot reload.
- Windows native psmux parity.
