# Wave 3: Deep Stub Reduction — `ccb-providers` + `ccb-daemon`

## Problem

The CCB Python→Rust migration is tracked in `.trellis/spec/migration-roadmap.md` as four dependency-ordered waves. Wave 1 (CLI `Phase2Services` impl) and Wave 2 (runtime launch + completion/heartbeat) must be closed before Wave 3 because the provider/daemon deep code is only reachable once the CLI dispatches commands and the daemon can start agent runtimes.

Current stub inventory (measured with `grep -rln "TODO: align with Python"` on 2026-06-24):

| Crate | Current stub files | Share of workspace stubs |
|-------|-------------------|--------------------------|
| `ccb-providers` | **368** | ~35% |
| `ccb-daemon` | **345** | ~33% |
| **Wave 3 total** | **713** | ~68% |

The roadmap snapshot from 2026-06-22 cited 463/348; the current lower numbers reflect Wave 1/2 stub consolidation. These two crates are the last large block of `TODO: align with Python` placeholders.

Stubs are not uniformly distributed:

- `ccb-providers`: 200 top-level module stubs, 0 stubs in the six provider subdirs under `src/providers/`, 3 stubs in the legacy `src/agy/` tree, 8 in `src/common_runtime/`, 4 in `src/pane_log_support/`, plus the duplicate provider runtime trees under `src/codex/`, `src/claude/`, `src/droid/`, `src/opencode/`.
- `ccb-daemon`: 166 top-level module stubs, 63 in `src/services/dispatcher_runtime/`, 14 in `src/services/project_namespace_runtime/`, 7 in `src/supervision/`, 2 in `reload_*.rs` files.

Without Wave 3 the Rust workspace has the shell of a multi-agent runtime but lacks parity for provider execution adapters, dispatcher lifecycle, namespace materialization, config reload, and supervision recovery.

## Scope

Wave 3 is split into provider sub-themes and daemon sub-themes. Each sub-theme is a self-contained, testable unit. The work is organized by functional dependency, not by individual stub file.

### Provider sub-themes

1. **Provider adapter surface and registry parity**
   - Canonicalize the split between `src/providers/<name>.rs` (adapter surface) and `src/<name>/` (supporting modules).
   - Remove or merge the legacy duplicate `src/agy/` stub tree.
   - Ensure `build_default_backend_registry()` and `build_default_execution_registry()` register every targeted provider with a non-`None` `execution_adapter`.

2. **Codex execution adapter parity**
   - Files: `rust/crates/ccb-providers/src/providers/codex.rs`, `rust/crates/ccb-providers/src/codex/launcher.rs`, `rust/crates/ccb-providers/src/codex/launcher_runtime/*`
   - Python reference: `lib/provider_backends/codex/execution.py`, `lib/provider_backends/codex/execution_runtime/*`
   - Behaviors: `start_active_submission`, `poll_submission`, delivery-acceptance guard, reader refresh, session rotation, task-complete / turn-aborted terminal decisions.

3. **Claude execution / comm / binding parity**
   - Files: `rust/crates/ccb-providers/src/providers/claude.rs`, `rust/crates/ccb-providers/src/claude/comm_runtime/*`, `rust/crates/ccb-providers/src/claude/session.rs`, `rust/crates/ccb-providers/src/claude/launcher_runtime/*`
   - Python reference: `lib/provider_backends/claude/execution.py`, `lib/provider_backends/claude/comm_runtime/*`, `lib/provider_backends/claude/session.py`
   - Behaviors: prompt ready-wait, deferred send, exact hook polling, event-batch polling, session rotation, API-error terminal detection, reply-delivery short-circuit.

4. **Gemini execution adapter parity**
   - Files: `rust/crates/ccb-providers/src/providers/gemini/mod.rs`, `rust/crates/ccb-providers/src/providers/gemini/log_reader.rs`, `rust/crates/ccb-providers/src/providers/gemini/launcher.rs`
   - Python reference: `lib/provider_backends/gemini/execution.py`, `lib/provider_backends/gemini/log_reader.py`, `lib/provider_backends/gemini/launcher.py`
   - Behaviors: tmp-root discovery, log-reader state capture, prompt send, anchor emission, assistant-final / turn-boundary decisions.

