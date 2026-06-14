# Gap Report: ccb-cli

**Goal:** A5 — verify 1:1 parity of Python `cli/` + `ask_cli/` vs Rust `ccb-cli`.
**Date:** 2026-06-14
**Methodology attempted:** Recursive public `def` extraction (385 fns); name-presence
check against Rust `fn <name>` in `ccb-cli/src`.

## Result: ⚠️ MECHANICAL AUDIT INVALID (consolidated-dispatch divergence)

| Metric | Value |
|--------|-------|
| Python public fns | 385 |
| Name-match "gaps" (raw) | 346 |
| Rust ccb-cli LOC | 4 792 |
| Python `cli/` LOC | 21 486 (185 files) |

The 346 "gaps" are **~90% false positives**. Rust `ccb-cli` deliberately consolidated
185 Python files into ~13 files using a hand-written parser + dispatch table
(`entry.rs::dispatch` + `commands.rs`). Python's many small `cmd_*` functions map to a
single `commands::<verb>` each. Name matching is meaningless here.

## Real, actionable gaps (from functional Phase E/G analysis — CONFIRMED)

These are the actual missing/incomplete CLI capabilities:

1. **`watch`** — single RPC call, no CLI-side poll loop (Phase E).
2. **`ccb update` / `uninstall` / `reinstall`** — return stub text "not implemented in
   this build" (`commands.rs:558-571`). (Note: `versioning` backend now translated —
   see `ccb-cli/src/versioning.rs`; `cmd_update` wiring is the remaining step.)
3. **`ccb tools install/doctor`** — return fixed `"ok"`/`"scheduled"` stub
   (`commands.rs:406`).
4. **`ccb ctx-transfer`** — the 8-function context-transfer pipeline has NO Rust
   implementation (see `docs/gap-reports/ccb-memory.md` cluster 1).
5. **`roles add`** — wired but requires installed-role prerequisites (functional, not a stub).

## What IS complete

All P0–P2 message/lifecycle commands dispatch to real daemon RPCs with no stubs:
`start/stop/kill/ask/wait/cancel/queue/trace/inbox/ack/resubmit/retry/clear/reload/
restart/ps/status/ping/shutdown/logs/cleanup/doctor/config/pend/repair/fault/roles`.
`--version`/`--help`/`-h` work outside a project.

## Verdict & recommended action

Mechanical 1:1 audit is **not meaningful** for ccb-cli (by design, it's a consolidated
dispatcher). The real to-do list is the ~4 functional gaps above (watch polling,
management_runtime wiring, tools_runtime, ctx-transfer) — these are tracked as Phase E/G
follow-ups, not line-level translation work.
