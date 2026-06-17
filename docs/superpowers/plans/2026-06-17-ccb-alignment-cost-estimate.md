# CCB Python→Rust 1:1 对齐成本与周期预估

基于 Phase 1 `ccb-daemon` 的实施数据推算。

## 一、当前状态

| 维度 | 数据 |
|---|---|
| ccb-daemon 总存根 | 430 |
| 已实现（不再含 stub 标记） | ~36 |
| 仍为 stub / 缺失 | ~394 |
| A 类（Rust 别处已实现） | 84 |
| B 类（真正缺失） | 307 |
| C 类（不适用） | 13 |
| B 类复杂度分布 | Small 181 / Medium 146 / Large 21 |
| 其他 crate 存根总数 | ~1,189 |

## 二、ccb-daemon 剩余工作量估算

### 2.1 按类别拆分

| 类别 | 数量 | 单件参考工时 | 人时合计 | 说明 |
|---|---:|---:|---:|---|
| A 类清理 | 84 | 0.1 h | ~8 h | 删除 stub 标记、改为 re-export 或确认已覆盖 |
| B-Small | ~150 | 0.5 h | ~75 h | 小数据类 / 纯工具函数 / 简单包装 |
| B-Medium | ~120 | 1.5 h | ~180 h | 业务逻辑、多函数模块、需加测试 |
| B-Large | ~15 | 4 h | ~60 h | 核心编排器（如剩余 mount/dispatcher 深层模块） |
| C 类标注 | 13 | 0.1 h | ~1 h | 标记不适用原因 |
| 目录重对齐* | — | — | ~40 h | 删除 150 个重复 flat 文件、移动 2 个真实代码文件、审计 lib.rs/mod.rs、修复 import |
| **小计** | | | **~364 h** | 约 **9 人周**（串行） |

\* 目录重对齐可随实现渐进完成；若单独批量做，约 1 周。

### 2.2 并行/子代理压缩后的墙钟时间

子代理可把大量 Small/Medium 任务并行化，但存在依赖链：

- **无依赖可并行的任务**：A 类 + B-Small 中约 60% → 可压缩到 2–3 天墙钟
- **有依赖链的任务**：B-Medium/Large 编排器（namespace → mount → dispatcher → start_flow）→ 约 1–1.5 周
- **集成调试、测试、clippy 清理**：约 2–3 天

**ccb-daemon 全部收尾**：在持续使用子代理并行的情况下，预计 **2–3 周墙钟**。

### 2.3 仅 P0/P1 关键路径

已覆盖 P0/P1 优先级清单中的约 20–24 个核心模块。剩余关键项主要是深层格式化、重试、最终化策略和边界错误处理。

- **P0 剩余收尾**（reply-delivery 细节、mount 异常恢复、start_flow 主 orchestrator 与 flat `start_flow_runtime_service.rs` 的归一）：约 **3–5 天**
- **P1 剩余收尾**（drain/handoff/mount 边界 case、transaction 回滚细节）：约 **2–4 天**

## 三、其他 crate 工作量估算

| Crate | 存根数 | 人时估算 | 墙钟估算 | 备注 |
|---|---:|---:|---:|---|
| ccb-providers | 381 | ~280 h | 2–3 周 | 多 provider（Codex/Claude/Gemini/Droid/AGY/OpenCode），后端结构相似，可批量模板化 |
| ccb-cli | 196 | ~160 h | 1.5–2 周 | CLI 命令、参数解析、状态机，很多依赖 ccb-daemon |
| ccb-agents | 75 | ~70 h | 4–6 天 | 角色包、布局、项目解析 |
| ccb-memory | 56 | ~50 h | 3–5 天 | 记忆存储、检索、序列化 |
| ccb-provider-core | 46 | ~40 h | 3–5 天 | provider 抽象层、启动器、通信器 |
| ccb-mailbox | 19 | ~20 h | 1–2 天 | 消息投递、收件箱 |
| ccb-completion | 11 | ~15 h | 1–2 天 | 完成协调 |
| ccb-terminal | 7 | ~10 h | 1 天 | tmux 后端，底层命令 |
| ccb-storage / ccb-heartbeat | 4 | ~5 h | <1 天 | 极少 stub |
| **其他合计** | **~1,189** | **~650 h** | **5–7 周** | |

## 四、全项目总估算

| 范围 | 人时 | 墙钟（子代理并行） |
|---|---:|---:|
| ccb-daemon 收尾 | ~364 h | 2–3 周 |
| 其他 crate 对齐 | ~650 h | 5–7 周 |
| 集成测试 + 端到端验证 + 文档 | ~120 h | 1–2 周 |
| **总计** | **~1,134 h** | **8–12 周** |

## 五、关键假设与风险

1. **子代理可用且稳定**：估算基于可并行派发多个子代理；若串行执行，周期翻倍以上。
2. **Python 参考稳定**：若 Python 侧同时演进，对齐成本会显著上升。
3. **目录重对齐一次性完成**：若选择批量做，需额外 1 周；渐进做则摊入各模块实现中。
4. **ccb-terminal 债务**：当前 `ccb-terminal` 有 80 个 clippy 警告，如后续需严格 `-D warnings`，需额外 1–2 天清理。
5. **真实 tmux 集成测试**：多窗 UI 最终验证需要真实 tmux 环境，可能暴露未覆盖的边界行为。

## 六、建议节奏

- **最近 2 周**：完成 ccb-daemon P0/P1 收尾 + 目录重对齐决策落地。
- **第 3–6 周**：ccb-providers + ccb-cli 对齐（用户可见命令和 provider 生命周期）。
- **第 7–10 周**：ccb-agents / ccb-memory / ccb-mailbox / ccb-completion 对齐。
- **第 11–12 周**：全workspace端到端测试、性能/稳定性验证、文档更新。
