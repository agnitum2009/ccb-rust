# 方案 A 周期评估：P0/P1 可用 + 放弃全面 1:1

方案 A 的核心：**不再追求逐文件目录对齐，以 `ccbr start / ask / reload` 三个命令端到端可用为验收标准**，清理 A/C 类占位 stub，内部模块划分允许符合 Rust 习惯。

## 验收标准

1. `ccbr start <project>` 能创建 tmux session、按布局启动 agent pane。
2. `ccbr ask <agent> <message>` 能把消息提交到目标 agent，并能收到回复。
3. `ccbr reload` 能热更新 agent/window 布局且不丢失会话。
4. `cargo test --workspace` 与 `cargo clippy -p ccb-daemon --no-deps -- -D warnings` 保持通过。

## ccb-daemon：1.5–2.5 周

| 工作项 | 内容 | 天数 |
|---|---|---|
| P0 收尾 | reply-delivery 格式化/重试/最终化、mount 异常恢复、start_flow orchestrator 归一 | 3–5 |
| P1 收尾 | drain/handoff 边界、transaction rollback、additive mount 验证 | 2–4 |
| A/C 类清理 | 84 个 A 类改为 re-export 或删标记；13 个 C 类标注/删除 | 1–2 |
| 目录重对齐（渐进） | 仅对正在实现的模块做必要移动，不批量重构 | 1–2 |
| 集成调试 | 与 ccb-mailbox / ccb-completion / ccb-terminal 联调 | 1–2 |
| **小计** | | **8–15 天** |

## ccb-providers：1–1.5 周

381 个存根看似很大，但 6 个 provider（Codex/Claude/Gemini/OpenCode/Droid/AGY）结构高度相似：

- 实现 1 个参考 provider 的 launcher + communicator + session + polling（约 3–4 天）。
- 其余 5 个按参考模板调整命令路径、CLI 参数、环境变量（约 2–4 天）。
- provider-core 的抽象层已存在，主要工作是填充 backend。

## ccb-cli：0.5–1 周

196 个存根中，P0/P1 只需要：
- `start`、`ask`、`reload`、`status`、`stop` 等核心命令。
- 其余命令可先保留 stub 或返回 "not yet implemented"。

约 5–7 天可覆盖关键路径。

## 其他依赖 crate：1 周

| Crate | 天数 | 说明 |
|---|---|---|
| ccb-mailbox | 2–3 | 消息投递、收件箱、路由 |
| ccb-completion | 1–2 | 完成协调、job 生命周期 |
| ccb-agents | 3–5 | 布局解析、角色包、项目配置 |
| ccb-memory | 2 | 仅保证 P0/P1 需要的记忆读写 |
| ccb-terminal | 1 | 已有基础，补缺口 |
| ccb-storage / heartbeat | 1 | 可忽略或最小实现 |
| **小计** | **9–13 天** | |

## 端到端测试与打磨：0.5–1 周

- 在真实 tmux 环境跑 `start → ask → reload` 全流程。
- 修边界 bug、补齐缺失错误处理。
- 更新用户文档 / AGENTS.md 中 Rust 相关说明。

## 总周期

| 阶段 | 墙钟 |
|---|---|
| ccb-daemon P0/P1 收尾 | 1.5–2.5 周 |
| ccb-providers | 1–1.5 周 |
| ccb-cli | 0.5–1 周 |
| 其他依赖 crate | 1 周 |
| 端到端测试与打磨 | 0.5–1 周 |
| **总计** | **4.5–7 周** |

取中值：**约 5–6 周**。

## 与全面 1:1 对比

| 方案 | 周期 | 产出 |
|---|---|---|
| 全面 1:1 | 8–12 周 | 完整目录/文件对齐，但大量重复和 A 类清理 |
| 方案 A | 4.5–7 周 | 核心命令可用，内部 Rust 化，后续按需补模块 |

## 风险

1. ** start_flow_runtime_service.rs 与 start_flow/service.rs 归一**：需要决定保留哪个 orchestrator，可能引入 1–2 天额外重构。
2. **Provider 后端差异**：某些 provider（如 AGY、Droid）的本地/远程启动方式差异较大，模板化可能不够，需单独处理。
3. **真实 tmux 行为**：多窗布局在真实终端中可能暴露 Rust 版与 Python 版在 pane 编号、窗口命名上的细微差异。
