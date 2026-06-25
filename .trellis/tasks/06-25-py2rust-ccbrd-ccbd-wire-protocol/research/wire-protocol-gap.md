# Wire Protocol / Owner Gap Matrix

Date: 2026-06-26

## Method

Owner classification follows `/mnt/g/owner` responsibility-chain rules:

- classify the surface first;
- separate provider owner from consumer owner;
- use Python `ccb` 7.5.2 as reference capability owner;
- use Rust `ccbrd` as current implementation owner;
- treat Trellis/CodeGraph/reference code as evidence, not owner truth.

## Registered RPC coverage

Python registered ops: 26.

Rust registered ops: 33.

Python-only ops: none.

Rust-only ops: `ask`, `cleanup`, `fault_arm`, `fault_clear`, `fault_list`, `logs`, `maintenance_tick`.

Conclusion: handler registration is covered; remaining gaps are behavior/shape parity.

## Gap matrix

| Surface | Python owner anchor | Rust owner anchor | Status | Priority | Required action |
|---|---|---|---|---|---|
| `submit` ask delivery | `backup/python-reference/lib/ccbd/handlers/submit.py`, `backup/python-reference/lib/provider_backends/codex/execution_runtime/start.py` | `rust/crates/ccbr-daemon/src/handlers/submit.rs`, `rust/crates/ccbr-daemon/src/app.rs`, `rust/crates/ccbr-providers/src/providers/codex.rs` | closed | P0 | Codex prompt dispatch is provider-owned during execution start; daemon heartbeat regression proves Python-style `submit` drives wrapped prompt delivery. |
| Rust local `ask` op | none; Rust extension | `handlers/ask.rs` | guarded_extension | P0-supporting | Keep as CLI convenience; skip manual pane send when provider-owned start already sent the prompt. |
| `project_view` sidebar readback | Python `handlers/project_view.py`, `project_view/**` | Rust `handlers/project_view.rs`, `tools/ccb-agent-sidebar/src/model.rs` | closed | P0 | Rust now emits the sidebar-consumed Python `{view, cache}` shape including project/ccbd/namespace/sidebar/window/agent/comms fields. |
| `inbox` / `mailbox_head` / `ack` | Python mailbox handlers | Rust mailbox handlers + `ccbr-mailbox` | closed | P0 | Rust uses mailbox-control state for readback and accepts Python `ack.inbound_event_id` while preserving Rust `event_id` alias. |
| `project_restart_panes` | Python schedules in-place restart callback | Rust `handlers/project_restart.rs` + `run_start_flow` | closed_with_divergence | P0 | Rust now performs synchronous topology recreation via `run_start_flow`; no no-op `scheduled` success remains. |
| `project_restart_agent` | Python restarts one agent with busy gate | Rust restarts all agents to preserve layout | accepted_divergence_candidate | P1 | Document/verify layout reason; ensure response shape does not mislead Python clients. |
| `shutdown` red X | Python graceful daemon stop | Rust full workspace exit | accepted_local_divergence | P0 closed locally | User confirmed red X means complete workspace exit; Rust must kill tmux session/provider processes. |
| `stop-all` | Python prepare/finalize project stop | Rust direct stop flow | closed_with_local_divergence | P1 | Rust executes stop flow directly with caller-provided `force`; user-facing `shutdown` owns full workspace exit with forced cleanup. |
| `project_clear_context` | Python provider clear implementation | Rust handler | closed | P1 | Rust now sends `/clear` to real namespace/registry panes and returns Python-compatible target/result rows. |
| `project_reload_config` | Python reload transaction | Rust reload handler | closed | P1 | Rust now returns Python dry-run/apply shape, publishes successful config into runtime read models, and blocks busy removed agents. |
| `project_focus_window/agent` | Python tmux focus service | Rust focus handlers | closed | P1 | Rust now validates namespace epoch, selects tmux window/pane, returns Python-style `focused` response, and refreshes sidebars best-effort. |
| `get` / `watch` | Python dispatcher handlers | Rust dispatcher handlers | closed | P2 | Rust now returns Python-visible get payloads, fails unknown jobs, consumes `watch.cursor`, and rejects negative cursors. |
| `queue` / `trace` / `cancel` | Python dispatcher + mailbox-control handlers | Rust dispatcher + mailbox-control handlers | closed | P2 | Queue/cancel already use mailbox-control/mailbox terminal state; trace now rejects Python-invalid `all`/agent targets and uses mailbox-control trace only. |
| `resubmit` / `retry` | Python message-bureau lifecycle handlers | Rust dispatcher handlers | closed | P2 | Rust now validates mailbox message/attempt lineage, enqueues retry/resubmit jobs, records new message/attempt records, and returns Python lifecycle payloads. |
| Provider Codex session/polling | Python provider reference | Rust `ccbr-providers` | intentionally_diverged | P0 policy | Keep hooks enabled, named session files, structured JSONL authoritative, active-only polling. |

