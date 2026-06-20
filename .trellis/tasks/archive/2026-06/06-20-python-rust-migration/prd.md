# Python → Rust 迁移项目管理（PRD）

## 1. 概述

将 CCB（v7.5.x）当前以 `lib/` 为核心的 Python 运行时实现，逐步迁移为以 `rust/` 工作空间为核心的 Rust 实现。Rust 实现应成为发布产物与运行时行为的唯一真相源，Python 代码保留为开发期参考、兼容 hook 和少数平台专用脚本。

## 2. 背景

- `lib/` 下约有 **13 万行** Python 代码，覆盖 CLI、daemon（ccbd）、mailbox、terminal、providers、completion、agents、memory、storage、heartbeat 等子系统。
- `rust/crates/` 已建立对应的 crate 骨架与部分实现，并已有 `plans/rust-python-test-parity-matrix.md` 记录当前测试映射状态。
- 发布流程已通过 `scripts/build_*.py` 将 Rust 二进制（`ccb`、`ccbd`、`ask`、`autonew`、`ctx-transfer`）打包为发布产物。
- 当前处于**部分迁移**：核心 crate 已有实现和集成测试，但大量 Python 行为尚未在 Rust 中对等实现。

## 3. 目标

1. **功能对等**：所有 P0/P1 运行时行为在 Rust 中有等价实现，行为与 Python 参考实现一致。
2. **测试对等**：Rust 测试集群覆盖 Python 参考测试所覆盖的核心场景，覆盖率达到或超过 Python 侧对应集群。
3. **发布切换**：正式 release tarball 只包含 Rust 原生二进制；`lib/` Python 实现不再进入 release。
4. **可维护性**：迁移后的代码遵循 `rust/crates/` 现有约定，文档、测试、错误处理、日志一致。

## 4. 范围

### 4.1 在范围内

- `lib/` 下各子系统按模块映射迁移到 `rust/crates/` 对应 crate：
  - `lib/cli/` → `crates/ccb-cli/`
  - `lib/ccbd/` → `crates/ccb-daemon/`
  - `lib/mailbox_kernel/` + `lib/mailbox_runtime/` → `crates/ccb-mailbox/`
  - `lib/message_bureau/` → `crates/ccb-message-bureau/`
  - `lib/terminal/`（如存在）→ `crates/ccb-terminal/`
  - `lib/provider_backends/` → `crates/ccb-providers/` + `crates/ccb-provider-core/`
  - `lib/completion/` → `crates/ccb-completion/`
  - `lib/agents/` → `crates/ccb-agents/`
  - `lib/memory/` → `crates/ccb-memory/`
  - `lib/project/` + `lib/project_memory/` → `crates/ccb-project/` + `crates/ccb-workspace/` + `crates/ccb-memory/`
  - `lib/storage/`（如存在）→ `crates/ccb-storage/`
  - `lib/heartbeat/` → `crates/ccb-heartbeat/`
  - `lib/jobs/` → `crates/ccb-jobs/`
  - `lib/ui_text/` → `crates/ccb-ui-text/`
  - `lib/types/` 相关 → `crates/ccb-types/`
- 将 `plans/rust-python-test-parity-matrix.md` 维护为动态文档，每完成一个模块即更新映射。
- 逐步退役已被 Rust 替代的 Python 测试，保留跨版本兼容性验证测试。

### 4.2 不在范围内

- 完全重写产品语义或引入新功能；迁移以行为等价为主。
- 删除 `bin/` 中作为 source-install 兼容层保留的 Python wrapper 与 provider hook 脚本。
- 将 Windows/WSL 专用工具一次性迁移到 Rust（可保留 Python 实现，按阶段处理）。
- 实时 provider CLI 的 live 集成测试（保留 Python 作为参考测试）。

## 5. 验收标准

1. `cargo test --workspace -- --test-threads=1` 全部通过。
2. 每个迁移模块的 Rust 测试集群至少覆盖 Python 参考测试对应集群中声明的 P0/P1 场景。
3. `python scripts/build_linux_release.py` 生成的 release tarball 不包含 `lib/` Python 实现（保留的 hook 脚本除外）。
4. 迁移后的 Rust 二进制能够启动 daemon、创建项目、启动 agents、发送 `/ask`、处理 mailbox 消息、完成一次 provider 执行周期。
5. `plans/rust-python-test-parity-matrix.md` 中所有 `partial` 集群的状态更新为 `complete` 或有明确 deferred 说明。
6. 代码风格通过 `cargo fmt --check` 和 `cargo clippy --workspace`（允许的 lint 除外）。

## 6. 约束与假设

- 必须保持 ccbd 控制平面协议与 socket 接口向后兼容，避免破坏现有客户端。
- 必须保持 tmux namespace 与 pane identity 逻辑，防止运行时状态错乱。
- 迁移按 crate 逐个推进，允许 Python 与 Rust 双轨运行，但最终发布产物仅依赖 Rust。
- 子任务应独立可测，每个 child task 对应一个模块或一个明确边界。

## 7. 风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 模块边界理解偏差导致 Rust 实现行为不一致 | 高 | 每个模块先写设计文档和测试映射，再实现；关键路径保留 Python 参考测试对比 |
| 大规模并行迁移导致回归难以定位 | 高 | 按 crate 分阶段，每阶段通过后再进入下一阶段；每次只激活一个 child task |
| 测试矩阵维护滞后 | 中 | 将矩阵更新作为每个 child task 的验收项之一 |
| provider 后端差异大，统一抽象困难 | 中 | 复用 `ccb-provider-core` 抽象，避免在每个 provider 中重复协议处理 |
