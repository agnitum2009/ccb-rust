# Implementation Plan: Owner Alignment Gap Closure

## Phase A — Owner/RPC audit artifact

- [x] Enumerate Python registered ccbd ops.
- [x] Enumerate Rust registered ccbrd ops.
- [x] Confirm Python-only op set is empty.
- [x] Write `research/wire-protocol-gap.md` with owner surface, status, and priority.

## Phase B — P0 compatibility gaps

1. [x] `submit` delivery semantics
   - Read Python `handlers/submit.py`, Rust `handlers/submit.rs`, Rust `handlers/ask.rs`, dispatcher delivery path.
   - Added regression proving Python-style `submit` can drive Codex provider delivery on heartbeat:
     `app::tests::test_submit_heartbeat_delivers_codex_prompt_through_provider`.
   - Minimal fix: move Codex prompt dispatch back to the provider execution owner, matching Python.
     Rust-only `ask` now skips manual pane send when the provider already sent the prompt.
   - Validation:
     `cargo test -p ccbr-daemon submit -- --test-threads=1`,
     `cargo test -p ccbr-daemon ask -- --test-threads=1`,
     `cargo test -p ccbr-daemon -- --test-threads=1`,
     `cargo test -p ccbr-providers codex -- --test-threads=1`.

2. [x] `project_restart_panes`
   - Added regressions proving endpoint no longer returns no-op `scheduled` success:
     `test_restart_panes_without_agents_fails_loudly`,
     `test_restart_panes_triggers_start_flow_for_all_agents`.
   - Minimal fix: reuse existing `run_start_flow` to recreate the full agent topology and return structured `ok`/`failed`.
   - Validation: `cargo test -p ccbr-daemon project_restart -- --test-threads=1`.

3. [x] Sidebar/project_view contract sweep
   - Compare Python `project_view` required keys against Rust response.
   - Added regression proving ccbrd returns the sidebar-consumed Python shape:
     `test_project_view_matches_sidebar_wire_shape`.
   - Minimal fix: keep the existing aggregate handler, but fill the Python
     `{view, cache}` contract with `project`, `ccbd`, `namespace.sidebar.view`,
     window metadata, agent job/activity fields, and comms action fields.
   - Validation:
     `cargo test -p ccbr-daemon test_project_view_matches_sidebar_wire_shape -- --test-threads=1`,
     `cargo test -p ccbr-daemon -- --test-threads=1`,
     `(cd tools/ccb-agent-sidebar && cargo test -- --test-threads=1)`.

## Phase C — P1 parity gaps

- [x] `project_clear_context`: verify provider-specific clear semantics and structured response.
  - Added handler-level regressions:
    `project_clear_context_targets_all_agent_panes_with_provider_clear`,
    `project_clear_context_dedupes_requested_agents_and_rejects_unknown`,
    `project_clear_context_reports_missing_panes`.
  - Minimal fix: replace no-op success with real namespace-backed pane
    resolution, Python-compatible target normalization, `/clear` send-key
    sequence, OpenCode delayed submit, and per-agent `cleared/skipped/failed`
    results.
  - Validation:
    `cargo test -p ccbr-daemon project_clear -- --test-threads=1`.
- [x] `project_reload_config`: verify additive/reload shape vs Python.
  - Added payload-wrapper regressions:
    `non_dry_run_apply_payload_matches_python_reload_shape`,
    `published_reload_payload_is_marked_mutating_without_errors`.
  - Updated reload integration coverage to assert Python `published/blocked`
    apply shape, invalid-config non-dry-run diagnostics, registry/dispatcher
    read-model sync, and busy-agent remove blocking.
  - Minimal fix: route non-dry-run handler through the Python-aligned additive
    reload apply service, flatten apply payload fields at the socket boundary,
    publish successful config into Rust runtime read models, and implement the
    pre-namespace unload blocker for busy/outstanding removed agents.
  - Validation:
    `cargo test -p ccbr-daemon --test reload_tests -- --test-threads=1`,
    `cargo test -p ccbr-daemon project_reload -- --test-threads=1`.
- [x] `project_focus_*`: verify tmux targeting and response shape.
  - Added handler-level planning regressions:
    `focus_agent_plans_window_and_pane_selection`,
    `focus_tool_window_does_not_select_agent_pane`,
    `focus_rejects_stale_namespace_epoch`.
  - Minimal fix: replace no-op focus handlers with namespace epoch validation,
    window/agent lookup, tmux `select-window` + `select-pane`, Python-style
    success response, and best-effort sidebar refresh.
  - Validation:
    `cargo test -p ccbr-daemon project_focus -- --test-threads=1`,
    `cargo test -p ccbr-daemon -- --test-threads=1`.
