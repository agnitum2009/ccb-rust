# Daemon lifecycle parity — design

## Architecture

- **Provider launch layer** (`ccb-providers`): each provider owns `build_runtime_launcher`, `build_start_cmd`, `prepare_launch_context`, `build_session_payload`, `post_launch`.
  - Provider-specific home/session preparation stays in provider submodules (e.g. `codex::launcher_runtime`, `claude::launcher_runtime`).
  - Shared utilities (`provider_start_parts`, `apply_provider_command_template`, `caller_context_env`, `provider_user_session_env`, `pane_title_marker`) live in `ccb-provider-core`.
- **Orchestration layer** (`ccb-daemon`): `start_runtime::agent_runtime_binding` resolves binding state (dead/stale/foreign/alive) and decides whether to reuse, relaunch, or create a new pane. `ProviderLauncher` sends the final command to a tmux pane and persists session payload.
- **CLI service layer** (`ccb-cli`): `ps_service`, `wait_service`, `start_service` call daemon socket/client abstractions already present in `ccb-cli/src/services`.

## Data flow

1. `ensure_agent_runtime`-style entry (in tests or daemon start flow) receives `(spec, runtime_dir, binding, ...)`.
2. It calls `resolve_runtime_binding_state` which:
   - checks `binding_runtime_alive` / `binding_requires_replacement`;
   - decides `agent_action` (`reuse`, `relaunch`, `create`, `degraded`);
   - for `create`/`relaunch`, computes `pane_id` (via tmux backend) and invokes provider launcher.
3. Provider launcher produces `command` + `session_payload` + `session_path`.
4. Command is sent to pane via `TmuxBackend::send_keys` or equivalent; session payload written to `session_path`.
5. Resulting `BindingState` / `StartAgentExecution` returned to caller.

## Compatibility

- Keep `ProviderRuntimeLauncher` struct simple (provider + launch_mode) to avoid changing the backend registry ABI; dispatch logic stays in `ccb-daemon::provider_launcher::build_launch_plan`.
- Reuse existing `ccb-agents::policy::should_restore_provider_history` for resume logic.
- Reuse existing `ccb-provider-hooks` and `ccb-provider-profiles` for profile/env resolution.

## Risk & rollback

- `ccb-daemon/src/start_runtime/*` are currently stubs; filling them in is additive but touches orchestration critical path. Use trait injection for tmux/backend calls in tests.
- Provider launcher changes are localized to per-provider modules; if a provider breaks, revert its module only.
- Avoid changing `ccb-provider-core::contracts::ProviderRuntimeLauncher` shape to prevent registry churn.
