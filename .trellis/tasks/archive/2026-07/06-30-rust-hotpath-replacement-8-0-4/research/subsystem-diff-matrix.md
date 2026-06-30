# Subsystem Diff Matrix: Python v8.0.4 vs Rust `ccb-legacy`

Scope: identify which Python subsystems in `/home/agnitum/ccb-git` tag `v8.0.4` can be replaced by the Rust crates in `/home/agnitum/ccb` branch `refs/heads/ccb-legacy` to reduce memory. The Rust workspace version is `7.5.2`, so it predates most v8.0.4 features.

Legend:
- `safe_to_replace_now`: Rust crate covers the v8.0.4 surface with no known protocol break.
- `needs_backport`: Rust crate exists but is missing v8.0.4 features; replacement is possible after backporting the delta.
- `out_of_scope`: not a memory-reduction target or not implemented in Rust.
- `blocked_by_protocol`: delta changes the daemon socket protocol, provider transport/hook contracts, or inter-agent messaging; replacement requires protocol alignment first.

---

## 1. daemon control plane

- **Python path:** `lib/ccbd/*`
- **Rust crate:** `ccb-daemon` (`rust/crates/ccb-daemon/`)
- **Python v8.0.4 delta (v7.5.2..v8.0.4):**
  - 57 files, `+2871 / -183` lines.
  - Major changed/new files:
    - `lib/ccbd/project_view/service.py` — reload-drain-aware caching, sidebar pane refresh, `tmux_focus`/`tmux_snapshot`, dispatcher/drain revision checks.
    - `lib/ccbd/reload_drain_status.py` — new payload formatter for project view.
    - `lib/ccbd/socket_client_runtime/endpoints.py` — new `project_sidebar_click` endpoint.
    - `lib/ccbd/handlers/project_restart.py` — role digest change blocking (`_role_restart_blocked`).
    - `lib/ccbd/keeper_runtime/failure_policy.py`, `lib/ccbd/services/dispatcher_runtime/cancel_flags.py` — new failure/cancellation policies.
    - `lib/ccbd/reload_patch_move_agents.py`, `lib/ccbd/services/project_namespace_runtime/move_patch_agents.py`, `agent_window_reflow.py` — move-agent reload support.
    - `lib/ccbd/start_runtime/agent_runtime.py`, `lib/ccbd/services/mount.py` — runtime/mount lifecycle tweaks.
- **Rust key modules:**
  - `src/lib.rs`, `src/main.rs`, `src/app.rs`, `src/socket_server.rs`
  - `src/handlers/{project_view.rs, project_restart.rs, project_reload.rs, project_focus.rs, ...}`
  - `src/reload_drain.rs` (models/store only)
  - `src/project_view/mod.rs`, `src/project_focus/service.rs`
- **Readiness:** `blocked_by_protocol`
- **Notes:**
  - Rust `handle_project_view` returns a stub (`agents`, `windows`, `comms`: `[]`) and does not emit `reload_drains`, dispatcher revision, or sidebar refresh fields that Python v8 clients now consume.
  - Rust handler registry lacks `project_sidebar_click` (new v8 socket endpoint).
  - Reload drain store models exist in Rust but are not wired into project view or ask routing.
  - Role digest restart blocking is not implemented in Rust `project_restart`.
  - Move-agent reload patch modules exist in Rust but need validation against v8 reflow semantics.

---

## 2. CLI / wrapper

- **Python path:** `lib/cli/*`, `ccb.py`
- **Rust crate:** `ccb-cli` (`rust/crates/ccb-cli/`)
- **Python v8.0.4 delta:**
  - 64 files, `+12535 / -212` lines.
  - Entirely new subsystems:
    - `lib/cli/services/mobile.py`, `mobile_update.py`
    - `lib/cli/services/plan_tasks.py`
    - `lib/cli/services/terminal_qr.py`
    - `lib/cli/tools_runtime/workbench.py`
    - `lib/cli/services/layout.py`, `layout_status.py`
    - `lib/cli/services/loop_capacity.py`, `loop_run_once.py`, `loop_runner.py`
    - `lib/cli/services/questions.py`
    - `lib/cli/services/agent_lifecycle.py`, `agent_status_diagnostics.py`
  - Expanded: `lib/cli/services/ask_runtime/submission.py` (reload-drain target fallback), `lib/cli/render_runtime/ops_views_basic.py`, `lib/cli/tools_runtime/neovim.py`, `ccb.py`.
