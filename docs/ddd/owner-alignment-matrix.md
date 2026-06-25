# Owner Alignment Matrix — Python 7.5.2 vs Rust ccbr

> 方法论：第一性原理 + MECE 10 域拆解 + codegraph 系统性 owner 对齐。
> 产出：11 个系统性 gap，按影响排序。

## MECE 10 域 Owner 对齐

| # | 域 | Python 单一 owner | Rust owner | Gap 数 |
|---|---|---|---|---|
| 1 | Daemon lifecycle | lifecycle.py (startup→restore, shutdown→terminate) | app.rs (start/heartbeat/shutdown) | 1（restore 分散） |
| 2 | Topology/namespace | materialize_topology（含 sidebar + apply_project_tmux_ui） | materialize_topology（但 start_flow 绕过 + 不含 apply_project_tmux_ui） | **4** |
| 3 | Agent registry | AgentRegistry | registry（AgentRuntimeEntry） | 1（pane_id 漂移） |
| 4 | Job lifecycle | JobDispatcher facade → dispatcher_runtime | JobDispatcher + handlers + app | **3** |
| 5 | Mailbox/messaging | MessageBureauFacade + ControlService | ccbr-mailbox crate | 1（comms view） |
| 6 | Provider launch | provider_backends/codex/launcher_runtime | ccbr-providers/codex/launcher_runtime | 1（AGENTS.md OMX） |
| 7 | Project view | ProjectViewService → build_project_view | **dual**（handler + orphan service） | **3** |
| 8 | CLI/protocol | cli/ + socket_server_runtime | ccbr-cli + socket_server | 0（基本对齐） |
| 9 | Configuration | config.py | ccbr-agents::config | 0（对齐） |
| 10 | Provider runtime | per-agent codex.bridge 进程 | heartbeat active-only poll | 0（架构差异，~40x CPU 优） |

## 11 个 Gap 详情

### P0（用户可见 blocker）

1. **start_flow 绕过 materialize_topology**（域 2）—— sidebar pane 不在 `ccbr start` 时创建
2. **@ccb_* vs @ccbr_* pane 标签**（域 2）—— Python UI（ccb-border.sh/status.sh/sidebar）读 `@ccb_agent`/`@ccb_role`；Rust 设 `@ccbr_role` → 找不到
3. **project_view dual ownership**（域 7）—— handler（活的）+ orphan service（死的）→ 形状不一致
4. **apply_project_tmux_ui 未内联**（域 2）—— Python materialize_topology 内联调；Rust 不调

### P1（功能缺失）

5. **comms view 缺 owner**（域 5+7）—— Python `_comms_view()` 从 dispatcher 取数据；Rust 硬编码 `[]`
6. **AGENTS.md 被 OMX 覆盖**（域 6）—— agent 看不到 coordination rules
7. **namespace 不完整**（域 7）—— 缺 epoch/socket/session/active_window/sidebar

### P2（架构不一致，非用户可见）

8. **pane-id 跟踪漂移**（域 3）—— registry pane_id ≠ 实际 tmux pane
9. **cancel 不在 dispatcher facade**（域 4）—— 独立 handler
10. **persist vs terminate divergence**（域 4）—— Rust 加了 persist_running_jobs（Python 无此概念）
11. **restore 分散**（域 1+4）—— app + dispatcher + execution 三处

## DDD 价值评估

| DDD 原则 | 对 Rust 的价值 | 对应 gap |
|---|---|---|
| **Aggregate Root（单一所有权）** | ✅ **高**：project_view dual ownership（#3）、topology 绕过（#1） | #1, #3 |
| **Bounded Context（边界清晰）** | ✅ **高**：start_flow vs materialize_topology 边界模糊 | #1, #4 |
| **Ubiquitous Language（统一命名）** | ✅ **中**：@ccb_* vs @ccbr_* 命名统一 | #2 |
| **State Machine（状态机封装）** | ⚠️ **中**：JobStatus 散落（DDD plan P0）但非当前 blocker | #10 |
| **Read Model（读模型分离）** | ✅ **高**：project_view 是 read model；需要单一 owner | #3, #5, #7 |
| **Domain Event（领域事件）** | ⚠️ **低**：当前无 event-driven 需求 | — |

## Trellis 约束（C1-C10 提取）

- **≤400 行/文件**：project_view handler 当前 ~70 行 ✓；orphan service ~100 行（合并后 ≤400 ✓）
- **≤20KB/文件**：当前文件均达标
- **≤999ms 验证**：cargo test project_view < 5s（✓）；live verify < 30s
- **手术式修改**：每次只改 1 个 gap + 验证（与 owner 对齐一致）
- **大任务 subagent**：每个 gap 作为一个 sub-agent task
