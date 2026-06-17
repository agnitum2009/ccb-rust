# ccb-daemon P0 / P1 stub priorities

Generated from `docs/superpowers/plans/2026-06-16-ccb-daemon-stub-classification.md`.

## P0 — end-to-end `ccbr start → ask → multi-window UI`

These stubs are on the critical path for starting a project, submitting an ask, and having multiple agent panes/windows mounted.

| # | Rust stub | Python file | Complexity | Why critical |
|---|---|---|---|---|
| 1 | `services/project_namespace_runtime/backend.rs` | `services/project_namespace_runtime/backend.py` | Large | Tmux backend abstraction used by every namespace ensure/materialize operation. |
| 2 | `services/project_namespace_runtime/records.rs` | `services/project_namespace_runtime/records.py` | Small | Layout signature normalization; blocks deterministic namespace comparison. |
| 3 | `services/project_namespace_runtime/ensure_context.rs` | `services/project_namespace_runtime/ensure_context.py` | Medium | State container for namespace ensure/recreate decisions. |
| 4 | `services/project_namespace_runtime/ensure.rs` | `services/project_namespace_runtime/ensure.py` | Large | Core `ensure_project_namespace` / recreate / reflow logic. |
| 5 | `services/project_namespace_runtime/materialize_topology.rs` | `services/project_namespace_runtime/materialize_topology.py` | Large | Translates topology plan into tmux windows/panes. |
| 6 | `supervision/mount_runtime/service.rs` | `supervision/mount_runtime/service.py` | Large | Core mount orchestrator (`ensure_mounted`, `stabilize_superseded_runtime`). |
| 7 | `supervision/mount_runtime/transitions.rs` | `supervision/mount_runtime/transitions.py` | Medium | Mount/reflow transitions and persistence. |
| 8 | `supervision/mount_runtime/starting.rs` | `supervision/mount_runtime/starting.py` | Small | Builds `StartingRuntime` for mount attempts. |
| 9 | `start_flow_runtime/service_tmux.rs` | `start_flow_runtime/service_tmux.py` | Medium | Tmux namespace helpers used during start flow. |
| 10 | `start_flow_runtime/service_context.rs` | `start_flow_runtime/service_context.py` | Small | Start-context / action-record builders. |
| 11 | `services/dispatcher_runtime/submission_service.rs` | `services/dispatcher_runtime/submission_service.py` | Medium | Plans agent/message submission. |
| 12 | `services/dispatcher_runtime/reply_delivery_runtime/preparation_service.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_service.py` | Medium | Orchestrates reply-delivery preparation. |
| 13 | `services/dispatcher_runtime/reply_delivery_runtime/preparation_message.rs` | `services/dispatcher_runtime/reply_delivery_runtime/preparation_message.py` | Medium | Builds reply-delivery jobs/messages. |
| 14 | `services/dispatcher_runtime/reply_delivery_runtime/claims.rs` | `services/dispatcher_runtime/reply_delivery_runtime/claims.py` | Small | Claims reply-delivery job IDs. |

## P1 — `ccbr reload` hot-swap

These stubs implement the additive reload transaction pipeline.

| # | Rust stub | Python file | Complexity | Why critical |
|---|---|---|---|---|
| 1 | `reload_transaction_service.rs` | `reload_transaction_service.py` | Medium | `publish_additive_reload_transaction` orchestrator. |
| 2 | `reload_transaction_publish.rs` | `reload_transaction_publish.py` | Small | `publish_or_rollback` logic. |
| 3 | `reload_transaction_results.rs` | `reload_transaction_results.py` | Small | Result constructors for blocked/failed/published. |
| 4 | `reload_apply_service.rs` | `reload_apply_service.py` | Large | `run_additive_reload_apply` main apply orchestrator. |
| 5 | `reload_apply_namespace.rs` | `reload_apply_namespace.py` | Medium | Applies namespace patch during reload. |
| 6 | `reload_drain.rs` | `reload_drain.py` | Large | Drain queue / intent / transitions. |
| 7 | `reload_handoff.rs` | `reload_handoff.py` | Medium | Reload handoff store and validation. |
| 8 | `reload_runtime_mount_service.rs` | `reload_runtime_mount_service.py` | Medium | Runs additive agent mounts. |
| 9 | `reload_runtime_mount_start.rs` | `reload_runtime_mount_start.py` | Small | Adapter to call start flow for additive mounts. |
| 10 | `reload_patch_additive_agents.rs` | `reload_patch_additive_agents.py` | Small | Computes additive agent steps. |

## Not in scope for this phase

- Keeper-runtime stubs (`keeper_runtime/*`, `app_state.rs`, `stores.rs`) — C-class, no keeper process in Rust daemon.
- Pure model/re-export stubs that are already covered by `models/` or `ccb-mailbox` — A-class.
- Deep reply-delivery formatting/retry/finalization refinements — needed for full feature parity but can follow the core P0 path above.
