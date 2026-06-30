# Design: Heartbeat PyO3 Production PoC

## Boundary

The heartbeat subsystem is a pure state-machine evaluator plus a small JSON
store. It has no business-domain truth outside of job progress timestamps. The
Rust extension re-implements the same state machine and store logic.

The PoC boundary is:
- **In scope**: replace the Python module imported by `job_heartbeat_runtime/tick.py`.
- **Out of scope**: changing policy values, daemon lifecycle, or any other
  subsystem.

## Compatibility strategy

Python `heartbeat` exposes dataclasses (`HeartbeatState`, `HeartbeatDecision`,
etc.) with exact field names. The Rust extension exposes the same names but as
read-only Python objects with getters, plus `to_record()` / `from_record()`.

For the PoC we will insert a thin adapter shim that:
1. Imports from `ccb_py_heartbeat`.
2. Wraps returned Rust objects in lightweight Python `SimpleNamespace` or
   dataclass instances where the rest of the daemon expects attribute access.

This avoids touching every consumer while still moving the hot-path state
machine into native code.

## Data flow

```text
ccbd tick loop
  → JobHeartbeatService.tick(dispatcher)
    → tick.py: evaluate_heartbeat(policy, ..., state)
      → ccb_py_heartbeat.evaluate_heartbeat(...)   [Rust]
        → returns (HeartbeatState, HeartbeatDecision) [Rust objects]
      → adapter: convert to Python dataclass-like objects
    → dispatcher emits job_heartbeat_* events
```

State persistence (`HeartbeatStateStore`) is also moved to Rust for the PoC;
the Python shim forwards `load`/`save`/`remove`/`list_all` to the Rust store.

## Rollout shape

1. **Shadow mode (optional but recommended)**: run both Python and Rust
   evaluators side-by-side for a few ticks, compare decisions, log mismatches.
2. **Active mode**: use Rust evaluator and store; Python module still available
   as fallback.
3. **Observation**: 10–30 minutes or 100+ ticks, whichever comes first.
4. **Rollback**: rename the shim file or restore the original import.

## Rollback mechanics

The PoC will create a small Python module file at a path that takes precedence
over `ccb-git/lib/heartbeat/`. Rollback is either:

- `rm /path/to/shim/heartbeat.py` (Python falls back to `ccb-git/lib/heartbeat`), or
- setting `CCB_HEARTBEAT_RUST=0` env var if the shim reads it.

We choose the env-var gate for the PoC so rollback is instant and does not
require filesystem changes while `ccbd` is running.

## Risk assessment

| Risk | Mitigation |
|---|---|
| Import path shadowing breaks other code | Place shim only in the production Python path used by `ccbd`; verify `import heartbeat` from `ccbd` Python gets shim, from CLI still gets original. |
| Rust panic kills `ccbd` | PyO3 catches panics and turns them into Python exceptions; `ccb_py_heartbeat` does not use `unsafe`. |
| JSON schema mismatch | Rust `HeartbeatStateStore` writes identical `schema_version`/`record_type` headers. |
| Timestamp parsing drift | Both implementations use the same `seconds_between` semantics; adapter passes strings through unchanged. |
| Memory regression | Bound observation window and automatic rollback trigger. |
