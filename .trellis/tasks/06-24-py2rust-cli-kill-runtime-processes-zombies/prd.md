# py2rust CLI kill runtime processes/zombies parity

## Goal

Align Rust `ccb-cli::kill_runtime::processes` and `kill_runtime::zombies` with Python `lib/cli/kill_runtime/processes.py` and `zombies.py` so that the tests in `test_cli_kill_runtime_processes.py` and `test_cli_kill_runtime_zombies.py` have passing Rust equivalents.

## Requirements

1. Extend `crates/ccb-cli/src/kill_runtime/processes.rs`:
   - Add `kill_pid_tree_once(pid, force)` that mirrors Python `_kill_pid_tree_once`:
     - On Windows: build `taskkill /T /PID` args (with optional `/F`) and run.
     - On POSIX: try signaling the process group via `killpg`; fall back to `kill_pid`.
   - Refactor `terminate_pid_tree` to use `kill_pid_tree_once` for parity with Python's retry/force flow.
   - Expose `proc_pid_state(pid)` and optionally `proc_pid_state_at(pid, proc_root)` for testability.
   - Keep existing `kill_pid`, `is_pid_alive`, `wait_for_pid_exit` behavior; ensure `is_pid_alive` treats `/proc/{pid}/stat` state `Z` as dead and `D` as alive.

2. Implement `crates/ccb-cli/src/kill_runtime/zombies.rs`:
   - `find_all_zombie_sessions(is_pid_alive, list_tmux_sessions_fn)` matching Python.
   - `kill_global_zombies(yes, is_pid_alive, find_all_zombie_sessions_fn, input_fn, kill_tmux_session_fn)` matching Python, including prompts and result messages.
   - Internal helpers: `list_tmux_sessions`, `kill_tmux_session`, `parse_zombie_session`, `print_zombie_sessions`, `confirm_cleanup`, `cleanup_zombie_sessions`, `print_cleanup_result`.
   - Regex pattern: `^(codex|gemini|opencode|claude|droid|agy|kimi|deepseek)-(\d+)-`.

3. Add integration tests:
   - `crates/ccb-cli/tests/kill_runtime_processes_tests.rs` covering:
     - `kill_pid_tree_once` on POSIX prefers process group.
     - `kill_pid_tree_once` on Windows uses taskkill.
     - `is_pid_alive` treats zombie state as dead and uninterruptible state as alive.
     - `collect_project_process_candidates` and `collect_project_authority_pid_candidates` (already implemented in `ccb-runtime-pid-cleanup`; tests assert CLI wrapper parity).
   - `crates/ccb-cli/tests/kill_runtime_zombies_tests.rs` covering the four Python zombie tests.

## Acceptance Criteria

- `cargo test -p ccb-cli --test kill_runtime_processes_tests -- --test-threads=1` passes.
- `cargo test -p ccb-cli --test kill_runtime_zombies_tests -- --test-threads=1` passes.
- `cargo fmt --all -- --check` passes.
- `cargo clippy -p ccb-cli --tests` produces no new warnings.
- `plans/rust-python-test-parity-matrix.md` updated to note the two Python tests are covered.

## Out of Scope

- Live process termination cross-checks (keep mocked/subprocess tests).
- Changes to ccbd control-plane protocol, mailbox kernel, tmux namespace, or provider hook injection.
