# Handoff: CCB Python → Rust 严格 1:1 转译（Phase 1 基础设施层）

**日期**：2026-06-13  
**工作目录**：`/home/agnitum/ccb`  
**Python 参考版本**：`/home/agnitum/ccb-v7.5.2`  
**Rust workspace**：`/home/agnitum/ccb/rust`  
**执行入口**：已统一改为 `ccbr`（原 `ccb`）

---

## 1. 项目目标

将 CCB v7.5.2 Python 项目（`lib/` 下的全部基础设施与业务包）按**严格 1:1** 转译到 Rust workspace（`rust/crates/`）。

原则：
- 每个 Python 公开函数 / 类 / 常量都要有 Rust 等价物。
- 不破坏现有 Rust 公共 API。
- 所有新增代码都要有对应测试。
- 每次改动后保持 `cargo test --workspace` 与 `cargo clippy --workspace -- -D warnings` 通过。

---

## 2. 当前整体状态

```text
cargo test --workspace -- --test-threads=1
total passed: 1338 total failed: 0

cargo clippy --workspace -- -D warnings
# clean

/root/.local/bin/codegraph sync
# Already up to date
```

---

## 3. 已完成模块（Phase 1 基础设施层）

| Python 包 | Rust crate | 说明 |
|---|---|---|
| `runtime_env` | `ccb-runtime-env` | 从 `ccb-types` 抽出，成为独立 canonical crate |
| `ui_text` | `ccb-ui-text` | 完整 1:1 |
| `stdio_runtime` | `ccb-stdio-runtime` | 完整 1:1 |
| `storage` | `ccb-storage` | 补齐 cursor_store、paths_agents、paths_ccbd、paths_targets 等 |
| `storage_classification` | `ccb-storage-classification` | 从 `ccb-storage` 独立出来 |
| `project` | `ccb-project` | 补齐 discovery、ids、resolver、runtime_paths |
| `workspace` | `ccb-workspace` | 完整 1:1（actors/binding/git_worktree/materializer/planner 等） |
| `runtime_pid_cleanup` | `ccb-runtime-pid-cleanup` | 完整 1:1 |
| `pane_registry_runtime` | `ccb-pane-registry` | 完整 1:1 |
| `mailbox_kernel` / `message_bureau` | `ccb-mailbox` / `ccb-message-bureau` | 公共边界重导出对齐 |
| `heartbeat` / `maintenance_heartbeat` | `ccb-heartbeat` | 根级重导出、注入式构造器、模型校验构造器 |
| `terminal_runtime` | `ccb-terminal` | 补齐 api/detect/backend_env/theme 及标题解析 helper |
| `completion` | `ccb-completion` | 补齐 BaseCompletionDetector/TickableCompletionDetector/REPLY_PRIORITY/CompletionValidationError |
| `provider_sessions` | `ccb-provider-sessions` | 根级重导出、构造器别名对齐 |
| `provider_hooks` | `ccb-provider-hooks` | 根级重导出精简对齐、write_activity/read_activity_evidence 行为对齐、settings_runtime helper 补齐 |
| `provider_core` | `ccb-provider-core` | 根级 62 项重导出、KIMI specs、protocol_runtime（CCB_BEGIN:/CCB_DONE:/CCB_REQ_ID:）、registry/session_binding/memory_projection 补齐 |

**CLI 入口**：
- `ccb` → `ccbr`：wrapper、binary、install.sh、release 脚本、Python 测试已更新。

---

## 4. 剩余模块（未完成）

按建议优先级排序：

