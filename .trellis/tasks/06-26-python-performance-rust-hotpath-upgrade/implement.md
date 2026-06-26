# Implementation Plan: Python high-CPU runtime loop Rust replacement

## Phase 0 — Baseline measurement, no code changes

- [x] Use `/home/agnitum/ccb-git` at HEAD `06cbdc3` as Python latest baseline.
- [x] Use `/home/agnitum/ccb/ccb-legacy` `ccb-legacy` as the Rust replacement implementation worktree.
- [x] Reproduce 2-Codex smoke baseline for repeatability.
- [x] Reproduce 4+ Codex n14-like baseline for CPU acceptance.
- [x] Capture per-process CPU/RSS evidence:
  - `ccbd/main` / `ccbd` process
  - each `provider_backends.codex.bridge` process
  - Codex provider CLI processes separately
- [ ] Capture syscall evidence separately if needed for Slice A/B diagnosis.
- [ ] Record active vs idle behavior separately.
  - Startup/idle live profiles are captured.
  - Active ask-storm profile remains pending.
- [ ] Confirm whether measured bridge CPU is from:
  - FIFO bridge wait
  - Codex comm log polling
  - binding tracker polling
  - readiness polling
  - pane/session readback

## Phase 1 — Contract freeze

- [ ] Freeze ask stability scenarios before replacing loop implementation:
  - plain ask A -> B
  - queued asks to same target preserve order
  - callback continuation chain
  - reply delivery
  - cancel/resubmit while idle
  - first ask after idle
- [ ] Freeze Codex event/readback fixtures:
  - anchor seen
  - final assistant event
  - turn boundary
  - no false completion from prompt echo
  - session/log rotation
- [ ] Write expected outputs in Python latest tests first.

## Phase 2 — Rust accelerator shape

- [x] First-shape decision: create a narrow Rust sidecar/accelerator boundary for Python `.ccb` runtime.
- [x] Do not call it `ccbrd`; it is a Python-runtime accelerator, not the ccbr daemon.
- [x] Protocol decision: Unix domain socket + JSON-RPC/JSONL frame owned by `ccb-legacy` / Python `.ccb` accelerator.
- [x] Crate placement decision: `rust/crates/ccb-runtime-accelerator`, binary `ccb-runtime-accelerator`.
- [x] Inputs:
  - project root
  - `.ccb` runtime paths
  - active job descriptors
  - Codex session/log/pane references
- [x] Outputs:
  - completion/readback events
  - runtime health/activity events
  - clear wake/failure state
- [x] Python remains owner of:
  - socket protocol
  - job store/mailbox
  - public CLI
  - lifecycle reports
- [ ] Rust owns:
  - multiplexed waits
  - active-job polling cadence
  - transcript/readback parsing
  - no idle-agent polling

## Phase 3 — First milestone implementation slices

### Slice 0 — Protocol shell, fallback, baseline

- [x] Decision: do this before hot-loop replacement.
- [x] Add `ccb-legacy` sidecar protocol shell over Unix socket JSON-RPC/JSONL frame.
- [x] Add Python fallback path so unavailable sidecar returns to current Python polling.
- [x] Add baseline measurement harness for `ccbd` CPU and per-agent Codex bridge CPU.
- [x] Verify no public socket/job/mailbox behavior changes in Slice 0.
  - Slice 0 only adds a sidecar binary, a Python fallback client, and tests.
  - No Python daemon/socket/job/mailbox integration is enabled yet.
  - Slice 0 originally recorded `capabilities.hot_loop_replacement_active=false`; later Slice A default replacement flips it to `true`.

### Slice A — Codex bridge / active-job observation

- [x] Replace per-agent Codex active-job observation, not all daemon logic.
- [x] Add Rust sidecar `codex_observe` primitive for active Codex job descriptors.
  - Input is explicit `jobs[]` descriptors from Python: `job_id`, `session_path`, `request_anchor`, and prior state.
  - Output is per-job state plus completion items: `anchor_seen`, `assistant_chunk`, `turn_boundary`, `turn_aborted`.
  - The sidecar reads only the passed Codex session JSONL path; it does not scan all agents and does not poll idle agents.