- [x] `get` / `watch`: verify Python response envelopes and empty/error cases.
  - Added regressions:
    `handlers::get::tests::get_returns_python_result_payload_shape`,
    `handlers::get::tests::get_unknown_job_fails_like_python_handler`,
    `handlers::watch::tests::watch_uses_python_cursor_payload`,
    `handlers::watch::tests::watch_rejects_negative_cursor`.
  - Minimal fix: make `get` return Python-visible job/readback fields and
    fail on unknown jobs; make `watch` consume Python `cursor` and reject
    negative cursors while preserving `start_line` as a legacy alias.
  - Validation:
    `cargo test -p ccbr-daemon handlers::get::tests -- --test-threads=1`,
    `cargo test -p ccbr-daemon handlers::watch::tests -- --test-threads=1`,
    `cargo test -p ccbr-daemon test_watch_returns_activity_lines_for_target -- --test-threads=1`.
- [x] `queue` / `trace` / `cancel`: verify mailbox-control owner and Python trace shape.
  - Added trace regressions:
    `handlers::trace::tests::trace_rejects_legacy_all_target_like_python_handler`,
    `handlers::trace::tests::trace_missing_job_returns_error_instead_of_panicking`.
  - Minimal fix: make `trace` use mailbox-control trace only, add non-panicking
    `try_trace` in `ccbr-mailbox`, and remove the Rust-local dispatcher
    fallback for `all` / agent-name trace targets. Existing queue/cancel code
    already used mailbox-control/mailbox terminal state and remained covered by
    integration tests.
  - Validation:
    `cargo test -p ccbr-daemon handlers::trace::tests -- --test-threads=1`,
    `cargo test -p ccbr-daemon test_queue_returns_actual_per_agent_state -- --test-threads=1`,
    `cargo test -p ccbr-daemon test_trace_returns_job_history -- --test-threads=1`,
    `cargo test -p ccbr-daemon test_cancel_updates_mailbox_state -- --test-threads=1`,
    `cargo test -p ccbr-mailbox trace -- --test-threads=1`.
- [x] `inbox` / `mailbox_head` / `ack`: verify mailbox-control owner and Python ack payload.
  - Existing `inbox` and `mailbox_head` handlers already use mailbox-control state and Python agent/detail signatures.
  - Minimal fix: make `ack` consume Python `inbound_event_id`, while preserving Rust legacy `event_id` as an alias.
  - Validation:
    `cargo test -p ccbr-daemon test_ack_acknowledges_reply_event -- --test-threads=1`.
- [x] `stop-all` / `shutdown`: verify local workspace-exit owner and force semantics.
  - Existing `stop-all` calls the stop flow directly with the caller-provided force flag.
  - User-facing `shutdown` requests daemon shutdown; the daemon main loop then calls `CcbdApp::shutdown()`, which runs `stop_all(true, "shutdown")` and records `forced_cleanup`.
  - Validation:
    `cargo test -p ccbr-daemon test_start_stop_flow -- --test-threads=1`,
    `cargo test -p ccbr-daemon test_shutdown_handler_requests_shutdown -- --test-threads=1`,
    `cargo test -p ccbr-daemon app::tests::test_shutdown_forces_workspace_exit_cleanup --lib -- --test-threads=1`.
- [x] `resubmit` / `retry`: verify Python message-bureau lifecycle payloads.
  - Added handler-level regressions:
    `handlers::resubmit::tests::resubmit_recreates_message_with_python_payload_shape`,
    `handlers::resubmit::tests::resubmit_missing_message_fails_like_python_handler`,
    `handlers::retry::tests::retry_recreates_attempt_with_python_payload_shape`,
    `handlers::retry::tests::retry_missing_target_fails_like_python_handler`,
    `handlers::retry::tests::retry_active_attempt_is_rejected`.
  - Minimal fix: replace thin `resubmitted/retried/noop` stubs with mailbox
    owner-backed message/attempt lineage validation, dispatcher job enqueue,
    `record_submission` / `record_retry_attempt`, Python error conditions, and
    Python lifecycle response fields.
  - Validation:
    `cargo test -p ccbr-daemon handlers::resubmit::tests -- --test-threads=1`,
    `cargo test -p ccbr-daemon handlers::retry::tests -- --test-threads=1`.

## Phase D — Validation

