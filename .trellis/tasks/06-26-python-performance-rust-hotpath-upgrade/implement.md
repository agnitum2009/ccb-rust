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
- [ ] Python remains owner of:
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
  - `capabilities.hot_loop_replacement_active=false` explicitly records that no hot-loop semantics are replaced in this slice.

### Slice A — Codex bridge / active-job observation

- [ ] Replace per-agent Codex active-job observation, not all daemon logic.
- [x] Add Rust sidecar `codex_observe` primitive for active Codex job descriptors.
  - Input is explicit `jobs[]` descriptors from Python: `job_id`, `session_path`, `request_anchor`, and prior state.
  - Output is per-job state plus completion items: `anchor_seen`, `assistant_chunk`, `turn_boundary`, `turn_aborted`.
  - The sidecar reads only the passed Codex session JSONL path; it does not scan all agents and does not poll idle agents.
- [x] Keep Codex hooks enabled.
- [x] Keep provider CLI process unchanged.
- [x] Keep Python fallback path behind a feature flag / env knob until smoke passes.
  - `CCB_RUNTIME_ACCELERATOR_CODEX` defaults disabled.
  - `CCB_RUNTIME_ACCELERATOR_SOCKET` can point tests or manual smoke at an explicit sidecar socket.
  - Sidecar communication failures, malformed observations, per-job errors, and unknown item kinds fall back to the existing Python reader path.
  - Successful no-change observations do not fall through into the Python reader path, so the opt-in path does not pay both polling costs.
- [ ] Target active-job completion check around 200ms if needed, with zero idle polling.

### Slice B — ccbd maintenance hot loop

- [ ] Replace fixed no-op maintenance cadence with dirty-event + active-job wake scheduling.
- [ ] Keep Python request handlers and socket protocol as owner.
- [ ] Preserve immediate wake for submit/cancel/resubmit/retry/reply-delivery operations.
- [ ] Preserve heartbeat freshness semantics while avoiding no-op full-agent work.
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

Still pending for the broader milestone:

- Active ask-storm baseline.
- Live Codex ask/callback/reply smoke with `CCB_RUNTIME_ACCELERATOR_CODEX=1` and a managed sidecar socket.
- Syscall-level attribution if Slice A/B needs it.
- Python daemon lifecycle/start-monitor integration for the runtime accelerator sidecar.
- Slice B ccbd maintenance wake scheduling replacement.

## Risk / rollback

- Rollback is Python fallback path: disable Rust accelerator and return to current Python polling.
- Do not delete Python bridge code in first slice.
- Do not change public socket payloads in first slice.
- Do not change provider hook configuration.
