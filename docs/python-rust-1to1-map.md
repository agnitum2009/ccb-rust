# Python `lib/` → Rust 1:1 目录/模块/文件镜像规划

**日期**：2026-06-14
**原则**：目录对目录、模块对模块、文件对文件，严格 1:1。每个 Python `.py` 有同名 `.rs`（snake_case 直译），目录结构对应。目的：Python 迭代时能文件级 diff 到 Rust 并适配。
**状态**：规划文档（选项 B 交付物）。执行按 §6 优先级。

---

## 1. 命名约定

| Python | Rust | 规则 |
|--------|------|------|
| `reload_plan.py` | `reload_plan.rs` | snake_case 直译，同名 |
| `lib/ccbd/reload_plan.py` | `ccb-daemon/src/reload_plan.rs` | flat 包 → flat 文件 |
| `lib/ccbd/handlers/start.py` | `ccb-daemon/src/handlers/start.rs` | 子目录对应 |
| `__init__.py` | `mod.rs` | 包初始化 → 模块声明 |
| `SomeClass` (PascalCase) | `struct SomeClass` | 类 → 结构体 |
| `snake_func` | `fn snake_func` | 同名 |

**目录镜像规则**：Python 包 `lib/<pkg>/` 是 flat（或含子目录）→ Rust 对应 crate 的 `src/` 下同结构。Python `lib/ccbd/` 的 flat `reload_*.py` → `ccb-daemon/src/reload_*.rs`（**不是**塞进 `reload/` 子目录——除非 Python 也是子目录）。

---

## 2. 包 → Crate 映射（全量 34 包 + 3 顶层文件）

| Python 包 | Rust crate | 合并说明 |
|-----------|-----------|---------|
| `agents` | `ccb-agents` | + rolepacks |
| `rolepacks` | `ccb-agents` | 合入 agents |
| `ccbd` | `ccb-daemon` | + fault_injection |
| `fault_injection` | `ccb-daemon` | 合入 daemon |
| `cli` | `ccb-cli` | + ask_cli |
| `ask_cli` | `ccb-cli` | 合入 cli |
| `memory` | `ccb-memory` | + project_memory |
| `project_memory` | `ccb-memory` | 合入 memory |
| `provider_backends` | `ccb-providers` | + execution + runtime + opencode |
| `provider_execution` | `ccb-providers` | 合入 providers |
| `provider_runtime` | `ccb-providers` | 合入 providers |
| `opencode_runtime` | `ccb-providers` | 合入 providers |
| `provider_core` | `ccb-provider-core` | |
| `provider_hooks` | `ccb-provider-hooks` | |
| `provider_sessions` | `ccb-provider-sessions` | |
| `provider_profiles` | `ccb-provider-profiles` | |
| `terminal_runtime` | `ccb-terminal` | |
| `message_bureau` | `ccb-message-bureau` | |
| `mailbox_kernel` | `ccb-mailbox` | + mailbox_runtime |
| `mailbox_runtime` | `ccb-mailbox` | 合入 mailbox |
| `completion` | `ccb-completion` | |
| `storage` | `ccb-storage` | |
| `storage_classification` | `ccb-storage-classification` | |
| `pane_registry_runtime` | `ccb-pane-registry` | |
| `workspace` | `ccb-workspace` | |
| `runtime_pid_cleanup` | `ccb-runtime-pid-cleanup` | |
| `project` | `ccb-project` | |
| `maintenance_heartbeat` | `ccb-heartbeat` | 合入 heartbeat |
| `heartbeat` | `ccb-heartbeat` | |
| `stdio_runtime` | `ccb-stdio-runtime` | |
| `runtime_env` | `ccb-runtime-env` (+ `ccb-types` 重导出) | |
| `ui_text` | `ccb-ui-text` | |
| `jobs` | `ccb-jobs` | |
| `provider_model_shortcuts.py` | `ccb-provider-core/src/model_shortcuts.rs` | 顶层 .py → crate 内同名 |
| `release_artifacts.py` | `ccb-cli/src/release_artifacts.rs` | |
| `role_aliases.py` | `ccb-agents/src/role_aliases.rs` | |

---

## 3. 逐 Crate 对齐状态（文件数对照）