- [x] Keep Codex hooks enabled.
- [x] Keep provider CLI process unchanged.
- [x] Keep Python fallback path behind an env knob.
  - `CCB_RUNTIME_ACCELERATOR_CODEX` defaults enabled for replacement.
  - `CCB_RUNTIME_ACCELERATOR_CODEX=0` explicitly disables the replacement and returns to Python reader polling.
  - `CCB_RUNTIME_ACCELERATOR_SOCKET` can point tests or manual smoke at an explicit sidecar socket.
  - `CCB_RUNTIME_ACCELERATOR_BIN` lets ccbd find the sidecar binary; default lookup also checks repo-local Rust build outputs.
  - Sidecar communication failures, malformed observations, per-job errors, and unknown item kinds fall back to the existing Python reader path.
  - Successful no-change observations do not fall through into the Python reader path, so the replacement path does not pay both polling costs.
- [x] Add ccbd sidecar lifecycle shell behind the same env gate.
  - Default-on: sidecar process is spawned unless `CCB_RUNTIME_ACCELERATOR_CODEX=0`.
  - Startup failures are non-fatal and keep Python fallback available.
  - Shutdown only removes sockets owned by the sidecar process ccbd started.
- [x] Target active-job completion check around 200ms if needed, with zero idle polling.
  - Python latest/global bridge idle wait now defaults to 1.0s event wait instead of 0.05s hot polling; FIFO messages still wake immediately through the persistent reader.
  - Python latest/global binding tracker now defaults to 5.0s idle interval instead of 0.5s repeated session/log scans; explicit `CCB_CODEX_BIND_POLL_INTERVAL` override remains.
  - Ambiguous Codex session-follow now caches an unchanged session-root signature, so idle bridges like `mn_c` do not repeatedly reparse the same large jsonl history.
  - Bound Codex session-follow now reuses the existing bound log after switch detection, avoiding a second workspace-wide current-log scan on every idle tick.
  - Bound/no-new-candidate Codex session-follow now caches unchanged session-root signatures, avoiding the full switch resolver itself on repeated idle ticks.

### Slice B — ccbd maintenance hot loop

- [x] Replace fixed no-op maintenance cadence with dirty-event + active-job wake scheduling.
  - Python latest/global reduction installed: idle heartbeat refreshes lease but skips full health/supervision/dispatcher maintenance until active work appears or `CCB_CCBD_IDLE_FULL_HEARTBEAT_INTERVAL_S` elapses.
- [ ] Keep Python request handlers and socket protocol as owner.
- [ ] Preserve immediate wake for submit/cancel/resubmit/retry/reply-delivery operations.
- [x] Preserve heartbeat freshness semantics while avoiding no-op full-agent work.
  - `MountManager.refresh_heartbeat()` validates the current lease holder on every tick, but debounces lease JSON rewrites to `CCB_CCBD_HEARTBEAT_WRITE_INTERVAL_S` seconds, default `5.0`.
  - Keeper no longer rewrites stable mounted `lifecycle.json` and debounces `keeper.json` `last_check_at`-only rewrites to `CCB_KEEPER_STATE_WRITE_INTERVAL_S` seconds, default `5.0`.
  - ProjectView now uses a longer idle cache TTL (`CCB_PROJECT_VIEW_IDLE_TTL_MS`, default `5000`) only when no dispatcher work or busy agent state is pending; active work stays at `CCB_PROJECT_VIEW_TTL_MS`, default `1000`.
  - ProjectView cache no longer relies on TTL for correctness: dispatcher job/event mutations increment an in-memory revision, so cached views are rebuilt immediately when submit/complete/cancel/retry style state changes happen.
  - Runtime registry now treats `last_seen_at`-only refreshes as in-memory freshness updates instead of per-agent JSON rewrites; material runtime changes still persist immediately with the freshest in-memory timestamp.
