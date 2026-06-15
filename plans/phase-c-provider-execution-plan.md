# Phase C: Provider Execution 计划（已实施）

## 目标
让 `ccb-daemon` 在 `start_flow` 创建 tmux pane 后，把对应 provider 的真实 CLI 启动到 pane 中。

## 已实施方案
采用“方案 B 变体”：在 daemon 侧接入 `ccb-providers` / `ccb-provider-core` 的 provider backend registry 与 runtime launcher 抽象，并补齐实际缺失的“把命令发送到 pane”这一步。

### 新增/修改文件

| 文件 | 说明 |
|---|---|
| `rust/crates/ccb-daemon/src/provider_launcher.rs` | 新增。持有 `ProviderBackendRegistry`，按 provider 构建启动命令，通过 `TmuxBackend::send_text` + Enter 发送到 pane。对 opencode 使用 `ccb_providers::opencode` 的 launcher 并写入 session payload 文件。 |
| `rust/crates/ccb-daemon/src/start_flow/service.rs` | 创建 layout 后，从 `AgentRegistry` 读取每个 agent 的 provider，调用 `ProviderLauncher` 启动；失败时记录 `failed` 状态与原因。 |
| `rust/crates/ccb-daemon/src/app.rs` | `run_start_flow` 传入 `&self.registry`；`load_agent_registry` 使用 `layout.project_root`（已 canonicalize）作为默认 workspace_path，避免相对路径 `.`。 |
| `rust/crates/ccb-daemon/src/lib.rs` | 注册 `provider_launcher` 模块。 |
| `rust/crates/ccb-daemon/src/services/registry.rs` | 新增 `update_pane_id`，更新 pane 时把 `registered` 平滑转为 `idle`。 |
| `rust/crates/ccb-daemon/src/stop_flow/service.rs` | 按 pane_id kill pane，而不是直接 kill session。 |
| `rust/crates/ccb-cli/src/entry.rs` | 修复 `parse_start`：显式 `ccb start agent1` 时不再把 `start` 当 agent name。 |

## 验证

- `cargo test --workspace --all-targets -- --test-threads=1`：全部通过
- `cargo clippy --workspace --all-targets -- -D warnings`：clean
- `cargo fmt -- --check`：clean
- 手动 E2E（`CODEX_START_CMD=/bin/sleep 30`）：`ccb start agent1` 后 tmux pane 中出现 `sleep` 子进程，`stop` 后 pane 被清理。
- 手动 E2E（`OPENCODE_START_CMD=/bin/sleep 30`）：opencode 启动写入 `agent1-session.jsonl`，包含 pane_id、start_cmd、workspace_path 等字段。

## 已知限制

- 这仍是“启动 provider CLI”层，尚未把 `ccb-providers::ExecutionService` 的 job/poll 循环接入 daemon；初始 prompt 的投递、session 文件轮询属于 Phase D/E。
- provider 启动命令目前依赖 `provider_start_parts` 或 opencode launcher；后续可在 `ccb-providers` 为其他 provider 补充专用 launcher。