## Non-claims

- CodeGraph accelerates lookup; it does not establish owner truth.
- Trellis tracks planning and acceptance; it does not establish daemon protocol truth.
- Python reference code establishes client-facing shape; it does not force Rust to copy slow polling/bridge internals.
- Codex hooks are always enabled; hook suppression is not an allowed compatibility strategy.

## Closure notes

### `submit` ask delivery — closed 2026-06-26

Owner finding:

- Python `submit` enqueues jobs, and provider execution owns prompt dispatch when the job starts.
- Rust had drifted: `ask` sent directly from the daemon handler while `submit` relied on heartbeat start without provider-owned Codex dispatch.

Fix:

- `ccbr-providers` Codex adapter now sends the wrapped prompt from `start_active_submission` when a tmux-backed Codex session is present.
- Codex launch session payload now carries `tmux_socket_path`, so provider-owned dispatch targets the workspace tmux socket instead of default tmux.
- Daemon heartbeat no longer overwrites provider-owned `prompt_text` with raw request body.
- Rust-only `ask` treats provider `prompt_sent=true` as delivered and avoids duplicate send.

Evidence:

- `cargo test -p ccbr-daemon submit -- --test-threads=1`
- `cargo test -p ccbr-daemon ask -- --test-threads=1`
- `cargo test -p ccbr-daemon app::tests --lib -- --test-threads=1`
- `cargo test -p ccbr-daemon -- --test-threads=1`
- `cargo test -p ccbr-providers codex -- --test-threads=1`

### `project_restart_panes` — closed with divergence 2026-06-26

Owner finding:

- Python returns `status=scheduled` only because it also returns an after-response callback that performs in-place pane respawn.
- Rust had returned the same `scheduled` shape without any after-response work; that was a misleading no-op success.

Fix:

- Rust `project_restart_panes` now reuses `run_start_flow` for all configured agents.
- Response reports `status`/`restart_status` as `ok` or `failed`, includes `agent_results`, and uses `restart_mode=recreate_topology` instead of claiming in-place restart.
- Empty agent topology returns structured failure with `reason=no_agents_configured`.

Evidence:

- `cargo test -p ccbr-daemon project_restart -- --test-threads=1`

### `project_view` sidebar readback — closed 2026-06-26

Owner finding:

- Python `ProjectViewService.build_response()` owns the daemon wire shape: top-level `{view, cache}`, with `view.project`, `view.ccbd`, `view.namespace.sidebar.view`, `view.windows`, `view.agents`, and `view.comms`.
- Rust had already returned the outer `{view, cache}` shell and `agent.window`, but several sidebar-consumed keys were absent or Rust-only named (`from_actor`/`to_agent` instead of `sender`/`target`).

Fix:

- Rust `handlers/project_view.rs` now keeps the single aggregate handler and fills the Python/sidebar contract directly.
- Window rows include `label`, `kind`, `order`, `active`, tmux ids, sidebar pane placeholder, and tool-window rows.
- Agent rows include job-derived `activity_*`, `current_job_id`, `queue_depth`, runtime fields, and stable window grouping.
- Comms rows include `short_id`, `sender`, `target`, `business_status`, `status_label`, `reply_*`, `recoverable`, `recover_target`, and `block_reason` while preserving legacy aliases.
- The sidebar red-X test was updated to the user-confirmed local divergence: red X invokes `ccb shutdown`, not old `ccb kill`.

Evidence:

- `cargo test -p ccbr-daemon test_project_view_matches_sidebar_wire_shape -- --test-threads=1`
- `cargo test -p ccbr-daemon -- --test-threads=1`
- `(cd tools/ccb-agent-sidebar && cargo test -- --test-threads=1)`

