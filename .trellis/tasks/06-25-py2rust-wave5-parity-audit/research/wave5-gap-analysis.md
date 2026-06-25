# Wave 5 Python→Rust Parity Gap Analysis

Generated from full-matrix review + S3/S4 live verification.
North star: 1:1 Rust replacement of Python `lib/`, keeping reference `ccb` runnable.

## Method
- Baseline: Python `lib/` modules vs Rust `rust/crates/ccbr-*` crates (HEAD).
- Existing matrix: `plans/rust-python-test-parity-matrix.md` already maps most clusters.
- Live cross-check: S3 multi-agent recovery + S4 edge scenarios on `/mnt/d/dapro-ass`.

## Criticality-ranked gap list

| Rank | Gap | Python reference | Rust location | Criticality | Proposed Wave 5 subtask |
|---|---|---|---|---|---|
| 1 | Daemon restart does not restore running jobs (trace empty after restart) | `lib/ccbd/services/dispatcher_runtime/restore_running_jobs.py` (conceptual), `test_v2_daemon_startup_wait.py` | `crates/ccbr-daemon/src/app.rs` startup/heartbeat, `crates/ccbr-jobs` persistence | **P0** | `06-25-py2rust-wave5-daemon-restore-jobs` |
| 2 | Persistent `MountManager` / `OwnershipGuard` across daemon restarts | `lib/ccbd/supervision/mount_runtime/service.py` | `crates/ccbr-daemon/src/supervision/mount_runtime/` | **P0** | `06-25-py2rust-wave5-mount-ownership-persist` |
| 3 | `RuntimeSupervisionLoop` end-to-end recovery orchestration | `lib/ccbd/supervisor_runtime/lifecycle.py`, `lib/ccbd/supervision/loop_runtime.py` | `crates/ccbr-daemon/src/supervision/loop_runner.rs` (stub) | **P0** | `06-25-py2rust-wave5-supervision-loop` |
| 4 | Mid-run job cancel (true cancellation while provider is running) | `lib/ccbd/services/dispatcher_runtime/cancel_runtime.py` | `crates/ccbr-daemon/src/services/dispatcher.rs` cancel | **P1** | `06-25-py2rust-wave5-midrun-cancel` |
| 5 | Provider timeout / stall handling | `lib/ccbd/services/job_heartbeat_runtime/` | `crates/ccbr-daemon/src/services/dispatcher.rs` + provider polling | **P1** | `06-25-py2rust-wave5-provider-timeout` |
| 6 | Auth failure surfaces explicit CLI error | `lib/provider_backends/codex/auth_runtime.py` | `crates/ccbr-provider-profiles/src/codex_home_config.rs` | **P1** | `06-25-py2rust-wave5-auth-error-surface` |
| 7 | Rich `ping` payload / runtime health round-trip | `lib/ccbd/handlers/ping_runtime/` | `crates/ccbr-daemon/src/handlers/ping.rs` | **P1** | `06-25-py2rust-wave5-rich-ping` |
| 8 | Codex pane-shutdown-text delivery guard | `test_stability_regressions.py::test_codex_delivery_guard_fails_on_shutdown_text_without_anchor` | `crates/ccbr-providers/src/providers/codex.rs` | **P2** | `06-25-py2rust-wave5-codex-delivery-guard` |
| 9 | `test_v2_start_foreground.py` / `test_v2_start_service.py` | `lib/cli/start_foreground_runtime/`, `lib/ccbd/start_flow_runtime/service.py` | `crates/ccbr-cli/src/start_foreground.rs`, `crates/ccbr-daemon/src/start_flow/service.rs` | **P2** | `06-25-py2rust-wave5-start-foreground-service` |

## Already-closed in prior waves (do NOT create tasks)
- Cross-agent reply routing (S3.1) — fixed + live verified.
- Pane death respawn (S3.2) — `respawn_dead_agents` + `project_restart_agent` implemented.
- Stale-pane reuse guard (S3.4) — already in `start_flow/service.rs`.
- Empty message, long prompt, concurrency, UTF-8, special chars, reload-mid-ask (S4.1/2/3/8/9/7) — verified.

## Recommended execution order
1. P0 daemon recovery gaps (1-3) first — unblock production-grade daemon resilience.
2. P1 runtime control gaps (4-7) in parallel — improve user-visible robustness.
3. P2 provider/startup polish (8-9) last.

## Notes for ccb-legacy
- These gaps are behavior-level; ccb-legacy sync only needed when Rust code changes land.
- Use reverse-rename `s/ccbr/ccb/g; s/CCBR/CCB/g` on modified rust/ files + paths.
