# Python → Rust 迁移设计文档

## 1. 设计目标

- 让 Rust 工作空间成为 CCB 的**唯一运行时真相源**。
- 保持与 Python 参考实现**行为等价**，不引入新的用户可见语义。
- 按 crate 边界逐步迁移，避免一次性重写导致不可控回归。
- 复用 `ccb-provider-core`、`ccb-types`、`ccb-storage` 等共享 crate，减少跨层重复。

## 2. 模块映射

| Python 模块 | Rust Crate | 迁移优先级 | 备注 |
|-------------|------------|-----------|------|
| `lib/types/` 相关 | `crates/ccb-types/` | P0 | 共享协议、模型、枚举 |
| `lib/storage/` + 路径/分类 | `crates/ccb-storage/`, `ccb-storage-classification/` | P0 | 状态持久化、路径布局 |
| `lib/ui_text/` | `crates/ccb-ui-text/` | P0 | 国际化与文案 |
| `lib/project/` + `lib/project_memory/` | `crates/ccb-project/`, `ccb-workspace/`, `ccb-memory/` | P0 | 项目与 workspace 抽象 |
| `lib/heartbeat/` | `crates/ccb-heartbeat/` | P0 | 健康监测引擎 |
| `lib/jobs/` | `crates/ccb-jobs/` | P0 | 任务存储 |
| `lib/mailbox_kernel/` + `lib/mailbox_runtime/` | `crates/ccb-mailbox/` | P1 | 异步消息传递 |
| `lib/message_bureau/` | `crates/ccb-message-bureau/` | P1 | 控制队列、trace、facade |
| `lib/terminal/`（tmux 后端） | `crates/ccb-terminal/`, `ccb-pane-registry/` | P1 | pane 管理、布局 |
| `lib/ccbd/` | `crates/ccb-daemon/` | P1 | daemon 控制平面 |
| `lib/cli/` | `crates/ccb-cli/` | P1 | CLI 解析与命令路由 |
| `lib/completion/` | `crates/ccb-completion/` | P2 | 完成检测、profile、selector |
| `lib/agents/` | `crates/ccb-agents/` | P2 | 角色包、拓扑、布局 |
| `lib/memory/` | `crates/ccb-memory/` | P2 | 会话解析、自动转移、格式化 |
| `lib/provider_backends/` | `crates/ccb-providers/` + provider 子模块 | P2 | 各 provider 启动器、通信器、执行 |
| `lib/ask_cli/` | `tools/` 或 `ccb-cli/` 子命令 | P2 | `/ask` 入口 |

## 3. 迁移策略

### 3.1 分阶段推进

采用 **inside-out** 顺序：先核心共享层，再运行时服务，最后 provider 和 CLI。

1. **Phase 0：审计与基线** — 完成现状盘点，确认每个 crate 的已实现 API、测试缺口、与 Python 的行为差异。
2. **Phase 1：核心层** — 完成 `ccb-types`、`ccb-storage`、`ccb-ui-text`、`ccb-project`、`ccb-workspace`、`ccb-jobs`、`ccb-heartbeat`。
3. **Phase 2：通信与终端** — 完成 `ccb-mailbox`、`ccb-message-bureau`、`ccb-terminal`、`ccb-pane-registry`。
4. **Phase 3：控制平面与 CLI** — 完成 `ccb-daemon`、`ccb-cli`。
5. **Phase 4：完成与 Provider** — 完成 `ccb-completion`、`ccb-agents`、`ccb-memory`、`ccb-providers` 及 provider 子系统。
6. **Phase 5：测试对等与 Python 退役** — 补齐测试映射，移除 release tarball 中的 Python 实现，更新 `rust-python-test-parity-matrix.md`。

### 3.2 双轨运行

- 在迁移完成前，允许 Python 实现作为**行为参考**继续存在。
- 同一功能在 Rust 中实现后，对应的 Python 单元测试可标记为 `rust_equivalent`，并在 CI 中跳过。
- 关键路径保留端到端回归测试，确保 Rust 行为与 Python 参考一致。

### 3.3 边界处理

- **控制平面协议**：`ccbd` socket 协议保持字节级兼容；新增字段仅通过 JSON 扩展，旧客户端忽略未知字段。
- **tmux 命名空间**：pane 命名、window 命名、session 命名逻辑与 Python 完全一致，防止运行时状态漂移。
- **存储路径**：`ccb-storage` 使用与 Python 相同的路径布局，确保用户数据无缝迁移。
- **provider hook**：保留 `bin/ccb-provider-*-hook` 作为 source-install 入口，Rust 侧通过统一 hook 协议调用。

## 4. 数据流

```text
User CLI (ccb-cli)
      │
      ▼
 ccbd control plane (unix socket / JSON-RPC-like)
      │
      ▼
ccb-daemon service graph
      │
      ├─────────────┬─────────────┬──────────────┐
      ▼             ▼             ▼              ▼
ccb-terminal  ccb-mailbox  ccb-providers  ccb-completion
      │             │              │               │
      ▼             ▼              ▼               ▼
   tmux          message        provider       job/agents
   backend       bureau         backends       memory
```

- `ccb-cli` 仅作为客户端，不直接操作 tmux 或 provider。
- `ccb-daemon` 持有运行时真相，所有状态变更通过 daemon 服务图完成。
- `ccb-mailbox` 与 `ccb-message-bureau` 负责异步消息投递与控制队列。
- `ccb-providers` 通过 `ccb-provider-core` 抽象与具体 provider CLI 交互。

## 5. 测试策略

- **单元测试**：每个 crate 的 `tests/` 下按模块组织，覆盖核心逻辑和边界条件。
- **集成测试**：每个 crate 的 `tests/*_integration_tests.rs` 覆盖跨模块交互。
- **Python 参考测试**：保留为 `test/` 下的对照组，迁移一个模块后退役对应 Python 测试。
- **Parity Matrix**：`plans/rust-python-test-parity-matrix.md` 是测试映射的唯一真相源，每个 child task 必须更新。
- **CI 验证**：`cargo test --workspace -- --test-threads=1` 和 `cargo clippy --workspace`。

## 6. 兼容性策略

- **向后兼容**：socket 协议、配置文件格式、tmux namespace、存储路径均保持兼容。
- **新配置项**：新增项必须有默认值，旧配置无感升级。
- **Provider 兼容**：新 Rust provider 抽象必须支持所有现有 provider（Codex、Claude、Gemini、OpenCode、Droid、AGY）。
- **Rollback**：每个 phase 结束前保留可回滚的 git tag；若某 crate 迁移失败，可切换回 Python 参考实现继续发布。

## 7. 决策记录

| 决策 | 理由 |
|------|------|
| 按 crate 而非按文件迁移 | crate 对应现有 Rust 工作空间边界，与 CCB 子系统边界一致，便于独立测试和回滚 |
| 保留 Python hook 脚本 | provider 启动和 session 管理需要与外部 CLI 工具交互，hook 脚本是 source-install 兼容层 |
| 不迁移 Windows/WSL 工具到 Rust | 当前发布主战场为 Linux/macOS，Windows 工具可后续单独处理 |
| 使用 parity matrix 驱动测试退役 | 明确记录哪些 Python 测试已被 Rust 替代，避免测试重复或遗漏 |
