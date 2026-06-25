# Design: ccbd ↔ ccbrd Owner Alignment

## Goal

Use the `/mnt/g/owner` responsibility-chain method to align Python `ccb` 7.5.2 capability ownership with Rust `ccbr`/`ccbrd`, then close the remaining interoperability gaps without copying Python's low-performance runtime architecture.

## Owner model

| Layer | Owner role | Evidence anchor | Non-claim |
|---|---|---|---|
| Python `ccb` 7.5.2 | Reference capability owner for wire behavior and user-facing semantics | `backup/python-reference/lib/ccbd/**`, `lib/ccbd/**` | Not a performance/template owner for Rust internals |
| Rust `ccbrd` | Current implementation owner for daemon protocol, lifecycle, runtime integration | `rust/crates/ccbr-daemon/**` | Must not silently diverge from Python client-facing contracts |
| Rust providers | Runtime/session/polling owner | `rust/crates/ccbr-providers/**` | Must not disable Codex hooks |
| Python clients | Consumer owner for sidebar/ask/CLI compatibility | `bin/ccb-agent-sidebar`, Python `ask` surface | Consumer cannot redefine daemon truth |
| Trellis/CodeGraph | Evidence and planning accelerators | `.trellis/**`, CodeGraph index | Not owner truth |

## Current evidence

- Python registers 26 ccbd RPC ops.
- Rust registers all 26 Python ops plus local extensions: `ask`, `cleanup`, `fault_*`, `logs`, `maintenance_tick`.
- Handler-file presence is no longer the main gap; remaining risk is behavior/shape parity.
- Confirmed Rust+DDD deviations are allowed only when the client-facing contract remains compatible and the reason is documented.

## Primary surfaces and contracts

### 1. Submit / ask delivery

- Surface: interface + capability.
- Python owner: `handlers/submit.py` creates `MessageEnvelope` and calls `dispatcher.submit(envelope).to_record()`.
- Rust owner: `handlers/submit.rs`, `handlers/ask.rs`, dispatcher, mailbox, provider execution.
- Current risk: Rust `handlers/mod.rs` still documents `submit` as enqueue-only and says Python ask uses `submit`; Rust also has a local `ask` op. Python clients may call `submit`, not `ask`.
- Desired contract: `submit` must be sufficient for Python client ask-chain delivery, or explicitly delegate to the same delivery path as Rust `ask`.

### 2. Sidebar / project view

- Surface: projection/readback.
- Python owner: `handlers/project_view.py`, `project_view/**`.
- Rust owner: `handlers/project_view.rs`, project namespace runtime.
- Desired contract: Python sidebar gets `{view, cache, schema_version}` with agent `window`, mailbox/comms state, daemon status, and no "ccbd unavailable" for supported requests.

### 3. Mailbox / inbox / ack

- Surface: interface + readback.
- Python owner: `handlers/inbox.py`, `mailbox_head.py`, `ack.py`.
- Rust owner: `ccbr-mailbox`, daemon handlers.
- Desired contract: A ask B -> B final reply -> A inbox detail shows reply; ack/head state remains stable.

### 4. Restart / reload / clear / focus

- Surface: lifecycle gate + runtime integration.
- Python owner: `handlers/project_restart.py`, `project_reload.py`, `project_clear.py`, `project_focus.py`.
- Rust owner: daemon handlers and project namespace runtime.
- Current risk: `project_restart_panes` in Rust returns `scheduled` but does not restart panes; Python schedules an in-place restart callback.
- Desired contract: supported restart endpoints either perform/schedule real work or fail loudly with compatible structured reason.

### 5. Shutdown / stop-all

- Surface: lifecycle gate.
- Python owner: `handlers/shutdown.py`, `stop_all.py`.
- Rust owner: `app.shutdown()`, `stop_flow`.
- Local owner adoption: user confirmed red X means complete workspace exit. Rust shutdown must kill the managed tmux session and provider processes. This intentionally diverges from Python's graceful `force=False` shutdown for this project.

### 6. Provider sessions / polling

- Surface: runtime integration + policy.
- Python owner: provider reference behavior.
- Rust owner: `ccbr-providers`.
- Required deviation: keep Rust single-process, active-only polling; do not adopt Python per-agent bridge tight polling.
- Hard rule: never disable, remove, skip, or mask Codex hooks.

## Compatibility strategy

1. Preserve Python wire shape at the socket boundary.
2. Keep Rust implementation internals DDD/efficient when externally equivalent.
3. Record intentional divergence as owner adoption with non-claims.
4. Add regression tests for every closed gap before implementation.

## Rollback

- Each P0 change must be isolated to one surface and covered by targeted tests.
- If live Python client smoke regresses, revert only the surface commit, not unrelated owner documentation.
