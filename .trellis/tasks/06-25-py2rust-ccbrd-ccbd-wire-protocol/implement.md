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
- `queue/trace/cancel`: existing Rust handlers are wired to mailbox/dispatcher
  read models and covered by integration tests; keep under final package
  verification.
- `resubmit/retry`: still P2 residual; Rust thin/stub payloads do not yet
  match Python message-bureau lifecycle payloads.

## Phase D — Validation

- `cargo test -p ccbr-daemon -- --test-threads=1`
- `cargo test -p ccbr-providers -- --test-threads=1` when provider/session code changes.
- Live smoke in `/mnt/d/dapro-ass`:
  - start workspace
  - sidebar displays real state
  - Python/Rust ask path completes A -> B -> A inbox
  - red X/full shutdown leaves no managed tmux/provider remnants

## Guardrails

- Do not disable any Codex hook.
- Do not import Python per-agent bridge/tight polling.
- Do not touch unrelated dirty files.
- Do not claim full owner parity until every row in `wire-protocol-gap.md` is marked `closed`, `accepted_divergence`, `blocked`, or `none_with_reason`.
