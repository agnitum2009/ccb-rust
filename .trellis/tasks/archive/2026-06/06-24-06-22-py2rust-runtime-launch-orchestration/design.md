# W2: runtime launch orchestration — design

## Architecture

```
CLI / test entry
       │
       ▼
ensure_agent_runtime(context, command, spec, plan, binding, ...)
       │
       ├── binding reusable? ──Yes──► return launched=false
       │
       No
       │
       ├── resolve_provider_launcher(spec.provider)
       ├── prepare_provider_workspace(...)
       ├── create_or_respawn_pane(backend, assigned_pane_id, start_cmd, run_cwd)
       │       ├── assigned_pane_id: respawn-pane
       │       └── otherwise: detached fallback (new tmux session/pane)
       ├── apply_ccb_pane_identity(backend, pane_id, ...)
       ├── build_session_payload + write_session_file
       └── resolve_refreshed_binding(...) ──► return launched=true
```

## Components

### 1. `ccb-daemon/src/start_runtime/agent_runtime.rs`

Extend `start_agent_runtime` / add a standalone `ensure_agent_runtime` that performs the actual launch. It receives the same trait injectors as `resolve_runtime_binding_state` plus:

- `TmuxBackendFactory`: `Fn(socket_path) -> Box<dyn TmuxBackend>`
- `ProviderLauncherResolver`: `Fn(&str) -> Option<ProviderRuntimeLauncher>`
- `WorkspacePreparationFn`: 准备 provider workspace（目录 + settings overlay）
- `SessionFileWriterFn`: 写入 session payload
- `BindingResolverFn`: 从 runtime dir 解析刷新后的 `RuntimeBinding`

### 2. `ccb-daemon/src/provider_launcher.rs`

`build_launch_plan` 新增分支：

- `codex` → `ccb_providers::codex::{prepare_launch_context, build_start_cmd, build_session_payload}`
- `claude` → `ccb_providers::claude::{...}`
- `gemini` → `ccb_providers::providers::gemini::{...}`
- `agy` → `ccb_providers::providers::agy::{...}`
- `droid` → `ccb_providers::droid::{...}`

每个分支遵循现有 opencode/kimi/mimo 模式：

1. `runtime_dir = .ccb/runtime/<agent>/<provider>`
2. `prepare_launch_context(project_root, agent_name, workspace, events_path, runtime_dir)`
3. `build_start_cmd(restore, startup_args, ...)`
4. `build_session_payload(...)`
5. 写入 session 文件

### 3. `ccb-daemon/src/start_runtime/agent_runtime_binding.rs`

已实现的 `resolve_runtime_binding_state` 继续负责 binding 决策。`ensure_agent_runtime` 在其之后调用，或在内部组合使用。

### 4. 测试策略

- 使用 `tempfile::TempDir` 作为 `project_root`。
- 实现一个 `FakeTmuxBackend`：记录 `respawn_pane` / `create_pane` / `set_pane_title` / `set_pane_user_option` / `kill_pane` 调用，返回固定 pane id。
- 通过依赖注入替换真实 `TmuxBackend`，避免真实 tmux 依赖。
- 断言：session 文件存在、payload 包含预期字段、pane identity 被调用、返回 binding 的 `runtime_ref` 为 `tmux:<pane_id>`。

## Compatibility & risk

- `ProviderRuntimeLauncher` 形状保持不变；新分支通过 `ccb_providers` 公共 API 调用。
- `CcbdStartupAgentResult` 不修改；`RuntimeBindingState` / `AgentRuntimeResult` 保持当前定义。
- 风险：tmux backend 接口（`ccb_terminal::TmuxBackend`）需要支持 `set_pane_title` / `set_pane_user_option`；若未实现，先在 fake backend 中抽象。

## Rollback

按 provider 分支隔离；若某 provider launch 分支失败，仅 revert 对应 `provider_launcher.rs` match arm 与 tests。