- [ ] Milestone is incomplete until both Slice A and Slice B have before/after CPU evidence.

## Phase 4 — Compatibility gates

- [ ] Python latest tests pass on `/home/agnitum/ccb-git`.
- [ ] Rust module enters `ccb-legacy` first, after Python-compatible golden tests pass.
- [ ] `ccbr` consumes the shared crate/module only optionally, through `.ccbr` adapter, after `ccb-legacy` proof.
- [ ] No `.ccb` path assumptions enter `ccbr`; no `.ccbr` assumptions enter `ccb-legacy`.

## Phase 5 — Validation

- [ ] CPU: idle multi-Codex scenario drops bridge/daemon CPU materially versus baseline.
- [ ] Functional: ask/callback/reply/cancel/resubmit scenarios pass.
- [ ] Regression: provider log/readback fixtures match Python expected outputs.
- [ ] Resource cleanup: test workspace leaves no daemon/bridge/accelerator residue.

## Slice 0 validation evidence

- `cargo fmt --check -p ccb-runtime-accelerator`
- `cargo test -p ccb-runtime-accelerator -- --test-threads=1`
- `cargo build -p ccb-runtime-accelerator`
- `PYTHONPATH=lib pytest -q test/test_runtime_accelerator_client.py`
- Synthetic Python latest Phase 0 baseline:
  - `.trellis/tasks/06-26-python-performance-rust-hotpath-upgrade/evidence/python-latest-synthetic-phase0-baseline.json`
- Live Python latest startup/idle baseline:
  - `.trellis/tasks/06-26-python-performance-rust-hotpath-upgrade/evidence/live-baseline/live-2codex-startup-profile.json`
  - `.trellis/tasks/06-26-python-performance-rust-hotpath-upgrade/evidence/live-baseline/live-4codex-startup-profile.json`
  - `.trellis/tasks/06-26-python-performance-rust-hotpath-upgrade/evidence/live-baseline/live-baseline-summary.md`
  - 2-Codex: ccbd avg CPU `26.370%`; provider/codex avg CPU `29.950%`; max project procs `23`.
  - 4-Codex: ccbd avg CPU `18.783%`; provider/codex avg CPU `71.075%`; max project procs `39`.
  - Both live runs ended with `ccb_test kill -f` and process-residue checks.
- Real Unix socket smoke:
  - `ccb-runtime-accelerator serve --socket <ephemeral>`
  - `ccb-runtime-accelerator ping --socket <ephemeral>`
  - `ccb-runtime-accelerator baseline-snapshot --project-root /home/agnitum/ccb/ccb-legacy`
- Slice A primitive validation:
  - `codex_observe` unit tests for anchor + assistant + task boundary.
  - `codex_observe` unit test proving assistant output before anchor does not emit completion items.
  - `codex_observe` unit test proving missing session files return per-job errors, not whole-batch failure.
  - Real Unix socket smoke for `codex_observe` over an ephemeral legacy `.ccb/runtime-accelerator` socket.
- Slice A Python adapter validation:
  - `uv run --with pytest pytest -q test/test_codex_runtime_accelerator_polling.py test/test_codex_execution_polling.py test/test_runtime_accelerator_client.py`
  - `python -m compileall -q lib/provider_backends/codex/execution_runtime test/test_codex_runtime_accelerator_polling.py`
  - `git diff --check`
  - `cargo fmt --check -p ccb-runtime-accelerator`
  - `cargo test -p ccb-runtime-accelerator -- --test-threads=1`
- Slice A lifecycle shell validation:
  - `uv run --with pytest pytest -q test/test_runtime_accelerator_lifecycle.py test/test_codex_runtime_accelerator_polling.py test/test_runtime_accelerator_client.py test/test_codex_execution_polling.py`
  - `python -m compileall -q lib/runtime_accelerator lib/provider_backends/codex/execution_runtime lib/ccbd/app_runtime test/test_runtime_accelerator_lifecycle.py test/test_codex_runtime_accelerator_polling.py`
  - `git diff --check`
  - `cargo fmt --check -p ccb-runtime-accelerator`
  - `cargo test -p ccb-runtime-accelerator -- --test-threads=1`
