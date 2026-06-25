# Design: CCB 剩余 Python→Rust parity 迁移

## 总体结构

采用 4-wave dependency-ordered 执行顺序。Wave 1 是架构杠杆点：没有 `Phase2Services` 的实现，后续 render/handler/launcher 零件无法被 CLI 端到端调用。因此 Wave 1 必须最先启动并完成。

```text
Wave 1: CLI Phase2Services impl
    │
    ▼
Wave 2: runtime launch + completion/heartbeat
    │
    ▼
Wave 3: providers deep + daemon deep
    │
    ▼
Wave 4: e2e recovery + terminal namespace + install/update/MCP/edge
```

## Wave 1 关键决策

### 1.1 `Phase2Services` 实现位置

新增 `crates/ccb-cli/src/phase2_services_impl.rs`（或复用 `phase2_services.rs` 的 `DaemonPhase2Services`）。

- 优先复用已有的 `DaemonPhase2Services`（已基于 `CcbdClient` 转发 daemon RPC）。
- 当前 `DaemonPhase2Services` 中多个方法返回 `"not yet implemented"` 错误；Wave 1 的目标是让核心命令可跑，因此需要：
  - 补全 `ps_summary`、`ping_target`、`queue_target`、`trace_target`、`watch_target`、`inbox_target`、`ack_reply`、`cancel_job`、`start_agents` 等方法的 daemon RPC 或本地实现。
  - 确保 `submit_ask` 与 `restart_agent` 已可工作（本会话已验证 ask/restart 渲染）。

### 1.2 dispatch 入口

确认 `ccb` 主入口在 `src/entry.rs` 中路由到 phase2 dispatch。当前已有 `dispatch` 调用；Wave 1 需要确保 `run_cli` 在识别 v2 命令时实际走 `dispatch`，而不是 legacy `commands.rs`。

### 1.3 测试策略

- 每个命令先补一个最小 Rust 集成测试，验证 `dispatch → Phase2Services impl → render` 输出与 Python 一致。
- 优先覆盖：ps、ping、wait、kill、start、ask、restart、logs、maintenance、reload。

## Wave 2 关键决策

### 2.1 runtime launch 编排

承接 `06-20-py2rust-daemon-lifecycle` 的 Phase B。文件集中在：

- `crates/ccb-daemon/src/start_runtime/agent_runtime.rs`
- `crates/ccb-daemon/src/start_runtime/agent_runtime_binding.rs`
- `crates/ccb-daemon/src/start_runtime/ensure_agent_runtime.rs`

需要实现：detached fallback、stale binding 检测、foreign binding 处理、tmux namespace 限制下的 pane 复用。

### 2.2 completion / heartbeat

- `crates/ccb-completion/` 已接入 daemon heartbeat 的 `CompletionTrackerService`。
- Wave 2 需要补齐 job store 与 completion 编排的剩余 parity（`test_v2_ccbd_dispatcher.py` 等 dispatcher/fastpath 之外的部分）。
- heartbeat classifier 已较完整，但 CLI maintenance 编排（`test_maintenance_heartbeat.py`）在 Python 侧，需移植到 Rust CLI。

## Wave 3 关键决策

### 3.1 providers deep

按 provider 拆分：`codex`、`claude`、`gemini`、`droid`、`agy`、`opencode`。

每个 provider 需要补齐：

- execution adapter（poll/start/submit）
- communicator / session binding
- pane log support parsing
- hook settings / activity scripts 的 Rust 等价实现或明确保留 Python hook 脚本

### 3.2 daemon deep

`ccb-daemon` 348 stub 主要分布在：

- `services/dispatcher_runtime/`
- `services/project_namespace_runtime/`
- `reload_*` 模块
- `supervision/`

Wave 3 按子主题（dispatcher、namespace、reload、supervision）创建孙任务或子 PRD。

## Wave 4 关键决策

### 4.1 e2e recovery

- 多 agent 会话持久化/恢复：`test_v2_ccbd_*` 系列。
- 依赖 Wave 2/3 的 runtime launch 与 provider adapter 实现。

### 4.2 terminal namespace / pane identity

- `crates/ccb-terminal/` 与 `crates/ccb-daemon/` 的 namespace/state 集成。

### 4.3 边缘 parity

- install/update 系列：`test_cli_management_update.py`、`test_install_*.py`。
- MCP delegation：`test_mcp_delegation_server*.py`。
- Windows/WSL：如无 Rust 等价，写入 out-of-scope 记录。

## Cross-layer considerations

- `Phase2Services` 是 CLI 与 daemon 之间的契约层。修改 service trait 或 payload 字段会影响 render 函数和 daemon handler，必须同步更新。
- provider adapter 与 completion tracker 通过 job store / execution service 交互，新增 completion item kind 或 decision 字段会跨 `ccb-completion`、`ccb-daemon`、`ccb-providers` 三层。
- 每次 wave 完成后必须更新 parity matrix，否则后续 wave 无法追踪剩余缺口。

## Rollback / 风险缓解

- Wave 1 若发现 `Phase2Services` 设计需要 trait 签名变更，应先在 design.md 中记录并重新 review，再进入实现。
- 每个 provider/daemon 子主题在 Wave 3 应独立可测，避免一次性大 PR。
- 真实 provider CLI 测试保持 mock；不引入外部 API 调用。