5. **Droid execution adapter parity**
   - Files: `rust/crates/ccb-providers/src/providers/droid.rs`, `rust/crates/ccb-providers/src/droid/execution_runtime/*`, `rust/crates/ccb-providers/src/droid/comm.rs`, `rust/crates/ccb-providers/src/droid/session.rs`
   - Python reference: `lib/provider_backends/droid/execution.py`, `lib/provider_backends/droid/execution_runtime/*`, `lib/provider_backends/droid/comm.py`
   - Behaviors: terminal-text log polling, `<<DONE:req_id>>` extraction, reply buffer merging, turn boundary.

6. **AGY execution adapter parity**
   - Files: `rust/crates/ccb-providers/src/providers/agy/mod.rs`, `rust/crates/ccb-providers/src/providers/agy/native_log.rs`, `rust/crates/ccb-providers/src/providers/agy/session.rs`, `rust/crates/ccb-providers/src/agy/*` (legacy stubs to merge/remove)
   - Python reference: `lib/provider_backends/agy/execution.py`, `lib/provider_backends/agy/execution_runtime/*`, `lib/provider_backends/agy/native_log.py`
   - Behaviors: native transcript observation, SHA-1 reply deduplication, session-rotate on transcript path change, anchor wait timeout, terminal decision on completion.

7. **OpenCode execution adapter parity**
   - Files: `rust/crates/ccb-providers/src/providers/opencode.rs`, `rust/crates/ccb-providers/src/opencode/session.rs`, `rust/crates/ccb-providers/src/opencode/runtime/*`, `rust/crates/ccb-providers/src/opencode/execution_runtime/*`
   - Python reference: `lib/provider_backends/opencode/execution.py`, `lib/provider_backends/opencode/session.py`, `lib/provider_backends/opencode/runtime/*`
   - Behaviors: storage-root resolution, session filtering by ID, message reader state capture, req-id matching, assistant completion detection.

8. **Shared provider infrastructure**
   - Files: `rust/crates/ccb-providers/src/common_runtime/*`, `rust/crates/ccb-providers/src/pane_log_support/*`, `rust/crates/ccb-providers/src/helper_cleanup.rs`, `rust/crates/ccb-providers/src/session_paths.rs`, `rust/crates/ccb-providers/src/workspace_preparation.rs`, `rust/crates/ccb-providers/src/model_shortcuts.rs`
   - Python reference: `lib/provider_backends/common_runtime/*`, `lib/provider_backends/pane_log_support/*`, `lib/provider_runtime/helper_cleanup.py`
   - Behaviors: serialization round-trips, pane log lifecycle recovery, helper manifest cleanup, provider instance resolution, runtime spec matching.

### Daemon sub-themes

1. **Dispatcher runtime lifecycle and polling**
   - Files: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/lifecycle.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/polling_service.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/completion.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/state*.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/records.rs`
   - Python reference: `lib/ccbd/services/dispatcher_runtime/lifecycle.py`, `lib/ccbd/services/dispatcher_runtime/polling_service.py`, `lib/ccbd/services/dispatcher_runtime/state.py`
   - Behaviors: `submit_jobs`, `tick_jobs`, `poll_completion_updates`, dispatcher state rebuild, attempt/job/record append.

2. **Dispatcher submission and routing**
   - Files: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_service.rs` (existing partial impl), `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission_recording.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/submission.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/routing.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/callbacks.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/comms_recover.rs`
   - Python reference: `lib/ccbd/services/dispatcher_runtime/submission_service.py`, `lib/ccbd/services/dispatcher_runtime/routing.py`, `lib/ccbd/services/dispatcher_runtime/callbacks.py`, `lib/ccbd/services/dispatcher_runtime/comms_recover.py`
   - Behaviors: `/ask` plan, target resolution, callback validation, broadcast draft expansion, retry attempt resolution, comms recoverability.

3. **Dispatcher finalization and reply delivery**
   - Files: `rust/crates/ccb-daemon/src/services/dispatcher_runtime/finalization*.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/reply_delivery*.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/visible_reply.rs`, `rust/crates/ccb-daemon/src/services/dispatcher_runtime/execution_cleanup.rs`
   - Python reference: `lib/ccbd/services/dispatcher_runtime/finalization.py`, `lib/ccbd/services/dispatcher_runtime/reply_delivery.py`
   - Behaviors: terminal decision merge, job completion, preparation message generation, reply delivery dispatch, visible reply state.

