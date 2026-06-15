# Rust 第二稿规划：基于 Rust 语义的系统能力 1:1 还原

**日期**：2026-06-15
**前提**：第一稿（文件级 1:1 对齐）已完成——1317/1317 Python 文件有对应 .rs，编译通过，现有真实代码 105k LOC 未被修改。
**目标**：基于现有 Rust 代码结构 + 1:1 stub 对照，用 Rust 语义实现真正的系统能力还原。允许最小改动文件名/路径以符合 Rust 惯例。

---

## 1. 现状基线

- **真实代码**：340 文件 / 105,292 LOC —— 构成了当前可运行的 Rust CCB
- **stub 占位**：1,606 文件 / 4,965 LOC —— 每个平均 3 行，仅标记 Python 对应关系
- **运行验证**：`ccbr start/ask/ping/status/inbox/queue/shutdown` 全链路可用
- **编译验证**：`cargo build --workspace --release` 通过

## 2. stub 的三类用途

每个 stub 文件对照其 Python 源文件，归入三类：

### A 类：功能已由现有真实代码覆盖（~60% stub）
Python 文件的逻辑已被 Rust 在其他文件中实现（如 trait 方法、struct 实现、合并模块）。
**动作**：在 stub 中填入实际 Rust 文件位置的交叉引用注释，不需要写新代码。

### B 类：功能缺失，需要实现（~25% stub）
Python 有逻辑但 Rust 侧确实没有。
**动作**：按优先级翻译为 Rust，使用 Rust 语义（trait/enum/error/Result，不照搬 Python dict/class）。

### C 类：第三方/平台特定/已被架构替代（~15% stub）
Python 的平台适配、Windows 特定、或被 Rust 架构根本性替代的代码。
**动作**：在 stub 中记录 `// CATEGORY-C: not applicable in Rust (reason)`，不需要实现。

## 3. 实现优先级（按系统能力链路）

### P0：让 `ccbr start → ask → 完整多窗口 UI` 端到端可用

这是当前最大缺口（start 只起单 session，不多窗口拓扑）。

| 工作项 | Python 对照 | Rust 现状 | 目标 |
|--------|------------|----------|------|
| 配置驱动 start 拓扑 | `start_preparation.py` | ❌ 硬编码 "default" agent | 读 config.windows/agents 动态展开 |
| 多窗口 tmux 布局 | `reload_append_layout.py` | ❌ 单 session | 按 [windows] 拓扑建多窗口多 pane |
| Provider 启动注入 | `provider_launcher.rs` | ✅ 已实现但未被 start_flow 调用 | 接入 run_start_flow |
| sidebar 启动 | `ccb-agent-sidebar` | ✅ 二进制存在 | start 后自动启动 |
| reload 热重载 | `reload_apply_service.py` 系列 | ✅ 部分（dry_run + transaction） | 补齐 apply 全链路 |

### P1：让 daemon 控制平面完整对标

| 工作项 | Python 对照 | Rust 现状 |
|--------|------------|----------|
| agent supervision 重启 | `supervisor_runtime/` | ✅ 基础实现 |
| pane health 检查 | `services/health.rs` | ✅ 基础实现 |
| 生命周期管理（clear/reload/restart） | `handlers/project_*.rs` | ✅ 已接线 |
| session 恢复 | `restore_report_store.py` | ⚠️ 部分 stub |

### P2：让 CLI 命令面完整

| 工作项 | Python 对照 | Rust 现状 |
|--------|------------|----------|
| 命令分发 | `cli/entrypoint.py` | ✅ 已实现（手写 parser） |
| 渲染视图 | `render_runtime/` | ⚠️ 部分视图缺失 |
| 上下文传输 | `memory/transfer.py` | ✅ 已实现（codegraph 验证） |
| 自更新 | `management_runtime/` | ✅ versioning 已实现 |

### P3：补齐 provider backends

provider_backends 有 662 个 stub，但已有 149 个真实文件覆盖了核心逻辑。
大部分 stub 是 Python 内部 helper（A 类），少量是真正的功能扩展点（B 类）。

## 4. 第二稿设计原则

1. **Rust 语义优先**：用 `trait + impl` 而非 Python 的 `class + mixin`；用 `enum` 而非 string tags；用 `Result<T, E>` 而非 try/except。
2. **最小路径改动**：保留现有文件名和目录结构（第一稿已建立），仅在有充分 Rust 理由时调整（如 keyword 冲突、module 可见性）。
3. **stub → 真实代码的转化流程**：
   - 读 Python 源文件
   - 分类 A/B/C
   - B 类：按 Rust 语义重写，在对应 stub 文件中原地替换
   - A 类：添加交叉引用注释
   - C 类：标记跳过
4. **编译保持**：每实现一个 B 类 stub 后 `cargo build -p <crate>` 验证。
5. **测试对照**：每个 B 类实现配 Rust 单元测试，以 Python 行为为基准。

## 5. 验收标准

- [ ] P0：`ccbr start` 能按 `.ccb/ccb.config` 的 `[windows]` 拓扑建多窗口 + 启动 provider + sidebar
- [ ] P0：`ccbr ask <agent> <msg>` 能得到 provider 回复
- [ ] P1：`ccbr reload` 能热重载配置变更
- [ ] 所有 B 类 stub 被替换为真实 Rust 代码
- [ ] `cargo test --workspace` 持续通过，每个新实现有对照测试
- [ ] `cargo clippy --workspace --all-targets` 干净

## 6. 建议执行方式

由于 stub 数量大（1606），建议：
1. **subagent 并行**：每个 crate 一个 subagent，分类 A/B/C 后产出分类清单
2. **P0 串行**：P0 工作项（start 拓扑 + provider 注入）必须串行，因为依赖关系紧密
3. **B 类翻译可并行**：不同 crate 的 B 类 stub 翻译互不依赖，可用 subagent 并行

## 7. 与第一稿的关系

第一稿（文件级 1:1）是**脚手架**：
- 建立了 Python→Rust 文件映射（可对比）
- 没有破坏现有代码
- 每个文件的位置已固定

第二稿是**填充**：
- 在脚手架中填入真实 Rust 实现
- 允许最小结构调整（如把 stub 内容替换为 trait+impl）
- 最终产出：能完整运行、行为与 Python 版一致的纯 Rust CCB
