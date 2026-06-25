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

- `project_clear_context`: verify provider-specific clear semantics and structured response.
- `project_reload_config`: verify additive/reload shape vs Python.
- `project_focus_*`: verify tmux targeting and response shape.
- `watch/get/trace/queue`: verify Python response envelopes and empty/error cases.

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
