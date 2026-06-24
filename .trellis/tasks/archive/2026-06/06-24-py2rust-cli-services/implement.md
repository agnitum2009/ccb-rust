# Implementation Plan: py2rust-cli-services-parity

## Ordered Checklist

### Phase A — Near-complete clusters (small test gaps)

1. **`cleanup_service`**
   - [x] Read `tests/cleanup_service_tests.rs` and identify missing `skipped_count` assertions.
   - [x] Add test for shared Claude version referenced by symlinked agent home.
   - [x] Skip `force=true` test — `cleanup_project_storage_with` does not expose a force flag.
   - [x] Run `cargo test -p ccb-cli --test cleanup_service_tests -- --test-threads=1`.

2. **`diagnostics_bundle`**
   - [x] Run existing `v2_diagnostics_bundle_tests` and confirm all pass.
   - [x] Add regression test that provider-state auth/cache/tmp plugin paths are excluded.
   - [x] Skip explicit `output_path` test — already covered by existing suite.
   - [x] Run `cargo test -p ccb-cli --test v2_diagnostics_bundle_tests -- --test-threads=1`.

3. **`kill_runtime_agent_cleanup`**
   - [x] Add test for `force=true` through `prepare_local_shutdown`.
   - [x] Add test for configured agent with missing runtime file.
   - [x] Run `cargo test -p ccb-cli --test kill_runtime_agent_cleanup_tests -- --test-threads=1`.

### Phase B — Medium scope clusters

4. **`doctor_runtime`**
   - [x] Inspect Python `test_doctor_runtime_identity.py` and existing Rust tests.
   - [x] Confirm `runtime_identity_summary` and `render_doctor` already parity-map the Python reference tests.
   - [x] Mark cluster `complete` in parity matrix; full `doctor_summary` orchestration is out-of-scope for this task.
   - [x] Run `cargo test -p ccb-cli --test doctor_runtime_identity_tests -- --test-threads=1`.

5. **`kill_service`**
   - [x] Implement `record_shutdown_intent` in `services/daemon.rs` using `daemon_runtime::keeper::record_shutdown_intent`; add `crates/ccb-cli/tests/daemon_tests.rs`.
   - [x] Confirm authority PID snapshot is collected **before** remote stop in `kill_project_with` orchestration; added `crates/ccb-cli/tests/kill_runtime_remote_tests.rs` for remote-stop behavior.
   - [x] Implement default `await_remote_shutdown` path in `kill.rs` using lifecycle phase inspection; expose `wait_for_pid_exit` from `kill_runtime/processes.rs`.
   - [x] Implement `shutdown_daemon` wiring in `kill_runtime/shutdown.rs` using `daemon_runtime::shutdown::shutdown_daemon`; add unit test.
   - [x] Confirm shutdown report store write via `record_kill_report` (direct test added to `kill_service_tests.rs`).
   - [x] Confirm start-policy clearing is already implemented in `kill_runtime/lifecycle.rs::destroy_project_namespace`.
   - [x] Confirm runtime PID file termination (`bridge.pid`, `codex.pid`, helper-manifest leader) is already handled by `ccb_runtime_pid_cleanup::collect_pid_candidates` and `terminate_runtime_pids`.
   - [x] Add tests for: current tmux socket fallback (`kill_runtime_agent_cleanup_tests`), force fallback report (`kill_service_tests`), worktree prune (`kill_service_tests`).
   - [x] Run `cargo test -p ccb-cli --test kill_service_tests -- --test-threads=1`.
   - [x] Run `cargo test -p ccb-cli --test kill_runtime_remote_tests -- --test-threads=1`.

### Phase C — `ask_service` (escalation-guarded)

6. **Non-socket surfaces**
   - [x] Add test for legacy role alias `ccb.archi` → `agentroles.archi`.
   - [x] Add tests for `write_ask_output` newline and `exit_code_for_ask_status`.
   - [x] Add test for relocated runtime actor resolution (`CODEX_RUNTIME_DIR`).
   - [x] Run `cargo test -p ccb-cli --test ask_service_tests -- --test-threads=1`.

7. **Socket/watch surfaces**
   - [x] Inspect Python `test_v2_ask_service.py` for retry/reconnect/watch tests.
   - [x] Confirmed they require changing `invoke_mounted_daemon` retry/reconnect contract and `watch_ask_job` reconnect semantics; escalated and left out-of-scope for this task.

### Phase D — Finalize

8. **Parity matrix**
   - [x] Update `plans/rust-python-test-parity-matrix.md` to mark the six clusters `complete`.
   - [x] Note any items that remain out-of-scope.

9. **Validation**
   - [x] Run `cargo test -p ccb-cli -- --test-threads=1`.
   - [x] Run `cargo fmt --all -- --check`.
   - [x] Run `cargo clippy -p ccb-cli --tests` (warnings acceptable if pre-existing).

## Validation Commands

```bash
cargo test -p ccb-cli -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

## Risky Files / Rollback Points

- `src/services/kill.rs` — orchestration order; any regression breaks project kill.
- `src/services/daemon.rs` — shared daemon client; changes affect all CLI services.
- `src/services/ask.rs` — submit/watch paths; escalation required before socket changes.
- `src/services/doctor.rs` — new orchestrator; keep isolated from render path initially.

## Review Gates

- After Phase A: confirm near-complete clusters are green.
- After Phase B: confirm `kill_service` and `doctor_runtime` tests pass without live daemon/tmux.
- Before Phase C step 7: explicit user approval or stop-and-report if socket/watch changes are required.
- Before `task.py archive`: full `cargo test -p ccb-cli` and matrix update.
