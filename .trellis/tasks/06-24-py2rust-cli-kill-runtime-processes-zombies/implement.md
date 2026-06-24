# Implementation Plan: py2rust CLI kill runtime processes/zombies parity

## Ordered Checklist

1. Read Python `test_cli_kill_runtime_processes.py`, `test_cli_kill_runtime_zombies.py`, and the existing Rust `kill_runtime/processes.rs`.
2. Add `kill_pid_tree_once` and process-group helpers to `crates/ccb-cli/src/kill_runtime/processes.rs`; refactor `terminate_pid_tree` to use it.
3. Make `proc_pid_state` testable (public + proc-root variant) and add an `is_pid_alive_with` internal variant for state injection.
4. Implement `crates/ccb-cli/src/kill_runtime/zombies.rs` with all public/internal helpers.
5. Create `crates/ccb-cli/tests/kill_runtime_processes_tests.rs` mirroring the Python process tests.
6. Create `crates/ccb-cli/tests/kill_runtime_zombies_tests.rs` mirroring the Python zombie tests.
7. Update `plans/rust-python-test-parity-matrix.md` to note parity.
8. Run validation commands.
9. Commit and archive task.

## Validation Commands

```bash
cargo test -p ccb-cli --test kill_runtime_processes_tests -- --test-threads=1
cargo test -p ccb-cli --test kill_runtime_zombies_tests -- --test-threads=1
cargo fmt --all -- --check
cargo clippy -p ccb-cli --tests
```

## Risky Files / Rollback Points

- `crates/ccb-cli/src/kill_runtime/processes.rs`: changing `terminate_pid_tree` affects `kill_service` and `shutdown` paths; keep behavior equivalent.
- `crates/ccb-cli/src/kill_runtime/zombies.rs`: new code only, no callers yet.