- `cargo test -p ccbr-daemon -- --test-threads=1`
- `cargo test -p ccbr-providers -- --test-threads=1` when provider/session code changes.
- Live smoke in `/mnt/d/dapro-ass`:
  - start workspace
  - `ccbr-agent-sidebar` displays real state
  - `bin/ask` / `ccbr ask` path completes A -> B -> A inbox
  - red X/full shutdown leaves no managed tmux/provider remnants
  - do not treat Python `ask_cli` failing on `.ccb` mount state as a ccbr bug; that belongs to the Python / `ccb-legacy` bloodline

## Guardrails

- Do not disable any Codex hook.
- Do not import Python per-agent bridge/tight polling.
- Do not touch unrelated dirty files.
- Do not claim full owner parity until every row in `wire-protocol-gap.md` is marked `closed`, `accepted_divergence`, `blocked`, or `none_with_reason`.

## Phase B/C follow-up — ask/provider owner closure and ccb-legacy sync (2026-06-26)

Owner finding:

- The failing live ask path was no longer a daemon registration gap; it was split across CLI route parsing, provider session payload ownership, and Claude completion anchoring.
- CLI `ask agent3 from agent1 -- ...` must use the Python `submit` contract, not Rust-local `ask`.
- Claude JSONL records the user turn as `CCBR_REQ_ID: req-*`; Rust tracked `<<BEGIN:req-*>>`, so the provider could miss `anchor_seen` and ignore the assistant completion.
- Provider session payloads must include `tmux_socket_path` so provider-owned send/readback targets the managed workspace socket.
- User constraint: Codex hooks must remain enabled; no hook masking/disablement is allowed.
- Bloodline constraint: equivalent Rust fixes were synchronized to the separate `ccb-legacy` branch worktree at `/tmp/ccb-legacy-sync`; Python `ccb` / `.ccb` state was not modified.

Fix:

- `ccbr_test` now execs the debug Rust binary directly instead of running the shell wrapper through Python.
- `ccbr-cli` accepts Python-style ask route syntax and calls `submit` for CLI ask receipts.
- `ccbr-daemon` writes `tmux_socket_path` into simple-provider session payloads.
- `ccbr-providers` Claude keeps failed deferred sends visible, guards pane fallback against startup chrome, and accepts both `<<BEGIN:req-*>>` and `CCBR_REQ_ID: req-*` anchors.
- `scripts/ccbr-test-cleanup.sh` reclaims only ccbr runtime/state for explicit test roots and preserves Python `ccb` state.
- `ccb-legacy` was updated with the corresponding `ccb-*` crate changes, including the legacy Codex session payload owner so its provider code matches the daemon call shape.

Evidence:

- Main line:
  - `python3 -m py_compile ccbr_test`
  - `bash -n scripts/ccbr-test-cleanup.sh`
  - `cd rust && cargo fmt --check -p ccbr-cli -p ccbr-daemon -p ccbr-providers`
  - `cargo test -p ccbr-cli entry::tests::test_parse_ask -- --test-threads=1`
  - `cargo test -p ccbr-cli commands::tests::ask_uses_python_submit_contract -- --test-threads=1`
  - `cargo test -p ccbr-cli --test ask_service_tests -- --test-threads=1`
  - `cargo test -p ccbr-daemon provider_launcher::tests::test_simple_provider_session_payload_includes_tmux_socket_path -- --test-threads=1`
  - `cargo test -p ccbr-providers providers::claude::tests -- --test-threads=1`
  - `cargo build -p ccbr-cli -p ccbr-daemon -p ccbr-providers`
- `ccb-legacy` sync line (`/tmp/ccb-legacy-sync`):
  - `python3 -m py_compile ccbr_test`
  - `git diff --check`
  - `cargo test -p ccb-cli entry::tests::test_parse_ask -- --test-threads=1`
  - `cargo test -p ccb-cli commands::tests::ask_uses_python_submit_contract -- --test-threads=1`
  - `cargo test -p ccb-cli --test ask_service_tests -- --test-threads=1`
  - `cargo test -p ccb-daemon provider_launcher::tests::test_simple_provider_session_payload_includes_tmux_socket_path -- --test-threads=1`
  - `cargo test -p ccb-providers providers::claude::tests -- --test-threads=1`
  - `cargo build -p ccb-cli -p ccb-daemon -p ccb-providers`
- Formatting note: legacy full crate `cargo fmt --check` is blocked by pre-existing rustfmt drift in touched files; no broad formatting was applied to avoid unrelated churn.
- Resource cleanup evidence for `/mnt/d/dapro-ass`: no matching ccbrd/provider runtime processes, `~/.local/state/ccbr/projects/302a3b148cf77d3ecab65db7becea51f0c9abed4d7f43271afdc1e7895b41e8c` absent, Python `~/.local/state/ccb/projects/302a3b148cf77d3ecab65db7becea51f0c9abed4d7f43271afdc1e7895b41e8c` preserved.
- Live agent smoke was not re-run after cleanup in this checkpoint to keep the user-requested clean resource state intact.

