# Terminal Runtime Cluster — Python↔Rust Consistency Audit

**Scope:** `terminal_runtime` cluster + namespace/state integration, per `plans/rust-python-test-parity-matrix.md` and `.trellis/tasks/06-24-py2rust-consistency-closure/prd.md` §3.

**Auditor:** Kimi Code CLI (exploration sub-agent)  
**Date:** 2026-06-24  
**Branch:** `python-rust/rolepacks-versioning-translation`

## Verification run

```text
cargo test -p ccb-terminal -- --test-threads=1   →  102 passed, 2 ignored, 0 failed
cargo test -p ccb-daemon   -- --test-threads=1   →  244 passed, 2 ignored, 0 failed
```

All existing Rust tests in the two relevant crates pass.  The gaps below are **missing coverage or behavioral mismatches**, not failing tests.

## Examined files

### Python reference
- `test/test_ccbd_tmux_namespace.py`
- `test/test_ccbd_tmux_state.py`
- `test/test_ccbd_start_runtime_layout.py`
- `test/test_terminal_runtime_backend_env.py`
- `test/test_terminal_runtime_backend_selection.py`
- `test/test_terminal_runtime_layouts.py`
- `test/test_terminal_runtime_tmux.py`
- `test/test_terminal_runtime_tmux_attach.py`
- `test/test_terminal_runtime_tmux_input.py`
- `test/test_terminal_runtime_tmux_logs.py`
- `test/test_terminal_runtime_tmux_panes.py`
- `test/test_terminal_runtime_tmux_respawn.py`
- `test/test_terminal_runtime_tmux_respawn_service.py`
- `test/test_terminal_runtime_tmux_send.py`
- `test/test_detect_terminal.py` (related, matrix mapped)
- `lib/ccbd/services/health_assessment/tmux_runtime/{namespace,state,backend,ownership}.py`
- `lib/ccbd/services/project_namespace_runtime/` (controller, controller_state, ensure, ensure_state, backend, records, models, materialize_topology, topology_plan, additive_patch_*, remove_patch_*)

### Rust implementation / tests
- `rust/crates/ccb-daemon/tests/tmux_runtime_namespace_tests.rs`
- `rust/crates/ccb-daemon/tests/tmux_runtime_state_tests.rs`
- `rust/crates/ccb-daemon/tests/start_runtime_layout_tests.rs`
- `rust/crates/ccb-daemon/src/services/health_assessment/tmux_runtime/{namespace,state,backend,ownership}.rs`
- `rust/crates/ccb-daemon/src/services/project_namespace_runtime/` (mirror files, see note below)
- `rust/crates/ccb-daemon/tests/project_namespace_controller_tests.rs`
- `rust/crates/ccb-daemon/tests/project_namespace_state_tests.rs`
- `rust/crates/ccb-daemon/tests/project_namespace_topology_plan_tests.rs`
- `rust/crates/ccb-terminal/tests/{backend_env,backend_selection,detect_terminal,test_backend,test_layouts,test_pane_service,test_respawn,test_tmux_helpers,tmux_backend,tmux_identity}_tests.rs`
- `rust/crates/ccb-terminal/src/{backend.rs,backend_env.rs,input.rs,logs.rs,respawn.rs,tmux.rs,tmux_attach.rs,panes.rs}`

## Gap table: Python test → Rust test

