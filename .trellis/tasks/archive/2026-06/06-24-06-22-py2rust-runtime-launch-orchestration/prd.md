# W2: runtime launch orchestration parity

## Goal

补齐 CCB Rust 侧 `ensure_agent_runtime` 编排，使其与 Python `lib/cli/services/runtime_launch.py` + `lib/cli/services/runtime_launch_runtime/ensure.py` 行为等价，并覆盖 `test_v2_runtime_launch.py` 中的编排断言点。

## Context

- Python 参考实现：`lib/cli/services/runtime_launch.py`、`lib/cli/services/runtime_launch_runtime/ensure.py`、`lib/cli/services/runtime_launch_runtime/tmux_runtime.py`、`lib/cli/services/runtime_launch_runtime/tmux_panes.py`、`lib/cli/services/runtime_launch_runtime/session_files.py`。
- Rust 已有基础：
  - `crates/ccb-daemon/src/start_runtime/agent_runtime*.rs` 已实现 `resolve_runtime_binding_state` / `start_agent_runtime` 的绑定决策与 attach 抽象（依赖注入）。
  - `crates/ccb-daemon/src/provider_launcher.rs` 已支持 opencode / kimi / mimo / deepseek 的 launch 分支。
  - `crates/ccb-providers` 已完成 codex / claude / gemini / agy / droid 的 `build_start_cmd` parity。
  - `crates/ccb-cli/src/services/runtime_launch_runtime/` 已存在 `ensure.rs`、`tmux_panes.rs`、`tmux_runtime.rs`、`session_files.rs`、`tmux_backend.rs` 等文件，但多为 stub 或未接入 `ProviderLauncher`。

## Requirements

1. **实现 `ensure_agent_runtime` 编排器**
   - 输入：`(context, command, spec, plan, binding, assigned_pane_id, style_index, tmux_socket_path)`。
   - 复用 `ccb-daemon/src/start_runtime/agent_runtime_binding.rs` 的绑定决策逻辑或等价实现。
   - 当 binding 可复用（有 `runtime_ref` + `session_ref` 且 pane alive）时直接返回 `launched=false`。
   - 否则：
     - 从 `ccb-provider-core` registry 取得 provider `ProviderRuntimeLauncher`。
     - 准备 provider workspace（复用/补齐 `prepare_provider_workspace`）。
     - 创建/复用 tmux pane：优先 `assigned_pane_id`，否则 detached fallback。
     - 调用 provider `build_start_cmd` + `build_session_payload`，写入 session 文件。
     - 应用 CCB pane identity（title/agent_label/project_id/order_index/slot_key/session_id）。
     - 解析刷新后的 binding 并返回 `RuntimeLaunchResult { launched: true, binding }`。

2. **扩展 `ProviderLauncher` 分支**
   - 在 `crates/ccb-daemon/src/provider_launcher.rs` 中新增 codex / claude / gemini / agy / droid 的 launch 分支，复用 `ccb-providers` 对应模块的 `prepare_launch_context`、`build_start_cmd`、`build_session_payload`。

3. **保留 trait/依赖注入接口**
   - `ensure_agent_runtime` 内部对 tmux backend、session file writer、provider launcher 的调用通过 trait/函数参数注入，便于单元测试 mock。

4. **测试**
   - 新增/扩展 `crates/ccb-daemon/tests/runtime_launch_ensure_agent_runtime_tests.rs`：
     - binding 可复用 → 不 launch。
     - binding 缺失 + mock tmux backend → 创建 pane、写入 session 文件、返回 launched binding。
     - stale binding → cleanup 旧 pane 后 launch。
     - detached fallback 路径。
   - 新增 `crates/ccb-daemon/tests/provider_launcher_codex_claude_tests.rs` 验证 codex/claude 分支产生的 command / session_path 形状。

## Acceptance criteria

- [ ] `cargo test -p ccb-daemon -- --test-threads=1` 中新增 orchestration 测试通过。
- [ ] `cargo test -p ccb-daemon` 无回归。
- [ ] `cargo clippy -p ccb-daemon --tests -- -D warnings` 通过或仅保留既有告警。
- [ ] `cargo fmt -p ccb-daemon` 干净。
- [ ] `plans/rust-python-test-parity-matrix.md` 更新 `runtime_launch` 行状态与映射文件。

## Out of scope

- 真实 provider CLI 实时交互测试（仍保留 Python 参考）。
- Windows / WSL 工具链。
- `completion` / `heartbeat classifier` parity（属于另一个 Wave 2 任务）。