- Default replacement / idle maintenance validation:
  - `uv run --with pytest pytest -q test/test_runtime_accelerator_lifecycle.py test/test_codex_runtime_accelerator_polling.py test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_skips_heavy_idle_maintenance_between_full_ticks test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_runs_heavy_maintenance_for_active_execution test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_skips_maintenance_while_start_lock_held test/test_runtime_accelerator_client.py test/test_codex_execution_polling.py`
  - `python -m compileall -q lib/runtime_accelerator lib/provider_backends/codex/execution_runtime lib/ccbd/app_runtime test/test_runtime_accelerator_lifecycle.py test/test_codex_runtime_accelerator_polling.py test/test_v2_ccbd_socket.py`
  - `cargo build -p ccb-runtime-accelerator`
  - `cargo test -p ccb-runtime-accelerator -- --test-threads=1`
- Global runtime install validation:
  - Bridge idle hot-loop reduction installed on global runtime: `/root/.local/share/codex-dual`.
  - Backup: `/home/agnitum/ccb-runtime-backups/codex-dual-bridge-idle-wait-20260626-141747`.
  - Verified installed `CCB_BRIDGE_IDLE_SLEEP` default is `1.0` and provider_core spool idle sleep is `0.2`.
  - Source validation in `/home/agnitum/ccb-git`: `PYTHONPATH=lib pytest -q test/test_codex_bridge_runtime.py test/test_codex_comm_io.py`, `git diff --check`, `python -m compileall -q lib/provider_backends/codex/bridge_runtime lib/provider_core test/test_codex_bridge_runtime.py`.
  - Binding tracker reduction installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-binding-idle-wait-20260626-142208`; validation: `PYTHONPATH=lib pytest -q test/test_codex_bridge_runtime.py test/test_codex_comm_io.py test/test_codex_binding_update.py`.
  - Ambiguous session-follow scan suppression validated in ccb-legacy: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_codex_session_switch.py test/test_codex_binding_update.py test/test_codex_bridge_runtime.py test/test_codex_comm_io.py` -> `12 passed`; commit `139b99d9 Avoid repeated ambiguous Codex session scans`.
  - Ambiguous session-follow scan suppression validated in ccb-git: same command -> `12 passed`; commit `72af42b Avoid repeated ambiguous Codex session scans`.
  - Ambiguous session-follow scan suppression installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-binding-ambiguous-scan-20260626-155524`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/provider_backends/codex/bridge_runtime/binding_runtime.py`. Existing bridge processes must restart to load this Python file.
  - Bound-log reuse after switch detection validated in ccb-legacy: same four-test command -> `13 passed`; commit `606affb2 Reuse bound Codex logs after switch checks`.
  - Bound-log reuse after switch detection validated in ccb-git: same four-test command -> `13 passed`; commit `d45e81b Reuse bound Codex logs after switch checks`.
  - Bound-log reuse installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-binding-bound-log-shortcut-20260626-155930`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/provider_backends/codex/bridge_runtime/binding_runtime.py`. Existing bridge processes must restart to load this Python file.
  - Bound/no-new-candidate scan cache validated in ccb-legacy: same four-test command -> `14 passed`; commit `a30206c1 Skip unchanged bound Codex session scans`.
  - Bound/no-new-candidate scan cache validated in ccb-git: same four-test command -> `14 passed`; commit `7caad21 Skip unchanged bound Codex session scans`.
  - Bound/no-new-candidate scan cache installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-binding-bound-scan-cache-20260626-160611`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/provider_backends/codex/bridge_runtime/binding_runtime.py`. Existing bridge processes must restart to load this Python file.
  - Runtime `last_seen_at`-only JSON rewrite suppression validated in ccb-legacy: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_ccbd_registry.py test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_records_step_metrics_without_background_worker test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_skips_heavy_idle_maintenance_between_full_ticks test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_runs_heavy_maintenance_for_active_execution test/test_v2_ccbd_socket.py::test_ccbd_full_idle_heartbeat_does_not_rewrite_last_seen_only_runtime_state` -> `12 passed`; commit `fed88936 Stop persisting runtime freshness-only ticks`.
  - Runtime `last_seen_at`-only JSON rewrite suppression validated in ccb-git: same command -> `12 passed`; commit `3f0f0cc Stop persisting runtime freshness-only ticks`.
  - Runtime `last_seen_at`-only JSON rewrite suppression installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-runtime-last-seen-only-20260626-161802`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/ccbd/services/registry.py`. Existing `ccbd` processes must restart to load this Python file.
- Legacy bloodline baseline updated in `/home/agnitum/ccb/ccb-legacy`: persistent Codex bridge FIFO reader, default `CCB_BRIDGE_IDLE_SLEEP=1.0`, default `CCB_CODEX_BIND_POLL_INTERVAL=5.0`, and regression tests. Validation: `PYTHONPATH=lib pytest -q test/test_codex_bridge_runtime.py test/test_codex_binding_update.py test/test_codex_comm_io.py`, `python -m compileall -q lib/provider_backends/codex/bridge_runtime test/test_codex_bridge_runtime.py test/test_codex_binding_update.py`, `git diff --check`.
  - ccbd idle maintenance reduction installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-ccbd-idle-maintenance-20260626-142656`; validation: `PYTHONPATH=lib pytest -q test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_skips_heavy_idle_maintenance_between_full_ticks test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_runs_heavy_maintenance_for_active_execution test/test_v2_ccbd_socket.py::test_ccbd_heartbeat_skips_maintenance_while_start_lock_held test/test_ccbd_socket_server_loop.py::test_next_worker_timeout_returns_immediate_when_maintenance_pending test/test_ccbd_socket_server_loop.py::test_next_worker_timeout_matches_base_timeout_without_pending_maintenance`.
  - ccbd lease heartbeat write debounce validated in both ccb-legacy and ccb-git: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_v2_ccbd_mount_ownership.py` -> `28 passed`.
  - ccbd lease heartbeat write debounce installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-heartbeat-write-debounce-20260626-151421`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/ccbd/services/mount.py`.
  - keeper/lifecycle idle-write suppression validated in ccb-legacy: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_v2_ccbd_keeper.py` -> `17 passed`.
  - keeper/lifecycle idle-write suppression validated in ccb-git: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_v2_ccbd_keeper.py` -> `21 passed`.
  - keeper/lifecycle idle-write suppression installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-keeper-write-debounce-20260626-152630`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/ccbd/keeper_runtime/stores.py /root/.local/share/codex-dual/lib/ccbd/keeper_runtime/loop.py`.
  - ProjectView idle TTL validated in ccb-legacy: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_ccbd_project_view.py` -> `58 passed`.
  - ProjectView idle TTL validated in ccb-git: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_ccbd_project_view.py` -> `64 passed`.
  - ProjectView idle TTL installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-project-view-idle-ttl-20260626-153240`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/ccbd/project_view/service.py`.
  - ProjectView dispatcher-revision invalidation validated in ccb-legacy: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_ccbd_project_view.py` -> `59 passed`; commit `89dc2557 Refresh ProjectView on dispatcher changes`.
  - ProjectView dispatcher-revision invalidation validated in ccb-git: `PYTHONPATH=lib uv run --with pytest pytest -q test/test_ccbd_project_view.py` -> `65 passed`; commit `89ba66f Refresh ProjectView on dispatcher changes`.
  - ProjectView dispatcher-revision invalidation installed on global runtime; backup: `/home/agnitum/ccb-runtime-backups/codex-dual-project-view-dispatcher-revision-20260626-154016`; validation: `python -m py_compile /root/.local/share/codex-dual/lib/ccbd/project_view/service.py /root/.local/share/codex-dual/lib/ccbd/services/dispatcher_runtime/facade_state.py /root/.local/share/codex-dual/lib/ccbd/services/dispatcher_runtime/records.py`.
  - Patched runtime: `/root/.local/share/codex-dual`
  - Backup: `/home/agnitum/ccb-runtime-backups/codex-dual-rust-accelerator-20260626-135033`
  - Installed sidecar: `/root/.local/share/codex-dual/bin/ccb-runtime-accelerator`
  - Verified default enabled and binary lookup with installed `_ccb-python`.
  - Verified `ccb-runtime-accelerator serve` + `ping` over a temporary socket.
  - Precise residue check found no running `/root/.local/share/codex-dual/bin/ccb-runtime-accelerator`.