## Phase D follow-up — full-suite test contract alignment and ccb-legacy sync boundary (2026-06-26)

Owner finding:

- Current uncommitted `ccbr` diff is test-only: it aligns stale CLI/handler regression tests with the already-implemented Python-compatible trace/readback and resubmit/retry owner behavior.
- `trace <agent>` and dispatcher-local trace fallback are intentionally rejected for the Python `trace` op; concrete `job_id` / mailbox ids remain the owner contract.
- `resubmit` test setup must record a terminal latest attempt without creating reply-delivery side effects, otherwise the test fixture manufactures an active/latest attempt and contradicts the Python lifecycle gate.
- `ccb-legacy` already has the prior provider/ask production sync commit. This test-only delta has no safe equivalent legacy owner: legacy `ccb-cli` still accepts agent-name trace and legacy `ccb-daemon::handlers::resubmit` is a thin dispatcher stub without the new mailbox-bureau handler tests. Forcing the ccbr expectations there would break legacy's current branch behavior rather than synchronize an equivalent fix.

Fix:

- Updated `ccbr-cli` integration/stub tests to target concrete jobs for trace and to expect Python-style rejection of unknown retry/resubmit targets.
- Updated the `ccbr-daemon` resubmit test fixture so the latest attempt is the intended terminal attempt and no synthetic reply-delivery attempt is introduced.
- Left `ccb-legacy` unchanged for this test-only pass after verifying the corresponding owner surfaces are not equivalent; Python `ccb` / `.ccb` state remains untouched.

Evidence:

- Main line:
  - `cd rust && cargo fmt --check -p ccbr-cli -p ccbr-daemon`
  - `cd rust && cargo test -p ccbr-cli -- --test-threads=1`
  - `cd rust && cargo test -p ccbr-daemon -- --test-threads=1`
  - `cd rust && cargo test -p ccbr-providers -- --test-threads=1`
  - `(cd tools/ccb-agent-sidebar && cargo test -- --test-threads=1)`
- Legacy sync check:
  - inspected `ccb-legacy:rust/crates/ccb-cli/tests/cli_integration_tests.rs`, `ccb-legacy:rust/crates/ccb-cli/tests/cli_stub_commands_tests.rs`, and `ccb-legacy:rust/crates/ccb-daemon/src/handlers/resubmit.rs`.
  - no ccb-legacy commit is required for this test-only adjustment because the branch lacks the same Python-trace/project-view/resubmit test owner.
- Resource cleanup after full-suite checks:
  - `CCB_TEST_ROOTS=/mnt/d/dapro-ass bash scripts/ccbr-test-cleanup.sh` reported ccbr test remnants reclaimed and Python `ccb` state untouched.
  - Reaped orphan tmux processes whose command lines were scoped to `/tmp/.tmp*/.ccbr/ccbrd/tmux.sock`.
  - Removed `/tmp/.tmp*/.ccbr` test leftovers; follow-up scan found no matching tmp `.ccbr` dirs and no ccbrd/sidebar/test-tmux processes.

## Owner v1.1 re-alignment pass (2026-06-26)

Owner spec learned:

- Re-read `/mnt/g/owner/software_owner_governance_spec_v1_1.md` and applied its canonical record model.
- Reclassified older mixed owner surfaces into the v1.1 MECE set: `domain_truth`, `policy`, `interface`, `capability`, `projection`, `evidence_admission`, `lifecycle_gate`.
- Produced `research/owner-alignment-v1_1.md` comparing Python CCB 7.5.2 source anchors with Rust CCBR owner anchors.

Finding:

- Python 26 ccbd RPC registrations are covered by Rust; Rust has seven local extensions (`ask`, `cleanup`, `fault_*`, `logs`, `maintenance_tick`).
- Current live interop blocker is not registration or prompt dispatch: real smoke showed `project-view` worked, `ask` returned accepted, and Claude wrote the exact token to its managed JSONL.
- Blocker owner is `provider_claude_session_polling` (`capability`): live smoke proved prompt dispatch and Claude JSONL reply exist, but daemon readback did not terminalize the job or deliver inbox.
- `claude_projects_root` in the simple Claude session payload is a required parity fix, but post-fix smoke showed it is not yet the complete root cause; next diagnosis must inspect reader state / heartbeat ingestion.
- `test_resource_cleanup` is a lifecycle-gate owner: live smoke interruption exposed orphan `/run/user/0/ccbr-runtime/tmux-*` and project-local `.ccbr/runtime` artifacts that cleanup must reclaim without touching Python `.ccb` state.

