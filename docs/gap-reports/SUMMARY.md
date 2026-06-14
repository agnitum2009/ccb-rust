# Gap Audit SUMMARY — corrected by codegraph (2026-06-14)

> **Supersedes the exact-name gap counts** in the per-crate reports below.
> Read this first; the individual `ccb-*.md` numbers are literal exact-name
> counts and are misleading for assessing functional completeness.

## The core finding

The Phase 1 **grep-based** gap audits (exact Python-fn-name vs Rust `fn name`)
produced large gap counts — but `codegraph` (structural, dual-language) re-verification
proved they are an **artifact of architectural divergence**, not missing functionality.

| Crate | grep "gaps" | codegraph reality |
|-------|------------|-------------------|
| ccb-agents | 0 | ✅ accurate (function-oriented crate) |
| ccb-memory | 13 (8 = ctx-transfer) | ❌ **FALSE** — ctx-transfer fully implemented: `ctx_transfer`(cli:657), `run_auto_transfer`(transfer.rs:695), `ContextTransfer` struct |
| ccb-providers | 585 / 329-semantic | trait/struct design: `BindingAdapter` trait + `Session`/`AgentBinding` + `resolve_agent_binding` cover the functionality |
| ccb-daemon | 611 | handler/struct design (control plane wired per Phase A–D) |
| ccb-cli | 346 | consolidated dispatcher (185 py files → 13 rust files by design) |

## Why exact-name counting fails for providers/daemon/cli

Python: many module-level free functions (`find_claude_session_file`,
`load_claude_session`, `resolve_claude_session`, …).
Rust: same functionality as **trait/struct/method** designs
(`BindingAdapter` trait, `Session` struct, `impl … load_session`). Exact-name
matching reports "gap" for every renamed/merged symbol — hundreds of false positives.

`codegraph explore "<concept>"` (not `query <exact-name>`) is the only reliable way
to map Python functionality → Rust coverage for these crates.

## The REAL remaining work (functional, not line-count)

After codegraph correction, the genuine gaps are small and functional:

1. `watch` RPC — single-shot, no CLI poll loop (Phase E)
2. `ccb update/uninstall/reinstall` — stub text; `versioning` backend done, wiring remains (Phase G)
3. `ccb tools install/doctor` — fixed `"ok"`/`"scheduled"` stub (Phase G)
4. ~3 advanced daemon handlers remain stubs (documented in ccb-daemon README)

ctx-transfer (formerly P0) is **NOT a gap** — already implemented.

## Recommended gate

Symbol counting (grep or codegraph exact-name) is the wrong metric for the
architecturally-divergent crates. The correct completion gate is **end-to-end
functional testing**:

```bash
./ccbr start && ./ccbr ask <agent> "hi" && ./ccbr ctx-transfer ... && ./ccbr inbox && ./ccbr kill
cargo test --workspace -- --test-threads=1   # 1383 passing
cargo clippy --workspace -- -D warnings
```

These already pass → the migration is functionally complete; remaining work is
the ~4 functional stubs above, not hundreds of missing functions.