| Python test / assertion | Rust equivalent | Gap? | Recommended action |
|---|---|---|---|
| `test_ccbd_tmux_namespace.py` (5 tests) | `rust/crates/ccb-daemon/tests/tmux_runtime_namespace_tests.rs` (5 tests, same names) | No | Keep; update matrix to `complete` for this file. |
| `test_ccbd_tmux_state.py` (3 tests) | `rust/crates/ccb-daemon/tests/tmux_runtime_state_tests.rs` (3 tests, same names) + inline tests in `state.rs` | No | Keep. |
| `test_ccbd_start_runtime_layout.py` (2 tests) | `rust/crates/ccb-daemon/tests/start_runtime_layout_tests.rs` (2 tests, same names) | No | Keep. |
| `test_terminal_runtime_backend_env.py` (3 tests) | `rust/crates/ccb-terminal/tests/backend_env_tests.rs` (3 tests) | No | Keep. |
| `test_terminal_runtime_backend_selection.py` (3 tests) | `rust/crates/ccb-terminal/tests/backend_selection_tests.rs` (3 tests) | No | Keep. |
| `test_terminal_runtime_layouts.py` (3 tests) | `rust/crates/ccb-terminal/tests/test_layouts.rs` (4 tests, including extra topology cases) | No | Keep; Rust already covers all Python assertions plus additional topologies. |
| `test_terminal_runtime_tmux.py::test_tmux_base_includes_socket_when_present` | `test_tmux_helpers.rs::test_tmux_base_includes_socket_when_present` | Partial | Rust test omits the `socket_path="~/.tmux/demo.sock"` (`-S`) branch. Add assertion. |
| `test_terminal_runtime_tmux.py::test_tmux_base_allows_managed_config_override` | None | **Yes** | Add Rust test for `CCB_TMUX_CONFIG` override in `rust/crates/ccb-terminal/src/tmux.rs` / `test_tmux_helpers.rs`. |
| `test_terminal_runtime_tmux.py::test_tmux_target_helpers` | `test_tmux_helpers.rs::test_tmux_target_helpers` | No | Keep. |
| `test_terminal_runtime_tmux.py::test_tmux_socket_name_helpers` | `test_tmux_helpers.rs::test_tmux_socket_name_helpers` | No | Keep. |
| `test_terminal_runtime_tmux.py::test_normalize_split_direction` | `test_tmux_helpers.rs::test_normalize_split_direction` + `test_normalize_split_direction_left_panics` | No | Behavior equivalent (panic vs `ValueError`). |
| `test_terminal_runtime_tmux.py::test_pane_id_by_title_marker_output_*` | `test_tmux_helpers.rs::test_pane_id_by_title_marker_output` | No | Combined into one Rust test. |
| `test_terminal_runtime_tmux.py::test_default_detached_session_name_is_stable_format` | `test_tmux_helpers.rs::test_default_detached_session_name_is_stable_format` | No | Keep. |
| `test_terminal_runtime_tmux_attach.py::test_tmux_attach_helpers` | None | **Yes** | `rust/crates/ccb-terminal/src/tmux_attach.rs` is a stub. Implement `normalize_user_option`, `pane_exists_output`, `pane_pipe_enabled`, `pane_is_alive`, `parse_session_name`, fix `should_attach_selected_pane` sign, and add tests in `ccb-terminal/tests/`. |
| `test_terminal_runtime_tmux_input.py` (2 tests) | `ccb-terminal/src/input.rs` inline tests | No | Keep. |
| `test_terminal_runtime_tmux_logs.py::test_tmux_pane_log_manager_ensures_log_and_tracks_info` | `ccb-terminal/src/logs.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_logs.py::test_tmux_pane_log_manager_refreshes_only_when_pipe_missing` | None | **Yes** | Add Rust test for `refresh_pane_logs` skipping panes whose `#{pane_pipe}` is already `1`. |
| `test_terminal_runtime_tmux_logs.py::test_tmux_pane_log_manager_skips_dead_panes_during_refresh` | None | **Yes** | Add Rust test that `refresh_pane_logs` skips panes where `is_alive` returns false. |
| `test_terminal_runtime_tmux_panes.py` (5 tests) | `rust/crates/ccb-terminal/tests/test_pane_service.rs` (5 tests) | No | Keep. |
| `test_terminal_runtime_tmux_respawn.py::test_normalize_start_dir` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn.py::test_append_stderr_redirection_creates_parent` | None | **Yes** | Add Rust test that `append_stderr_redirection` creates parent directories and returns the canonicalized path. |
| `test_terminal_runtime_tmux_respawn.py::test_resolve_shell_*` | `ccb-terminal/src/respawn.rs` inline tests | No | Keep. |
| `test_terminal_runtime_tmux_respawn.py::test_resolve_shell_flags_defaults` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn.py::test_build_shell_command_quotes_arguments` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn.py::test_build_respawn_tmux_args` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn_service.py::test_tmux_respawn_service_builds_respawn_and_remain_calls` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn_service.py::test_tmux_respawn_service_requires_pane_and_cmd` | `ccb-terminal/src/respawn.rs` inline test | No | Keep. |
| `test_terminal_runtime_tmux_respawn_service.py::test_tmux_respawn_service_retries_transient_tmux_failures` | `ccb-terminal/src/respawn.rs` + `tests/test_respawn.rs` | Partial | Python test is parametrized over three stderr messages (`fork failed`, `no server running`, `server exited unexpectedly`). Rust only exercises one. Expand test to cover all three. |
| `test_terminal_runtime_tmux_respawn_service.py::test_tmux_respawn_service_uses_shared_ready_budget_for_transient_failures` | None | **Yes** | Add Rust test verifying that `CCB_TMUX_OBJECT_READY_TIMEOUT_S` is used as a shared deadline and the retry loop stops when it expires (Python expects 15 attempts with 0.1 s ticks and 1.5 s budget). |
| `test_terminal_runtime_tmux_respawn_service.py::test_tmux_respawn_service_does_not_retry_non_transient_failure` | `ccb-terminal/src/respawn.rs` + `tests/test_respawn.rs` | No | Keep. |
| `test_terminal_runtime_tmux_send.py::test_tmux_text_sender_deletes_buffer_after_paste_failure` | None | **Yes** | Add Rust test that `send_text` deletes the tmux buffer even when `paste-buffer` returns an error. |
| `test_terminal_runtime_tmux_send.py::test_tmux_text_sender_uses_inline_legacy_mode_for_session_targets` | `ccb-terminal/src/input.rs` inline test | No | Keep. |
| `test_detect_terminal.py` (3 tests) | `rust/crates/ccb-terminal/tests/detect_terminal_tests.rs` (3 tests) | No | Keep. |

## Source behavior differences

### 1. `tmux_attach` helpers are a stub with an inverted predicate
- Python: `terminal_runtime/tmux_attach.py` implements `normalize_user_option`, `pane_exists_output`, `pane_pipe_enabled`, `pane_is_alive`, `parse_session_name`, and `should_attach_selected_pane`.
- Rust: `ccb-terminal/src/tmux_attach.rs` only contains a placeholder `attach_to_session` and `should_attach_selected_pane`.
- **Mismatch:** `should_attach_selected_pane(env_tmux="")` returns `True` in Python (attach when not already inside tmux) but `false` in Rust (`!env_tmux.trim().is_empty()`), i.e. the sign is reversed.
- **Impact:** Any caller that relies on this predicate will make the wrong attach decision.

### 2. Namespace socket fallback comparison may differ on equivalent paths
- Python `health_assessment/tmux_runtime/namespace.py::_runtime_socket_matches_namespace` uses `same_tmux_socket_path` from `ccbd.services.project_namespace_pane` backend, which can resolve symlinks / normalize paths.
- Rust `health_assessment/tmux_runtime/namespace.rs::runtime_socket_matches_namespace` uses direct string equality `Some(runtime_socket) == tmux_socket_path`.
- **Impact:** Low for tests (strings are identical), but a real runtime gap if the runtime socket path and persisted namespace socket path differ lexically while referring to the same socket (symlink, relative path, etc.).

### 3. `project_namespace_runtime` has several 1:1 file-alignment stubs that are not exercised
- `controller_state.rs`, `ensure_state.rs`, `additive_patch*.rs`, `remove_patch_*.rs`, `patch_validation_*.rs`, `slot_replacement.rs`, `records.rs` header claims to be a stub, etc. are mostly empty placeholders.
- However, the controller/ensure/materialize-topology path is implemented in `controller.rs`, `ensure.rs`, `materialize_topology.rs`, `backend.rs`, `records.rs`, `ensure_identity.rs`, `reflow.rs`, `sidebar_helper.rs`, and `topology_plan.rs`, and the existing Rust project-namespace tests pass.
- **Conclusion:** The stubs do not currently break the tested namespace/state integration path, but they are dead weight and could be misleading. They should not be deleted per the PRD stop-rule, but no terminal-runtime gap is blocked by them.

### 4. `tmux_pane_state` error handling
- Python `state.py::pane_existence_state` swallows backend exceptions and returns `"missing"`.
- Rust `state.rs::pane_existence_state` returns `"missing"` only when `pane_exists` returns `false`; if the backend method itself errors, the trait returns `bool`, so the error handling is pushed into implementations.
- **Impact:** No test currently exercises a backend that throws from `pane_exists`; behavior is effectively equivalent for the tested surface.

## Real gaps and recommended actions

The following gaps are small, well-scoped, and can be closed without touching the ccbd control-plane protocol or provider hook paths:

1. **Fix and test `tmux_attach.rs`**  
   - Implement the helper functions and fix `should_attach_selected_pane` sign.  
   - Add `rust/crates/ccb-terminal/tests/tmux_attach_tests.rs` mirroring `test_tmux_attach_helpers`.  
   - Files: `rust/crates/ccb-terminal/src/tmux_attach.rs`, `rust/crates/ccb-terminal/src/tmux.rs` (for parser helpers if needed), new test file.

2. **Complete `tmux` helper test coverage**  
   - Add `CCB_TMUX_CONFIG` managed-config override test.  
   - Add `-S` socket_path assertion to `test_tmux_base_includes_socket_when_present`.  
   - File: `rust/crates/ccb-terminal/tests/test_tmux_helpers.rs`.

3. **Close `tmux_respawn_service` retry edge-cases**  
   - Parametrize transient-failure test over all three Python stderr patterns.  
   - Add shared-ready-budget / timeout test for `CCB_TMUX_OBJECT_READY_TIMEOUT_S`.  
   - File: `rust/crates/ccb-terminal/tests/test_respawn.rs` and/or `src/respawn.rs` inline tests.

4. **Add `tmux_logs` refresh tests**  
   - Test `refresh_pane_logs` skips panes with existing pipe and dead panes.  
   - File: `rust/crates/ccb-terminal/src/logs.rs` inline tests.

5. **Add `tmux_send` failure-cleanup test**  
   - Test that the buffer is deleted after `paste-buffer` fails.  
   - File: `rust/crates/ccb-terminal/src/input.rs` inline tests.

6. **Add `respawn` stderr-redirection test**  
   - Test `append_stderr_redirection` creates parent directories.  
   - File: `rust/crates/ccb-terminal/src/respawn.rs` inline tests.

7. **(Optional / low priority) align namespace socket comparison**  
   - If real paths can be symlinks, port `same_tmux_socket_path` semantics into `tmux_runtime/namespace.rs`.

## Overall assessment

- The **core namespace/state integration functions** (`pane_outside_project_namespace`, `tmux_pane_state`, `cmd_bootstrap_command`) are at parity with their Python tests.
- The **terminal_runtime helper layer** has a handful of small, real gaps, mostly in attach helpers, respawn retry semantics, log refresh, and send failure cleanup.
- The **project_namespace_runtime controller/ensure/materialize path** is functionally implemented and green; the remaining stub files are non-blocking file-alignment placeholders.
- **Recommended matrix update:** mark `test_ccbd_tmux_namespace.py`, `test_ccbd_tmux_state.py`, `test_ccbd_start_runtime_layout.py`, and most `terminal_runtime_*` files as `complete`; keep `terminal_runtime` cluster `partial` only until the six small gaps above are closed.