4. **Project namespace runtime materialization**
   - Files: `rust/crates/ccb-daemon/src/services/project_namespace_runtime/ensure.rs` (existing partial impl), `rust/crates/ccb-daemon/src/services/project_namespace_runtime/materialize_topology.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/backend.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/topology_plan.rs`, `rust/crates/ccb-daemon/src/services/project_namespace_runtime/additive_patch*.rs`
   - Python reference: `lib/ccbd/services/project_namespace_runtime/ensure.py`, `lib/ccbd/services/project_namespace_runtime/materialize_topology.py`, `lib/ccbd/services/project_namespace_runtime/additive_patch*.py`
   - Behaviors: namespace ensure with topology, tmux backend policy, additive agent/window patch apply, slot replacement, sidebar layout.

5. **Config reload orchestration**
   - Files: `rust/crates/ccb-daemon/src/reload_apply.rs`, `rust/crates/ccb-daemon/src/reload_apply_*.rs`, `rust/crates/ccb-daemon/src/reload_runtime_mount_*.rs`, `rust/crates/ccb-daemon/src/reload_transaction*.rs`, `rust/crates/ccb-daemon/src/reload_patch_*.rs`, `rust/crates/ccb-daemon/src/reload_plan.rs`, `rust/crates/ccb-daemon/src/reload_additive_agents.rs`
   - Python reference: `lib/ccbd/reload_apply.py`, `lib/ccbd/reload_apply_*.py`, `lib/ccbd/reload_transaction*.py`
   - Behaviors: dry-run plan, add/remove agent/window ops, future-safe apply, mount start/state/unload, transaction signature and rollback.

6. **Supervision and recovery**
   - Files: `rust/crates/ccb-daemon/src/supervision/loop_.rs`, `rust/crates/ccb-daemon/src/supervision/loop_*.rs`, `rust/crates/ccb-daemon/src/supervision/mount*.rs`, `rust/crates/ccb-daemon/src/supervision/recovery*.rs`, `rust/crates/ccb-daemon/src/supervision/backoff.rs`, `rust/crates/ccb-daemon/src/supervision/cmd_slot.rs`
   - Python reference: `lib/ccbd/supervision/loop.py`, `lib/ccbd/supervision/mount.py`, `lib/ccbd/supervision/recovery*.py`
   - Behaviors: supervision loop tick, mount event recording, recovery transitions, backoff, command-slot serialization.

7. **Daemon top-level service wiring**
   - Files: `rust/crates/ccb-daemon/src/*.rs` with `TODO: align with Python` (166 files)
   - Python reference: corresponding `lib/ccbd/*.py` files
   - Behaviors: triage each stub as implement/delete/defer, then implement the ones required by the dispatcher/namespace/reload/supervision themes above.

## Execution Order

Providers first, daemon second.

- Provider execution adapters are the data source for `ExecutionService` and the daemon dispatcher's `poll_completion_updates`. Stabilizing item kinds, cursor shapes, and terminal decisions in `ccb-providers` prevents cascading type changes in `ccb-daemon`.
- `ccb-daemon` dispatcher runtime depends on `ccb-providers` via `ProviderExecutionRegistry` and `ExecutionAdapter`.
- `project_namespace_runtime`, `reload`, and `supervision` can be developed in parallel once the provider surface is stable, but their integration tests will reuse provider adapters for end-to-end coverage.

## Acceptance Criteria

### Dependency gate
- [ ] Wave 1 (`py2rust-cli-services-impl`) closed: `impl Phase2Services` exists and core CLI commands dispatch.
- [ ] Wave 2 (`py2rust-runtime-launch-orchestration`, `py2rust-completion`) closed: `ensure_agent_runtime` and `CompletionTrackerService` are parity-complete for their mapped Python tests.

