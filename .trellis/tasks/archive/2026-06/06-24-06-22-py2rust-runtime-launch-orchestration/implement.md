# W2: runtime launch orchestration — implementation plan

## Phase A: provider launcher branches (1–2 sessions)

1. **Inspect provider APIs**
   - Confirm `ccb-providers` exposes `prepare_launch_context`, `build_start_cmd`, `build_session_payload` for codex / claude / gemini / agy / droid.
   - Note each provider's session file path convention.

2. **Extend `provider_launcher.rs`**
   - Add `build_codex_launch`, `build_claude_launch`, `build_gemini_launch`, `build_agy_launch`, `build_droid_launch`.
   - Reuse existing `prepare_launch_context` / `build_session_payload` / `build_start_cmd` signatures.
   - Use `runtime_dir = project_root/.ccb/runtime/<agent>/<provider>`.
   - Write session file with `serde_json::to_string`.

3. **Tests**
   - `crates/ccb-daemon/tests/provider_launcher_codex_claude_tests.rs`:
     - codex launch produces command containing `codex`, session path ends with `codex-session.jsonl`.
     - claude launch produces command containing `claude`, session path under `.ccb/runtime/<agent>/claude/`.

## Phase B: ensure_agent_runtime orchestrator (2–3 sessions)

1. **Define orchestrator signature**
   - Add `ensure_agent_runtime` to `ccb-daemon/src/start_runtime/agent_runtime.rs` or new `ccb-daemon/src/start_runtime/ensure_runtime.rs`.
   - Use dependency-injection traits for tmux backend, provider launcher resolution, workspace prep, session file write, binding refresh.

2. **Implement reusable-binding check**
   - `binding_runtime_alive(binding)` helper using tmux backend.
   - If reusable, return `RuntimeLaunchResult { launched: false, binding: Some(binding.clone()) }`.

3. **Implement launch path**
   - Resolve `ProviderRuntimeLauncher`.
   - `prepare_provider_workspace` (layout, spec, workspace, runtime_dir).
   - `ProviderLauncher::launch(ctx)` or direct `build_launch_plan` + pane send.
   - If `assigned_pane_id` present → respawn; else detached fallback.
   - Apply pane identity.
   - Write session file.
   - Resolve refreshed binding.

4. **Tests**
   - `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`:
     - `test_reuse_alive_binding`.
     - `test_launch_creates_pane_and_session_file`.
     - `test_stale_binding_kills_old_pane`.
     - `test_detached_fallback_when_no_assigned_pane`.

## Phase C: integration with `start_agent_runtime` (1 session)

1. Wire `ensure_agent_runtime_fn` trait implementation to call the new orchestrator.
2. Ensure `start_agent_runtime` end-to-end test passes with mock `RuntimeService` and mock `EnsureAgentRuntimeFn`.

## Phase D: matrix & wrap-up

1. Update `plans/rust-python-test-parity-matrix.md` `runtime_launch` row.
2. Run final validation:
   - `cargo fmt -p ccb-daemon`
   - `cargo clippy -p ccb-daemon --tests -- -D warnings`
   - `cargo test -p ccb-daemon -- --test-threads=1`
3. Archive task via `task.py archive` + `add_session.py` + `codegraph sync`.

## Review gates

- Each new provider branch must have at least 2 shape assertions.
- `ensure_agent_runtime` tests must cover reuse, launch, stale, detached paths.
- No new clippy errors.