### `project_focus_window/agent` — closed 2026-06-26

Owner finding:

- Python focus handlers are not acknowledgements; they call `ProjectFocusService`, validate the ProjectView namespace epoch, target tmux, and return a `focused` response.
- Rust handlers were no-ops that returned `status=ok` without changing the active tmux window or pane.

Fix:

- Rust focus handlers now build a namespace-backed focus plan.
- `project_focus_window` selects `session:window` and, for agent windows, selects the first configured agent pane when present.
- `project_focus_agent` resolves the agent's configured window and pane, rejects missing panes, and selects both window and pane.
- Both handlers reject stale namespace epochs with `stale_view`, matching the sidebar retry path.
- Sidebar panes are refreshed best-effort with `C-l` after focus succeeds.

Evidence:

- `cargo test -p ccbr-daemon project_focus -- --test-threads=1`
- `cargo test -p ccbr-daemon -- --test-threads=1`

### `project_clear_context` — closed 2026-06-26

Owner finding:

- Python `project_clear_context` is a runtime-integration command, not a readback-only acknowledgement: it resolves requested agents, opens the project tmux namespace, verifies panes, and sends a provider clear sequence.
- Rust returned `status=ok` with empty `results`, so Python/sidebar clients would see a misleading successful no-op.

Fix:

- Rust now normalizes empty/`all`/deduped agent targets and rejects unknown or mixed `all` requests.
- Rust resolves panes from the mounted project namespace first and falls back to the agent registry.
- Rust sends `C-u`, literal `/clear`, and `Enter` to each live pane, preserving the Python OpenCode 300ms delayed submit.
- Per-agent result rows report `cleared`, `skipped`, or `failed` with Python-compatible reason fields.

Evidence:

- `cargo test -p ccbr-daemon project_clear -- --test-threads=1`

### `project_reload_config` — closed 2026-06-26

Owner finding:

- Python `project_reload_config` dry-run returns a plan, while non-dry-run returns a flattened apply payload with `published/blocked/noop/failed` status, mutation flags, operations, and diagnostics-derived errors.
- Rust had two reload paths: a Python-aligned additive apply service existed, but the RPC handler still used a Rust-local lightweight transaction returning `{status:"ok", applied:true}` and did not keep registry/dispatcher read models aligned with the published config.
- Rust also had an empty pre-namespace unload blocker, so removing a busy agent could publish when Python would block.

Fix:

- Non-dry-run handler now uses the additive reload apply service and wraps the result with Python `project_reload_payload.py`-style fields.
- Published reloads call the service-graph publish path so `current_config`, registry, and dispatcher agent lists reflect the new config.
- Removed-agent reloads block when the target has outstanding dispatcher work or a busy/running/active runtime state.
- Invalid non-dry-run configs return `dry_run=false`, `mutation_enabled=false`, `safe_to_apply=false`, and `diagnostics.reason=invalid_config`.

Evidence:

- `cargo test -p ccbr-daemon --test reload_tests -- --test-threads=1`
- `cargo test -p ccbr-daemon project_reload -- --test-threads=1`

### `get` / `watch` — closed 2026-06-26

Owner finding:

- Python `get` returns a rich job readback payload and raises `job not found`; Rust returned a reduced summary and `status=unknown`.
- Python `watch` reads `cursor`; Rust read only `start_line`, so Python clients would always restart from line 0. Python also rejects negative cursors.

Fix:

- Rust `get` now emits Python-visible fields including `job`, `snapshot`, visible reply fields, completion reason/confidence, target/provider fields, and message id placeholder.
- Rust `get` now errors on unknown jobs/agents.
- Rust `watch` accepts `cursor`, keeps `start_line` as an alias, and rejects negative cursor values.

Evidence:

- `cargo test -p ccbr-daemon handlers::get::tests -- --test-threads=1`
- `cargo test -p ccbr-daemon handlers::watch::tests -- --test-threads=1`
- `cargo test -p ccbr-daemon test_watch_returns_activity_lines_for_target -- --test-threads=1`

### `resubmit` / `retry` — closed 2026-06-26

Owner finding:

