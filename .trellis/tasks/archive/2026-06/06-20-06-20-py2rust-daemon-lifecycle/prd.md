# Daemon lifecycle parity

## Goal

补齐 CCB ccbd 控制面生命周期相关 Python 测试的 Rust parity。本任务聚焦 step 1：daemon 生命周期（kill / ask / start / ps / wait / runtime launch / socket client / health / namespace 等）。其中 `kill_service` 与 `ask_service` 的 parity 已在前期回合完成，剩余重点是 `runtime_launch`（`test_v2_runtime_launch.py`、`test_v2_runtime_launch_session_files.py`）以及 `ps_service` / `wait_service` / `start_service` / `start_foreground` / `daemon_startup_wait` 等守护进程生命周期服务。

## Confirmed facts

- Python 参考实现位于 `lib/ccbd/`、`lib/cli/services/`、`lib/provider_backends/`、`lib/terminal_runtime/`。
- Rust 实现位于 `rust/crates/ccb-daemon/`、`rust/crates/ccb-cli/`、`rust/crates/ccb-providers/`、`rust/crates/ccb-terminal/`。
- `test_v2_runtime_launch.py` 覆盖：
  - 各 provider launcher 的 `build_start_cmd`（Codex、Claude、Gemini、AGY、Droid、OpenCode）。
  - `ensure_agent_runtime` 的完整编排（pane 创建/复用、binding 更新、detached fallback、stale/dead/foreign binding 处理、tmux namespace 限制）。
  - `runtime_shared` 的 provider 可执行文件解析、环境变量覆盖、command template 应用。
  - provider workspace/home 准备（Codex home layout、Claude managed home、OpenCode memory config、AGY conversation metadata）。
- 已完成的 Rust parity 包括：`kill_service`、`ask_service`、`runtime_shared` 部分测试、`start_preparation` 部分测试、`workspace_preparation` hook-home-root、`session_binding` 策略、`codex_session_root_path`、`opencode` memory materialization、`claude` home layout。
- 剩余主要缺口：Codex/Claude/Gemini/AGY/Droid 的 `build_start_cmd` 与 launch context、Codex/Claude 的 managed home 准备、`ensure_agent_runtime` 编排、以及 `ps_service` / `wait_service` / `start_service` 等 CLI daemon 服务。

## Requirements

1. 在 Rust 中实现/补全各 provider launcher 的 `build_start_cmd`、`prepare_launch_context`、`build_session_payload`、`post_launch`（按 Python 1:1 行为）。
2. 在 Rust 中实现 `ensure_agent_runtime` 编排（或等价的 daemon start-runtime 绑定解析 + pane 启动流程），使其通过 `test_v2_runtime_launch.py` 中对应的直接调用测试。
3. 保持现有 Rust 架构：provider-specific 逻辑放在 `ccb-providers`，编排逻辑放在 `ccb-daemon`，CLI 服务放在 `ccb-cli`。
4. 每新增/修改一个模块，必须添加对应的 Rust 单元/集成测试；测试需与 Python 测试的断言点等价。
5. 更新 `plans/rust-python-test-parity-matrix.md`，将新覆盖的测试映射到 Rust 文件/测试。

## Acceptance criteria

- [ ] `cargo test -p ccb-providers -- --test-threads=1` 中新增 Codex/Claude/Gemini/AGY/Droid launcher parity 测试全部通过。
- [ ] `cargo test -p ccb-daemon -- --test-threads=1` 中新增 `ensure_agent_runtime` / `runtime_launch` 编排测试全部通过。
- [ ] `cargo test -p ccb-cli -- --test-threads=1` 中新增 `ps_service` / `wait_service` / `start_service` parity 测试全部通过（或至少本次迭代目标范围通过）。
- [ ] `plans/rust-python-test-parity-matrix.md` 的 `runtime_launch`、`ps_service`、`wait_service`、`start_service` 行更新为 `partial` → `complete`（或当前迭代范围）。
- [ ] 没有引入新的编译错误；相关 crate 的 `cargo clippy -p <crate> --tests` 通过或仅保留既有告警。

## Out of scope

- 步骤 2~5（terminal namespace/pane identity、provider hooks/profiles/settings、端到端多 agent 恢复、Windows/WSL 工具链）不在本任务内，但本任务输出的 launcher / runtime 接口应为后续步骤保留扩展点。
- 真实 provider CLI 的实时交互测试仍保留 Python 参考实现；Rust 侧使用 mock/filesystem/tmux stub。

## Open questions

- 是否将 `runtime_launch` 拆分为多个子任务（按 provider 或按编排层）？建议：由于各 provider launcher 可独立验证，拆分为子任务可并行推进。
