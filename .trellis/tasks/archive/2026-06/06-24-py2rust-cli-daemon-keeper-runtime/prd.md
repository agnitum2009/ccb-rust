# py2rust CLI daemon keeper runtime parity

## Goal

Align Rust `ccb-cli::services::daemon_runtime::keeper` with Python `lib/cli/services/daemon_runtime/keeper.py` so that `test_cli_daemon_keeper_runtime.py` has passing Rust equivalents.

## Requirements

1. Extend `crates/ccb-cli/src/services/daemon_runtime/keeper.rs`:
   - Add `KeeperContext` struct with `project_id`, `project_root`, and `paths`.
   - Add `spawn_keeper_process(context)` that mirrors Python:
     - Computes `lib_root` as the project root (equivalent to `Path(__file__).resolve().parents[3]`).
     - Builds command `[python, lib_root/ccbd/keeper_main.py, --project, project_root]`.
     - Prepends `lib_root` to `PYTHONPATH`.
     - Ensures runtime state root and ccbd dir exist.
     - Redirects stdout/stderr to `ccbd_dir/keeper.stdout.log` and `keeper.stderr.log`.
     - Runs with `start_new_session=True` equivalent (`setsid` on Unix).
   - Provide `spawn_keeper_process_with(context, spawn_fn)` for tests so the spawn can be captured without a real subprocess.
   - Implement `ensure_keeper_started_for_context(context, ...)` with injection closures matching Python's test parameters:
     - `mount_manager_factory(paths) -> M`
     - `ownership_guard_factory(paths, manager) -> G`
     - `process_exists_fn(pid) -> bool`
     - `process_cmdline_fn(pid) -> Vec<String>`
     - `spawn_keeper_process_fn(ctx)`
     - `ready_timeout_s`
   - The function should load keeper state, check `keeper_state_is_running_for_context`, acquire startup lock, recheck, spawn, and wait.
   - Add `keeper_state_is_running_for_context` and `wait_for_keeper_ready_for_context` helpers.

2. Add `crates/ccb-cli/tests/daemon_keeper_runtime_tests.rs` mirroring the three Python tests:
   - `spawn_keeper_process` captures command and PYTHONPATH.
   - `ensure_keeper_started` replaces state for unrelated live PID.
   - `ensure_keeper_started` reuses matching keeper state.

## Acceptance Criteria

- `cargo test -p ccb-cli --test daemon_keeper_runtime_tests -- --test-threads=1` passes.
- `cargo fmt --all -- --check` passes.
- `cargo clippy -p ccb-cli --tests` produces no new warnings.
- `plans/rust-python-test-parity-matrix.md` updated to note `test_cli_daemon_keeper_runtime.py` is covered.
- Changes committed and Trellis task archived.

## Out of Scope

- Real keeper process lifecycle (tests use mocked spawn).
- Changes to ccbd control-plane protocol or mailbox kernel contracts.
