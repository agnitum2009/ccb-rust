# Implementation Plan: py2rust CLI daemon keeper runtime parity

## Ordered Checklist

1. Read Python `test_cli_daemon_keeper_runtime.py` and `lib/cli/services/daemon_runtime/keeper.py`.
2. Add `KeeperContext` and keeper-state helpers to `crates/ccb-cli/src/services/daemon_runtime/keeper.rs`.
3. Implement `spawn_keeper_process` / `spawn_keeper_process_with`.
4. Implement `ensure_keeper_started_for_context` with all injection closures.
5. Add `wait_for_keeper_ready_for_context` and `keeper_state_is_running_for_context`.
6. Create `crates/ccb-cli/tests/daemon_keeper_runtime_tests.rs`.
7. Update `plans/rust-python-test-parity-matrix.md`.
8. Run validation commands.
9. Commit and archive task.

## Validation Commands

```bash
cargo test -p ccb-cli --test daemon_keeper_runtime_tests -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

## Risky Files / Rollback Points

- `crates/ccb-cli/src/services/daemon_runtime/keeper.rs`: keep existing `ensure_keeper_started`/`wait_for_keeper_ready` signatures used by `facade.rs` and `lifecycle.rs`; add new context-aware functions rather than breaking callers.