Next required work:

- Validate the payload parity fix with unit tests and keep the equivalent `ccb-legacy` sync where the same Rust owner surface exists.
- Continue root-cause diagnosis in `provider_claude_session_polling`: active execution `reader_state`, Claude reader offset/session reset, and heartbeat polling/terminalization.
- Cleanup evidence after this pass: `CCB_TEST_ROOTS=/mnt/d/dapro-ass bash scripts/ccbr-test-cleanup.sh` leaves no targeted ccbrd/tmux/provider process, removes ccbr daemon state, and leaves only `.ccbr/ccbr.config` plus `.ccbr/bin` in the test project.

Verification:

- Re-extracted Python/Rust RPC registrations: Python 26, Rust 33, Python-only none, Rust-only `ask`, `cleanup`, `fault_*`, `logs`, `maintenance_tick`.
- `bash -n scripts/ccbr-test-cleanup.sh`
- `git diff --check`
- `cd rust && cargo fmt --check -p ccbr-daemon`
- `cd rust && cargo test -p ccbr-daemon provider_launcher::tests::test_simple_provider_session_payload_includes_tmux_socket_path -- --test-threads=1`
- `/tmp/ccb-legacy-sync`: `cargo test -p ccb-daemon provider_launcher::tests::test_simple_provider_session_payload_includes_tmux_socket_path -- --test-threads=1`

## Claude provider readback closure (2026-06-26)

Root cause:

- Claude writes managed logs under project keys that preserve the leading slash as `-`, e.g. `/mnt/d/dapro-ass` -> `-mnt-d-dapro-ass`.
- Rust `ClaudeLogReader` had a private `project_key_for_path` that trimmed leading/trailing `-`, so it looked under `mnt-d-dapro-ass` and missed the real JSONL.
- Separately, daemon pane capture overwrote provider `reply_buffer` with full tmux TUI text. When pane fallback won the race, inbox contained prompt/TUI chrome instead of the clean Claude reply.

Fix:

- Align `ClaudeLogReader` project key generation with Python/Claude: every non-ASCII-alphanumeric character becomes `-`; no trim.
- Store daemon pane capture as `pane_text_buffer`, not `reply_buffer`.
- Keep pane fallback available only as fallback; Claude refuses pane fallback when `session_path` or `claude_projects_root` indicates structured logs are expected.
- When the completion tracker terminalizes from provider items, also call `execution.finish(job_id)` so active provider execution state is removed.

Evidence:

- `cd rust && cargo fmt --check -p ccbr-daemon -p ccbr-providers`
- `cargo test -p ccbr-providers claude::reader::tests -- --test-threads=1`
- `cargo test -p ccbr-providers providers::claude::tests -- --test-threads=1`
- `cargo test -p ccbr-providers providers::codex::tests::test_poll_waits_for_structured_log_instead_of_pane_fallback -- --test-threads=1`
- `cargo build -p ccbr-cli -p ccbr-daemon -p ccbr-providers`
- Live smoke `/tmp/ccbr-live-smoke.sh` in `/mnt/d/dapro-ass`:
  - `START_CODE=0`, `STATUS_CODE=0`, `PROJECT_VIEW_CODE=0`, `ASK_CODE=0`
  - `INBOX_FOUND=1`
  - inbox reply was clean: `"CCBR_SMOKE_SUBMIT_1782445181"`
- Resource cleanup:
  - `CCB_TEST_ROOTS=/mnt/d/dapro-ass bash scripts/ccbr-test-cleanup.sh`
  - no targeted ccbrd/tmux/provider/sidebar process remained
  - `.ccbr` only retained `ccbr.config` and `bin`
  - ccbr daemon state path for `/mnt/d/dapro-ass` was absent
- `ccb-legacy` equivalent Rust owner sync: commit `0580ae24 Keep legacy provider readback structured`.


## Bloodline boundary correction (2026-06-26)

User clarification:

- Python `ccb` and Rust `ccbr` runtimes must remain isolated.
- `ccbr` parity target is the Python 7.5.2 wire/user contract, validated through Rust `ccbr` clients.
- `ccb-legacy` is the Rust branch that must remain 100% compatible with Python `ccb`; ccbr-only local divergences must not be copied there.
- A forced Python `ask_cli` -> `.ccbr` smoke failed with `project ccbd is unmounted; run ccb first`; this is now classified as correct bloodline isolation, not a ccbr P0 defect.

Next validation target:

- Re-run live smoke with `bin/ask` / `ccbr ask` and `ccbr-agent-sidebar`, then cleanup all `/mnt/d/dapro-ass` test resources.