| Crate | Python 包 | Py 文件 | Rs 文件 | 状态 | 对齐债务 |
|-------|----------|--------:|--------:|------|---------|
| **ccb-providers** | backends+exec+runtime+opencode | 506 | 75 | 🔴 CONSOLIDATED | 最大：506→75，重度压缩 |
| **ccb-cli** | cli+ask_cli | 164 | 14 | 🔴 CONSOLIDATED | 164→14，dispatcher 合并 |
| **ccb-daemon** | ccbd+fault_injection | 330 | 79 | 🔴 CONSOLIDATED | 330→79，reload 子系统重灾区 |
| **ccb-agents** | agents+rolepacks | 50 | 13 | 🟠 CONSOLIDATED | 50→13 |
| **ccb-memory** | memory+project_memory | 40 | 14 | 🟠 CONSOLIDATED | 40→14 |
| **ccb-terminal** | terminal_runtime | 37 | 17 | 🟠 CONSOLIDATED | 37→17 |
| **ccb-message-bureau** | message_bureau | 30 | 1 | 🔴 CONSOLIDATED | 30→1，几乎全埋 |
| **ccb-provider-hooks** | provider_hooks | 13 | 5 | 🟠 CONSOLIDATED | 13→5 |
| **ccb-provider-core** | provider_core | 43 | 26 | 🟡 CLOSE | 43→26，轻度 |
| **ccb-provider-sessions** | provider_sessions | 7 | 4 | 🟡 CLOSE | 7→4 |
| ccb-completion | completion | 23 | 23 | 🟢 ALIGNED | |
| ccb-heartbeat | heartbeat+maint | 8 | 8 | 🟢 ALIGNED | |
| ccb-mailbox | mailbox_kernel+runtime | 14 | 13 | 🟢 ALIGNED | |
| ccb-pane-registry | pane_registry_runtime | 10 | 12 | 🟢 ALIGNED | |
| ccb-project | project | 5 | 7 | 🟢 ALIGNED | |
| ccb-provider-profiles | provider_profiles | 4 | 5 | 🟢 ALIGNED | |
| ccb-runtime-env | runtime_env | 2 | 4 | 🟢 ALIGNED | |
| ccb-runtime-pid-cleanup | runtime_pid_cleanup | 5 | 6 | 🟢 ALIGNED | |
| ccb-stdio-runtime | stdio_runtime | 2 | 3 | 🟢 ALIGNED | |
| ccb-storage | storage | 11 | 13 | 🟢 ALIGNED | |
| ccb-storage-classification | storage_classification | 3 | 2 | 🟢 ALIGNED | |
| ccb-ui-text | ui_text | 1 | 3 | 🟢 ALIGNED | |
| ccb-workspace | workspace | 8 | 9 | 🟢 ALIGNED | |
| ccb-jobs | jobs | 1 | 3 | 🟢 ALIGNED | |

**汇总**：14 个 crate 已 ALIGNED；8 个 CONSOLIDATED（对齐债务）；2 个 CLOSE。

---

## 4. CONSOLIDATED 重灾区的拆分目标

### 4.1 ccb-daemon（330 py → 79 rs）— reload 子系统
Python `lib/ccbd/` 的 reload 子系统（flat `reload_*.py`）→ Rust `ccb-daemon/src/reload_*.rs`（flat，非子目录）：

| Python 文件 | Rust 目标 | 现状 |
|---|---|---|
| `reload_plan.py` | `reload_plan.rs` | 埋在 `reload/plan.rs` |
| `reload_patch.py` | `reload_patch.rs` | 同上 |
| `reload_additive_agents.py` | `reload_additive_agents.rs` | 同上 |
| `reload_patch_additive_agents.py` | `reload_patch_additive_agents.rs` | 同上 |
| `reload_patch_remove_agents.py` | `reload_patch_remove_agents.rs` | 同上 |
| `reload_apply.py` / `reload_apply_service.py` / `reload_apply_namespace.py` / `reload_apply_results.py` / `reload_apply_plan.py` / `reload_apply_stages.py` / `reload_apply_graph.py` / `reload_apply_models.py` / `reload_apply_publish.py` / `reload_apply_runtime.py` | 各自同名 `.rs` | 埋在 `reload/transaction.rs` |
| `reload_transaction*.py`（6 个） | 各自同名 `.rs` | 同上 |
| `reload_runtime_mount*.py`（8 个） | 各自同名 `.rs` | ❌ 完全缺失 |
| `reload_append_layout.py` | `reload_append_layout.rs` | ❌ 缺失 |
| `reload_drain.py` / `reload_handoff.py` / `reload_transaction_context.py` 等 | 各自同名 | ❌ 缺失 |
| `start_preparation.py` | `start_preparation.rs` | ❌ 缺失（配置驱动 agent 准备） |

### 4.2 ccb-providers（506 py → 75 rs）
Python `provider_backends/`(530) + `provider_execution/`(51) + `provider_runtime/`(5) + `opencode_runtime/`(22) → `ccb-providers/src/`。当前按 provider 文件（claude.rs/codex.rs/...）组织，但 Python 按 backend + execution + runtime 分层。需重排为 Python 的分层结构。