Completed handoff commits:

- ccb-legacy `cb4b59cb Reduce Codex bridge idle CPU on the legacy line` validates the Python-compatible bridge/binding idle reduction baseline.
- ccb-git `f1e1383 Reduce idle CPU without changing runtime semantics` commits Python latest idle-loop reductions.
- ccb-git `76d9f75 Add Rust runtime accelerator sidecar` commits the standalone Rust sidecar workspace.
- ccb-git `4ea58bf Wire Python runtime to the Rust accelerator` commits Python/Rust switch glue and fallback tests.
- ccb-git `e530ba3 Document runtime accelerator review controls` commits PR review notes and switch matrix.
- ccb-legacy `7de1b5af Reduce idle lease write churn` commits heartbeat write debounce on the legacy proof line.
- ccb-git `a17d2ca Reduce idle lease write churn` commits the same debounce plus PR note update.
- ccb-legacy `a9c21a0b Stop rewriting stable keeper state` commits keeper/lifecycle idle-write suppression on the legacy proof line.
- ccb-git `552819e Stop rewriting stable keeper state` commits the same keeper/lifecycle suppression plus PR note update.
- ccb-legacy `1abf6abe Cache idle project views longer` commits the ProjectView idle TTL change on the legacy proof line.
- ccb-git `42d4032 Cache idle project views longer` commits the same ProjectView idle TTL change.
- ccb-legacy `139b99d9 Avoid repeated ambiguous Codex session scans` commits idle ambiguous session-follow scan suppression.
- ccb-git `72af42b Avoid repeated ambiguous Codex session scans` commits the same suppression for Python latest.
- ccb-legacy `606affb2 Reuse bound Codex logs after switch checks` commits bound-log reuse after switch detection.
- ccb-git `d45e81b Reuse bound Codex logs after switch checks` commits the same bound-log reuse for Python latest.
- ccb-legacy `a30206c1 Skip unchanged bound Codex session scans` commits bound/no-new-candidate scan caching.
- ccb-git `7caad21 Skip unchanged bound Codex session scans` commits the same scan caching for Python latest.
- ccb-legacy `89dc2557 Refresh ProjectView on dispatcher changes` commits automatic ProjectView cache invalidation on dispatcher job/event mutations.
- ccb-git `89ba66f Refresh ProjectView on dispatcher changes` commits the same automatic invalidation while preserving Python latest sidebar refresh deltas.
- ccb-legacy `fed88936 Stop persisting runtime freshness-only ticks` commits registry-level suppression of `last_seen_at`-only runtime JSON rewrites.
- ccb-git `3f0f0cc Stop persisting runtime freshness-only ticks` commits the same suppression for Python latest.

Still pending for the broader milestone:

- Active ask-storm baseline.
- Live Codex ask/callback/reply smoke with default `CCB_RUNTIME_ACCELERATOR_CODEX` replacement and managed sidecar socket remains user-owned for the global runtime.
- Post-restart live CPU confirmation for the new bridge idle wait remains pending user test.
- Syscall-level attribution if Slice A/B needs it.
- Post-start sidecar health monitoring/restart policy beyond initial ccbd startup.
- Post-restart live CPU confirmation for ccbd idle maintenance reduction.

## Risk / rollback

- Rollback is Python fallback path: disable Rust accelerator and return to current Python polling.
- Do not delete Python bridge code in first slice.
- Do not change public socket payloads in first slice.
- Do not change provider hook configuration.