- Python `resubmit` and `retry` are message-bureau lifecycle operations, not dispatcher-only acknowledgement stubs.
- Rust handlers accepted the RPCs but returned thin `resubmitted` / `retried` / `noop` payloads that did not validate message/attempt lineage and did not create Python-visible bureau records.

Fix:

- Rust `resubmit` now resolves the original message, requires terminal latest attempts for every target agent, enqueues fresh dispatcher jobs, records a new bureau message with `origin_message_id`, and returns `accepted_at`, `original_message_id`, new `message_id`, `submission_id`, and accepted job receipts.
- Rust `retry` now resolves target by `attempt_id` then `job_id`, rejects active/completed/non-latest attempts, enqueues one retry job, records a retry attempt, supports Python's `continue` body when terminal progress was already observed, and returns Python lifecycle fields.

Evidence:

- `cargo test -p ccbr-daemon handlers::resubmit::tests -- --test-threads=1`
- `cargo test -p ccbr-daemon handlers::retry::tests -- --test-threads=1`

### `queue` / `trace` / `cancel` — closed 2026-06-26

Owner finding:

- Python `trace` is a message-bureau control surface and accepts only concrete `sub_`, `msg_`, `att_`, `rep_`, or `job_` identifiers.
- Rust `trace` used mailbox-control for concrete ids, but returned a Rust-local dispatcher job-list fallback for `all` and agent-name targets, which is not Python wire parity.
- Rust `queue` and `cancel` were already wired through mailbox-control / mailbox terminal recording and covered by integration tests.

Fix:

- Added non-panicking mailbox `try_trace`.
- Rust `trace` handler now calls mailbox-control trace only and returns Python-style errors for invalid or missing concrete ids.
- Kept queue/cancel behavior unchanged, with integration tests as closure evidence.

Evidence:

- `cargo test -p ccbr-daemon handlers::trace::tests -- --test-threads=1`
- `cargo test -p ccbr-daemon test_queue_returns_actual_per_agent_state -- --test-threads=1`
- `cargo test -p ccbr-daemon test_trace_returns_job_history -- --test-threads=1`
- `cargo test -p ccbr-daemon test_cancel_updates_mailbox_state -- --test-threads=1`
- `cargo test -p ccbr-mailbox trace -- --test-threads=1`

### `inbox` / `mailbox_head` / `ack` — closed 2026-06-26

Owner finding:

- Python `inbox` and `mailbox_head` are mailbox-control readbacks keyed by `agent_name`, and `inbox.detail` is optional.
- Python `ack` reads `inbound_event_id`; Rust socket-client payloads also emit `inbound_event_id`.
- Rust handler read only legacy `event_id`, so Python clients could accidentally acknowledge the current head while their requested `inbound_event_id` was ignored.

Fix:

- Kept `inbox` and `mailbox_head` on mailbox-control state.
- `ack` now reads Python `inbound_event_id` first and preserves Rust `event_id` as a compatibility alias.
- The integration regression acknowledges through `inbound_event_id` and verifies the task reply leaves the inbox.

Evidence:

- `cargo test -p ccbr-daemon test_ack_acknowledges_reply_event -- --test-threads=1`

### `stop-all` / `shutdown` — closed with local divergence 2026-06-26

Owner finding:

- Python `stop_all` prepares a stop summary and finalizes after response; Rust owns a direct stop-flow service.
- Python `shutdown` is graceful, but the user confirmed the CCBR sidebar red X means complete workspace exit.
- Rust local owner therefore requires user-facing `shutdown` to force cleanup; `stop-all` remains the explicit force-parameter path.

Fix:

- `stop-all` continues to pass the request `force` flag into `app.stop_all(force, "stop_all")` and returns stop-flow fields.
- `shutdown` requests daemon shutdown; the daemon main loop calls `CcbdApp::shutdown()`, which persists running jobs, runs `stop_all(true, "shutdown")`, writes a shutdown report, records `forced_cleanup`, and releases the namespace.
- This preserves Codex hooks: shutdown terminates the managed runtime processes and never disables hooks.

Evidence:

- `cargo test -p ccbr-daemon test_start_stop_flow -- --test-threads=1`
- `cargo test -p ccbr-daemon test_shutdown_handler_requests_shutdown -- --test-threads=1`
- `cargo test -p ccbr-daemon app::tests::test_shutdown_forces_workspace_exit_cleanup --lib -- --test-threads=1`