### 4.3 ccb-cli（164 py → 14 rs）
Python `cli/`(185) 按 `services/`、`roles_runtime/`、`management_runtime/` 等子目录细分 → Rust `ccb-cli/src/` 只有 14 个 flat 文件（commands.rs/entry.rs/parser.rs）。需按 Python 子目录重排。

### 4.4 ccb-message-bureau（30 py → 1 rs）
Python `message_bureau/`(34) 30 个文件 → Rust 仅 1 个文件。几乎全部需拆分。

---

## 5. 目录镜像目标结构（示例：ccbd）

```
Python lib/ccbd/                    Rust ccb-daemon/src/
├── app.py                          ├── app.rs
├── main.py                         ├── main.rs
├── models.py                       ├── models.rs
├── handlers/                       ├── handlers/
│   ├── start.py                    │   ├── start.rs
│   ├── submit.py                   │   ├── submit.rs
│   └── ...                         │   └── ...
├── services/                       ├── services/
│   └── ...                         │   └── ...
├── reload_plan.py                  ├── reload_plan.rs      ← flat（非 reload/ 子目录）
├── reload_patch.py                 ├── reload_patch.rs
├── reload_apply_service.py         ├── reload_apply_service.rs
├── start_preparation.py            ├── start_preparation.rs
├── reload_runtime_mount_state.py   ├── reload_runtime_mount_state.rs
└── ...                             └── ...
```

**关键**：Python `lib/ccbd/reload_*.py` 是 flat（不在 reload/ 子目录）→ Rust 必须 flat 在 `ccb-daemon/src/`，**不能**用 `reload/` 子目录（除非 Python 也是）。

---

## 6. 执行优先级（按对齐债务 + 适配频率）

| 优先级 | Crate | 债务 | 理由 |
|--------|-------|------|------|
| P0 | ccb-daemon (reload 子系统) | 330→79 | Python 迭代最频繁（reload/start/topology）；当前结构让适配不可能 |
| P0 | ccb-providers | 506→75 | 最大债务；provider 迭代频繁 |
| P1 | ccb-cli | 164→14 | 用户命令迭代频繁 |
| P1 | ccb-message-bureau | 30→1 | 几乎全埋，需拆分 |
| P2 | ccb-agents | 50→13 | rolepacks 迭代 |
| P2 | ccb-memory | 40→14 | |
| P2 | ccb-terminal | 37→17 | |
| P2 | ccb-provider-hooks | 13→5 | |
| P3 | ccb-provider-core | 43→26 | 轻度 |
| P3 | ccb-provider-sessions | 7→4 | 轻度 |
| — | 14 个 ALIGNED crate | — | 已对齐，无需动 |

---

## 7. 执行规则（每个 crate 的拆分流程）

1. **建文件清单**：列出 Python 包的全部 `.py`（去 `__init__`）→ 目标 `.rs` 同名清单。
2. **逐文件映射**：每个 Python `.py` 的公开函数/类 → 对应 Rust `.rs` 的 pub fn/struct。存在的搬运，缺失的标记 `// TODO: translate from <pyfile>`。
3. **纯重构优先**：先搬运现有代码到正确文件名（保行为），再补缺失文件。
4. **保外部 API**：`mod.rs` 重导出，外部调用方不受影响。
5. **验证**：每拆一个文件 `cargo build -p <crate>` + `cargo test -p <crate> -- --test-threads=1`。
6. **对照测试**：每个新文件配 Python 行为基准的单元测试。

---

## 8. 验收标准

- [ ] 每个 CONSOLIDATED crate 的文件数 ≥ Python 文件数 × 0.8（接近 1:1）。
- [ ] 每个 Python `.py` 能在对应 Rust crate 找到同名 `.rs`（或明确标记缺失待补）。
- [ ] `cargo test --workspace -- --test-threads=1` 持续通过。
- [ ] `cargo clippy --workspace --all-targets` + `cargo fmt --check` 持续干净。
- [ ] 目录结构：Python 子目录 ↔ Rust 子目录 1:1。

---

## 附录：当前 14 个 ALIGNED crate（无需重构）

ccb-completion, ccb-heartbeat, ccb-jobs, ccb-mailbox, ccb-pane-registry,
ccb-project, ccb-provider-profiles, ccb-runtime-env, ccb-runtime-pid-cleanup,
ccb-stdio-runtime, ccb-storage, ccb-storage-classification, ccb-ui-text,
ccb-workspace

这 14 个 crate 的 Rust 文件数 ≥ Python 文件数，结构基本对齐，仅需抽查文件名是否 1:1。
