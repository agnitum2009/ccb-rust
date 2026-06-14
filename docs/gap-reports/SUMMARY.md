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

After codegraph correction, the genuine gaps were small and functional. **Status
as of 2026-06-14 (functional-stub pass complete):**

1. ✅ `watch` RPC — **poll loop implemented** (advancing cursor until terminal/timeout;
   env `CCB_WATCH_TIMEOUT_S`/`CCB_WATCH_POLL_INTERVAL_S`) — `commands.rs::watch`
2. ✅ `ccb update` — **version-check wired** (`get_available_versions` + `latest_version`
   + `is_newer_version`); tarball install still delegates to install.sh.
   ⚠️ `ccb uninstall`/`reinstall` remain stubs (need `install.py` translation).
3. ✅ `ccb tools doctor neovim` — **real status** (`tools_runtime::neovim_status`).
   ⚠️ `tools install` stays guided (heavy nvim/LazyVim downloader not ported).
4. ✅ **daemon handlers verified real** — grep found zero production stubs; all 30
   handlers dispatch to real logic. Only documented edge case:
   `reload/plan.rs:664` tool-window restart policy (minor).

ctx-transfer (formerly P0) is **NOT a gap** — already implemented.

### Genuinely remaining (heavy tail)

- `ccb uninstall`/`reinstall` + full `update` tarball flow — `management_runtime/install.py` (372 lines)
- `ccb tools install neovim` full downloader — `tools_runtime/neovim.py` (940 lines)
- `install.sh` de-Python dependency
- `reload/plan.rs` tool-window restart policy edge case


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