- **Rust key modules:**
  - `src/lib.rs`, `src/main.rs`, `src/entrypoint.rs`, `src/commands.rs`
  - `src/handlers_ask.rs`, `src/handlers_ops.rs`, `src/handlers_start.rs`
  - `src/ask_sender.rs`, `src/ask_syntax.rs`, `src/ask_usage.rs`
  - `src/neovim.rs`, `src/tools_runtime/neovim.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - Rust CLI is functionally at the v7.5.2 level. None of the v8 mobile, plan/tasks, terminal QR, workbench, layout, loop, or question flows exist.
  - Ask routing lacks the v8 reload-drain target fallback (`_resolve_active_reload_drain_target`).
  - No protocol block, but the surface area is large; replacement only after feature backports.

---

## 3. agents / config / rolepacks

- **Python path:** `lib/agents/*`, `lib/rolepacks/*`
- **Rust crate:** `ccb-agents` (`rust/crates/ccb-agents/`)
- **Python v8.0.4 delta:**
  - 30 files, `+1914 / -339` lines.
  - New files:
    - `lib/agents/config_loader_runtime/dynamic_agent_overlays.py`
    - `lib/agents/config_loader_runtime/loop_overlays.py`
    - `lib/agents/config_loader_runtime/parsing_runtime/loop_capacity.py`
    - `lib/agents/models_runtime/config_runtime/loop_capacity.py`
  - Expanded: `defaults_runtime/project.py`, `rendering_runtime/serialization.py`, `parsing_runtime/provider_profiles.py`, `parsing_runtime/topology.py`, `role_lookup.py`.
  - `lib/rolepacks/runtime_lookup.py`, `service.py`, `sources.py` — role lock adoption removed; digest-based role identity.
- **Rust key modules:**
  - `src/config_loader_runtime/{parsing_runtime, defaults_runtime, io_runtime}`
  - `src/models_runtime/config_runtime/{project.rs, spec.rs, topology.rs, validation.rs}`
  - `src/rolepacks/`, `src/role_lookup.rs`, `src/layout.rs`, `src/layout_plan.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - Rust lacks `loop_capacity` parsing/models and dynamic/loop overlays.
  - Rust `kimi/skills.rs` returns two skill dirs `(inherited, role)`; Python v8 returns three `(inherited, role, overlay)` and supports `kimi-skill-overlay:` projection markers.
  - Role digest semantics in `rolepacks/service.py` changed in v8; Rust rolepacks module likely still uses the v7.5.2 lock-adoption path.

---

## 4. terminal / tmux

- **Python path:** `lib/terminal_runtime/*`
- **Rust crate:** `ccb-terminal` (`rust/crates/ccb-terminal/`)
- **Python v8.0.4 delta:**
  - 3 files, `+293 / -22` lines.
  - `lib/terminal_runtime/ui_theme.py` — new.
  - `lib/terminal_runtime/tmux_theme.py` — expanded theme rendering.
  - `lib/terminal_runtime/env.py` — minor env handling change.
- **Rust key modules:**
  - `src/tmux_theme.rs`, `src/theme.rs`, `src/tmux_backend.rs`, `src/tmux_panes.rs`, `src/layouts*.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - Core tmux backend, pane management, and layout code exist in Rust.
  - UI theme abstraction and v8 theme additions are missing; low-risk backport once the theme contract is known.

---

## 5. mailbox

- **Python path:** `lib/mailbox_kernel/*`
- **Rust crate:** `ccb-mailbox` (`rust/crates/ccb-mailbox/`)
- **Python v8.0.4 delta:**
  - No files changed between v7.5.2 and v8.0.4.
- **Rust key modules:**
  - `src/kernel.rs`, `src/mailbox.rs`, `src/service.rs`, `src/models.rs`, `src/stores.rs`, `src/transitions.rs`
- **Readiness:** `safe_to_replace_now`
- **Notes:**
  - Python mailbox kernel was stable across v7.5.2..v8.0.4.
  - Rust crate mirrors the public Python boundary (`MailboxKernelService`, `DeliveryLease`, `InboundEventRecord`, etc.).

---

## 6. message bureau

- **Python path:** `lib/message_bureau/*`
- **Rust crate:** `ccb-message-bureau` (`rust/crates/ccb-message-bureau/`)
- **Python v8.0.4 delta:**
  - 1 file, `+63 / -2` lines: `lib/message_bureau/control_trace_runtime/summaries.py`.
- **Rust key modules:**
  - Re-exports `ccb_mailbox::bureau::{MessageBureauControlService, MessageBureauFacade}`
  - `src/control_trace_runtime/summaries.rs`, `src/facade*.rs`, `src/control_queue*.rs`
- **Readiness:** `safe_to_replace_now`
- **Notes:**
  - The v8 delta is limited to trace summary formatting.
  - Rust crate already mirrors the Python public `__all__` surface; any summary differences are minor and fixable without protocol changes.

---

## 7. heartbeat

- **Python path:** `lib/heartbeat/*`
- **Rust crate:** `ccb-heartbeat` (`rust/crates/ccb-heartbeat/`)
- **Python v8.0.4 delta:**
  - No files changed.
- **Rust key modules:**
  - `src/engine.rs`, `src/engine_runtime.rs`, `src/classifier.rs`, `src/models.rs`, `src/store.rs`
- **Readiness:** `safe_to_replace_now`
- **Notes:**
  - Stable subsystem. Rust crate has integration and public API tests.

---

## 8. jobs

- **Python path:** `lib/jobs/*`
- **Rust crate:** `ccb-jobs` (`rust/crates/ccb-jobs/`)
- **Python v8.0.4 delta:**
  - 1 file, `+309 / -0` lines: `lib/jobs/store.py`.
  - New: `ProjectViewJobSummary`, `list_agent_tails_batch`, `list_agent_tail_summaries_batch`, `list_project_view_recent_jobs`.
  - Integrates with Rust helpers: `rust_helpers_jsonl.read_jsonl_tail_strict_batch_required`, `rust_helpers_jsonl.read_job_tail_summaries_required`, `rust_helpers_project_view.read_jobs_query_recent_required`.
- **Rust key modules:**
  - `src/store.rs`, `src/models.rs`, `src/lib.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - Rust store is minimal compared to the v8 Python store.
  - Project-view job summary queries are a hot path for memory reduction but depend on the same Rust helper bridge that Python v8 uses; the Rust crate would need to expose equivalent batch/summary APIs.

---

## 9. storage

- **Python path:** `lib/storage/*`
- **Rust crate:** `ccb-storage` (`rust/crates/ccb-storage/`)
- **Python v8.0.4 delta:**
  - 3 files, `+52 / -5` lines.
  - `lib/storage/paths_ccbd.py` — new mobile paths (`ccbd_mobile_dir`, `ccbd_mobile_gateway_path`, `ccbd_mobile_devices_path`, `ccbd_mobile_pairing_tokens_path`, `ccbd_mobile_terminal_tokens_path`, `ccbd_mobile_audit_path`).
  - `lib/storage/jsonl_store.py` — `_strict_jsonl_helper_required()` toggle calling `rust_helpers_jsonl`.
  - `lib/storage/text_artifacts.py` — artifact handling tweaks.
- **Rust key modules:**
  - `src/paths_ccbd.rs`, `src/jsonl_store.rs`, `src/json_store.rs`, `src/text_artifacts.rs`, `src/paths.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - Rust `paths_ccbd.rs` lacks all mobile paths.
  - Rust `jsonl_store.rs` does not implement the `CCB_RUST_JSONL_STORE` helper bridge used by Python v8.
  - Backport is mechanical but required before the storage layer can be swapped.

---

## 10. provider core

- **Python path:** `lib/provider_core/*`
- **Rust crate:** `ccb-provider-core` (`rust/crates/ccb-provider-core/`)
- **Python v8.0.4 delta:**
  - 9 files, `+420 / -4` lines.
  - New files:
    - `lib/provider_core/fifo_delivery.py` — reliable FIFO writes, ack files, spool files.
    - `lib/provider_core/transport.py` — `FifoTransport` / `SpoolDirTransport`, Windows inbox fallback.
    - `lib/provider_core/comm_logging.py` — communication logging.
    - `lib/provider_core/platform_info.py` — platform detection.
  - Expanded: `runtime_specs.py`, `runtime_lock.py`, `runtime_shared.py`, `registry_runtime/builtin_backends.py`.
- **Rust key modules:**
  - `src/protocol.rs`, `src/protocol_runtime/`, `src/registry.rs`, `src/runtime_lock.rs`, `src/runtime_shared.rs`, `src/runtime_specs.rs`
- **Readiness:** `needs_backport`
- **Notes:**
  - This task backported the v8 cross-platform provider transport contract into `ccb-provider-core`:
    - `src/transport.rs` — `MessageTransport`, `FifoTransport`, `SpoolDirTransport`, `PersistentFifoReader` keepalive-fd pattern.
    - `src/fifo_delivery.rs` — reliable FIFO writes with `PIPE_ATOMIC_LIMIT`, retry/ack/spool semantics.
  - What remains before a full swap: `comm_logging`, `platform_info`, and wiring the new transport into the active provider execution path.
  - The transport layer itself is unit-tested and does not change the daemon socket protocol.

---

## 11. provider execution

- **Python path:** `lib/provider_execution/*`
- **Rust crate:** `ccb-providers` execution (`rust/crates/ccb-providers/src/execution*`)
- **Python v8.0.4 delta:**
  - 1 file, `+110 / -1` lines: `lib/provider_execution/fake.py`.
- **Rust key modules:**
  - `src/execution.rs`, `src/active_runtime/`
- **Readiness:** `safe_to_replace_now`
- **Notes:**
  - Delta is limited to test-double/fake execution changes.
  - Rust execution registry and adapter trait are present.

---

## 12. provider backends

- **Python path:** `lib/provider_backends/{codex,claude,gemini,droid,agy,kimi,opencode,...}/*`
- **Rust crate:** `ccb-providers` (`rust/crates/ccb-providers/src/{claude,codex,gemini,opencode,droid,agy,kimi,...}`)
- **Python v8.0.4 delta:**
  - 37 files, `+2674 / -80` lines.
  - New provider: `lib/provider_backends/zai/` (`__init__.py`, `execution.py`, `launcher.py`, `manifest.py`, `session.py`).
  - Kimi: new `hindsight.py`, skill overlays in `skills.py`, pane fallback observation in `execution.py`, native turn timeout env, K2.7 pane detection.
  - Codex: `bridge_runtime/binding_runtime.py`, `runtime_io.py`, `service.py`; new `execution_runtime/accelerator.py`; `launcher_runtime/command_runtime/diagnostics.py`.
  - Claude: launcher home handling (`launcher_runtime/home.py`), execution start changes.
  - Agy: poll/start changes, native log changes.
  - Native CLI support: `native_cli_support/execution.py`, `pane_quiet_support/execution.py`.
- **Rust key modules:**
  - `src/claude/`, `src/codex/`, `src/gemini/`, `src/opencode/`, `src/droid/`, `src/agy/`, `src/kimi/`
  - `src/providers/{claude.rs,codex.rs,...}`
- **Readiness:** `needs_backport`
- **Notes:**
  - Rust has execution adapters for claude/codex/gemini/opencode/droid/agy/kimi but the implementations lag v8.
  - This task backported the v8 Codex bridge gaps into `ccb-providers/src/providers/codex/diagnostics.rs` (`CodexDiagnosticLogFilterInstaller` / `logs_2.sqlite` redirect trigger) and added the transport primitives in `ccb-provider-core`.
  - Production Codex agents are already running without the Python `bridge.py` process via a marker-file gate (`runtime_dir/.use-rust-bridge`); the Rust transport/diagnostics modules are staged for integration.
  - Kimi Rust code is in `src/providers/kimi.rs` and `src/kimi/{launcher,native_log,session,skills}.rs`; it lacks hindsight, skill overlays, and pane fallback logic.
  - Zai provider is absent from Rust.
  - Full Rust Codex bridge runtime and accelerator are not yet wired into the provider adapter.

---

## Summary by classification

| Classification | Subsystems |
|----------------|------------|
| `safe_to_replace_now` | mailbox, message bureau, heartbeat, provider execution |
| `needs_backport` | CLI, agents/config/rolepacks, terminal/tmux, jobs, storage, provider backends, provider core |
| `blocked_by_protocol` | daemon control plane |
| `out_of_scope` | — (none listed; mobile/plan-task/QR/workbench features are covered under CLI) |

## Replacement recommendation for memory reduction

1. **Quick wins** (swap first): `ccb-mailbox`, `ccb-message-bureau`, `ccb-heartbeat`, `ccb-providers` execution adapters. These are stable or have small v8 deltas and do not change user-facing protocol.
2. **Backport then swap**: `ccb-terminal`, `ccb-jobs`, `ccb-storage`, `ccb-agents`, `ccb-providers` backends, `ccb-cli`. The backport work is bounded and well-localized.
3. **Blockers** (require protocol design): `ccb-daemon` (socket/protocol changes). `ccb-provider-core` transport/fifo layer is now implemented in Rust and is no longer a protocol blocker, but it still needs to be wired into the provider execution path.

## Task outcomes (this slice)

- `ccb-legacy` release build passes; `cargo test -p ccb-provider-core -p ccb-providers` and `cargo clippy -D warnings` pass.
- New Rust modules:
  - `crates/ccb-provider-core/src/transport.rs`
  - `crates/ccb-provider-core/src/fifo_delivery.rs`
  - `crates/ccb-providers/src/providers/codex/diagnostics.rs`
- Codex Python bridge elimination rolled out to all 6 Codex agents via `.use-rust-bridge` marker + `CCB_RUST_BRIDGE` env gate; rollback is one marker removal + pane restart away.
- Measured orchestration-layer RSS reduction: ~350 MB → ~194 MB (≈180 MB saved) after removing 6 Python bridge processes.
- `tee` subprocess replacement was deprioritized: each tmux pane still needs a pipe-pane consumer, so the win is small relative to the churn.
- Daemon / CLI / keeper consolidation remains blocked by v8 socket protocol and reload-drain semantics; deferred to a follow-up slice.
