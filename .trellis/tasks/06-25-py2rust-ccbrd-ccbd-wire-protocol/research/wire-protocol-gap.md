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
| `inbox` / `mailbox_head` / `ack` | Python mailbox handlers | Rust mailbox handlers + `ccbr-mailbox` | partially_verified | P0 | Preserve final reply and ack semantics under Python client flow. |
| `project_restart_panes` | Python schedules in-place restart callback | Rust `handlers/project_restart.rs` + `run_start_flow` | closed_with_divergence | P0 | Rust now performs synchronous topology recreation via `run_start_flow`; no no-op `scheduled` success remains. |
| `project_restart_agent` | Python restarts one agent with busy gate | Rust restarts all agents to preserve layout | accepted_divergence_candidate | P1 | Document/verify layout reason; ensure response shape does not mislead Python clients. |
| `shutdown` red X | Python graceful daemon stop | Rust full workspace exit | accepted_local_divergence | P0 closed locally | User confirmed red X means complete workspace exit; Rust must kill tmux session/provider processes. |
| `stop-all` | Python prepare/finalize project stop | Rust direct stop flow | partially_verified | P1 | Verify force=false/true response and lifecycle effects. |
| `project_clear_context` | Python provider clear implementation | Rust handler | closed | P1 | Rust now sends `/clear` to real namespace/registry panes and returns Python-compatible target/result rows. |
| `project_reload_config` | Python reload transaction | Rust reload handler | unknown | P1 | Compare additive/reload/blocked cases. |
| `project_focus_window/agent` | Python tmux focus service | Rust focus handlers | closed | P1 | Rust now validates namespace epoch, selects tmux window/pane, returns Python-style `focused` response, and refreshes sidebars best-effort. |
| `watch/get/queue/trace/resubmit/retry/cancel` | Python dispatcher handlers | Rust dispatcher handlers | unknown | P2 | Matrix response envelopes and edge cases. |
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