| Python 包 | Rust crate | Py 文件 | Rust 文件 | 难度 | 建议顺序 |
|---|---|---:|---:|---|---|
| `agents` / `rolepacks` | `ccb-agents` | 53 / 61 | 10 | 大 | 1 |
| `memory` / `project_memory` | `ccb-memory` | 39 / 49 | 14 | 大 | 2 |
| `ccbd` / `fault_injection` | `ccb-daemon` | 367 / 371 | 79 | 很大 | 3 |
| `cli` / `ask_cli` | `ccb-cli` | 185 / 187 | 11 | 很大 | 4 |
| `provider_backends` / `provider_execution` / `provider_runtime` / `opencode_runtime` | `ccb-providers` | 569 / 648 | 74 | 很大 | 5 |
| `provider_model_shortcuts.py` | 无 | 1 | 0 | 小 | 6 |
| `release_artifacts.py` | 无 | 1 | 0 | 小 | 6 |
| `role_aliases.py` | 无 | 1 | 0 | 小 | 6 |

> 注：文件数是粗略代理指标，真正的 1:1 完成度还要看每个 public API 是否对齐。`message_bureau` / `mailbox_runtime` / `maintenance_heartbeat` 等功能已合并到对应 Rust crate 内部，公共边界已对齐。

---

## 5. 关键架构决策

1. **`ccb-runtime-env` 独立**
   - 原来内嵌在 `ccb-types`，现在抽出为 canonical crate，`ccb-types` 仅做重导出。
2. **`ccb-storage-classification` 独立**
   - 原来内嵌在 `ccb-storage`，现在单独 crate，避免存储与分类互相污染。
3. **`ccb-message-bureau` 作为 facade crate**
   - 核心实现在 `ccb-mailbox::bureau`，`ccb-message-bureau` 仅做 crate 边界重导出。
4. **命令入口统一为 `ccbr`**
   - 避免与系统可能存在的 `ccb` 冲突，wrapper 中加入 source guard 限制执行目录。
5. **`ccb-provider-core` 协议双轨**
   - 新增 `protocol_runtime/` 实现 Python `CCB_BEGIN:` / `CCB_DONE:` / `CCB_REQ_ID:` 语义；
   - 保留 legacy `protocol.rs`（`<<BEGIN:` / `<<DONE:` / `req-`）供现有 `ccb-providers` 使用，避免一次改垮下游。

---

## 6. 已知问题 / 风险

- 无当前阻塞性问题。
- `ccb-provider-core::materialize_provider_memory_file` 因循环依赖无法调用 `ccb-memory`，目前是自包含实现；待依赖图重构后可进一步对齐。
- `ccb-provider-core::default_binding_adapter` 返回 `None`，provider-specific loader 在下游 `ccb-providers`；完整 `resolve_agent_binding` 需要下游注入 resolver。
- 大模块（`ccb-daemon`、`ccb-cli`、`ccb-providers`）改动面广，后续需要分阶段进行，避免单次改动过大。

---

## 7. 推荐下一步

1. **`ccb-agents`**：补齐 `agents/` 与 `rolepacks/` 的模型、runtime binding、role 解析。
2. **`ccb-memory`**：补齐 `memory/` 与 `project_memory/`。
3. 后续按 `ccb-daemon` → `ccb-cli` → `ccb-providers` 顺序推进。

---

## 8. 常用命令

```bash
cd /home/agnitum/ccb/rust

# 单 crate 测试
cargo test -p <crate> -- --test-threads=1

# 全 workspace 测试
cargo test --workspace -- --test-threads=1

# 严格 clippy
cargo clippy --workspace -- -D warnings

# 格式化检查
cargo fmt --check --workspace

# 代码图同步
/root/.local/bin/codegraph sync

# 端到端 smoke（在测试项目目录）
./ccbr start
./ccbr ask kimi "hello"
./ccbr kill
./ccbr shutdown
```

---

## 9. 参考资料

- `AGENTS.md`（项目根目录）：项目结构、开发纪律、停止规则。
- `docs/handoff-rust-migration-phase1.md`：本文档。
- Python 参考：`/home/agnitum/ccb-v7.5.2/lib`
- Rust workspace：`/home/agnitum/ccb/rust/crates`
