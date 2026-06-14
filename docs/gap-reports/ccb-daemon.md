# Gap Report: ccb-daemon

**Goal:** A4 — verify 1:1 parity of Python `ccbd/` + `fault_injection/` vs Rust `ccb-daemon`.
**Date:** 2026-06-14
**Methodology attempted:** Recursive public `def` extraction (636 fns); name-presence
check against Rust `fn <name>` in `ccb-daemon/src`.

## Result: ⚠️ MECHANICAL AUDIT INVALID (same divergence as ccb-providers)

| Metric | Value |
|--------|-------|
| Python public fns | 636 |
| Name-match "gaps" (raw) | 611 |
| Rust ccb-daemon LOC | 8 754 |
| Python `ccbd/` LOC | 34 406 (367 files, mostly micro-submodules) |

The 611 "gaps" are **~95% false positives**. ccb-daemon's Rust design is
handler/struct-based (`handle_submit`, `CcbdApp::heartbeat`, `impl SocketServer`, …)
while Python `ccbd/` is a forest of tiny module-level functions. Name matching fails
the same way it did for ccb-providers.

## Real, actionable gaps (from functional Phase A–D analysis, not grep)

These are CONFIRMED stub/missing behaviors regardless of the mechanical audit:

1. **`watch` RPC** — daemon handler exists but the CLI `watch` is single-shot; no
   streaming/poll loop wired end-to-end (Phase E gap).
2. **Some advanced daemon handlers remain stubs** (documented in
   `rust/crates/ccb-daemon/README.md`).
3. **`reload/plan.rs`** notes "explicit restart policy is not implemented" for
   tool-window changes.

## Verdict & recommended action

The mechanical 1:1 audit is **not meaningful** for ccb-daemon. The control plane
(start/stop/ask/submit/cancel/inbox/queue/trace/ack/mailbox) IS wired and tested
(Phase A–D verification). Treat the ~3 functional stubs above as the real to-do list,
not the 611 grep number.

A true 1:1 audit here would need a handler-mapping harness (`Python service fn →
Rust handler`), deferred as low-value since the runtime works.