### Provider sub-themes
- [ ] Provider adapter registry registers Codex, Claude, Gemini, Droid, AGY, and OpenCode with non-`None` `execution_adapter`.
- [ ] Legacy duplicate `src/agy/` stubs resolved (merged into `src/providers/agy/` or deleted with `lib.rs`/`providers/mod.rs` imports updated).
- [ ] Codex adapter tests in `rust/crates/ccb-providers/tests/provider_codex_tests.rs` pass and cover delivery-acceptance guard + session rotation.
- [ ] Claude adapter tests in `rust/crates/ccb-providers/tests/provider_claude_tests.rs` pass and cover deferred-ready send + exact-hook + event-batch paths.
- [ ] Gemini adapter tests in `rust/crates/ccb-providers/tests/provider_gemini_tests.rs` pass and cover log-reader state capture + assistant-final decision.
- [ ] Droid adapter tests in `rust/crates/ccb-providers/tests/provider_droid_tests.rs` pass and cover `CCB_DONE` terminal extraction.
- [ ] AGY adapter tests in `rust/crates/ccb-providers/tests/provider_agy_tests.rs` pass and cover native transcript observation + reply deduplication.
- [ ] OpenCode adapter tests in `rust/crates/ccb-providers/tests/provider_opencode_tests.rs` pass and cover storage-root resolution + req-id matching.
- [ ] Shared provider infrastructure tests in `rust/crates/ccb-providers/tests/runtime_tests.rs`, `provider_helper_cleanup_tests.rs`, `provider_instance_resolution_tests.rs`, `provider_session_paths_tests.rs`, `workspace_preparation_tests.rs` pass.
- [ ] `ccb-providers` stub count reduced from **368** to **≤ 50**.

### Daemon sub-themes
- [ ] Dispatcher submission/routing tests in `rust/crates/ccb-daemon/tests/daemon_integration_tests.rs` pass and exercise `plan_submission`, target resolution, and broadcast draft expansion.
- [ ] Dispatcher lifecycle/polling tests pass and exercise `tick_jobs` + `poll_completion_updates` with mocked `ExecutionService`.
- [ ] Dispatcher finalization/reply-delivery tests pass and exercise terminal decision merge + preparation message generation.
- [ ] Project namespace tests in `rust/crates/ccb-daemon/tests/project_namespace_controller_tests.rs`, `project_namespace_topology_plan_tests.rs`, `project_namespace_state_tests.rs` pass.
- [ ] Reload tests in `rust/crates/ccb-daemon/tests/reload_tests.rs` pass and cover dry-run add/remove agent/window and future-safe apply.
- [ ] Supervision tests pass (new tests added if none exist) and cover loop tick + mount event recording.
- [ ] `ccb-daemon` stub count reduced from **345** to **≤ 50**.

### Quality gates
- [ ] `cargo check --workspace` is clean.
- [ ] `cargo test -p ccb-providers -- --test-threads=1` passes.
- [ ] `cargo test -p ccb-daemon -- --test-threads=1` passes.
- [ ] `cargo test --workspace -- --test-threads=1` passes.
- [ ] `cargo clippy --workspace --all-targets` reports 0 errors.
- [ ] `cargo fmt --check` is clean.
- [ ] `plans/rust-python-test-parity-matrix.md` updated with new mappings and stub-count baseline.

## Out of Scope

- Real provider CLI live tests (Codex, Claude, Gemini, etc.). Rust tests use mocked `PromptTarget` and fixture log files; live CLI tests remain in Python reference.
- Windows bootstrap / WSL path utilities. No Rust equivalents unless a follow-up wave explicitly requests them.
- Python wrapper scripts (`bin/ask`, `bin/autonew`, `bin/ctx-transfer`, `ccb`) are replaced by native Rust binaries; their internal logic is not reimplemented in Rust.
- Provider-specific Python hook scripts retained for source installs but not required in release artifacts.
- `Phase2Services` trait redesign or ccbd control-plane protocol changes — if required, escalate to a Wave 1/2 follow-up rather than folding into Wave 3.

## References

- `.trellis/tasks/06-24-py2rust-remaining-parity/design.md` — 4-wave dependency ordering.
- `.trellis/spec/migration-roadmap.md` — stub counts, dependency constraints, done criteria.
- `plans/rust-python-test-parity-matrix.md` — cluster mapping and current parity status.
- Python provider references: `lib/provider_backends/{codex,claude,gemini,droid,agy,opencode}/`.
- Python daemon references: `lib/ccbd/services/dispatcher_runtime/`, `lib/ccbd/services/project_namespace_runtime/`, `lib/ccbd/reload_apply*.py`, `lib/ccbd/supervision/`.
- Runtime launch reference: `lib/cli/services/runtime_launch_runtime/ensure.py`.
