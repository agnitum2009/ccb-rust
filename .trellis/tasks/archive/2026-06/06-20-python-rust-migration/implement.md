# Python → Rust 迁移执行计划

## 1. 项目结构

- **Parent task**：`06-20-python-rust-migration`（本任务）
  - 负责整体范围、验收标准、跨模块协调、parity matrix 维护。
- **Child tasks**：每个 crate 或紧密相关的一组 crate 一个 child task。
  - 每个 child task 有自己的 `prd.md`、`design.md`（如需要）、`implement.md`。
  - child task 完成后 archive，并更新本 parent task 的进度。

## 2. 子任务拆分

| # | Child Task | 目标 Crate(s) | 优先级 | 依赖 |
|---|-----------|--------------|--------|------|
| 1 | 核心类型与存储迁移 | `ccb-types`, `ccb-storage`, `ccb-storage-classification`, `ccb-ui-text` | P0 | 无 |
| 2 | Project 与 Workspace 迁移 | `ccb-project`, `ccb-workspace` | P0 | #1 |
| 3 | Heartbeat 与 Jobs 迁移 | `ccb-heartbeat`, `ccb-jobs` | P0 | #1 |
| 4 | Mailbox 与 Message Bureau 迁移 | `ccb-mailbox`, `ccb-message-bureau` | P1 | #1, #2 |
| 5 | Terminal 与 Pane Registry 迁移 | `ccb-terminal`, `ccb-pane-registry` | P1 | #1 |
| 6 | Daemon 控制平面迁移 | `ccb-daemon` | P1 | #4, #5 |
| 7 | CLI 迁移 | `ccb-cli` | P1 | #6 |
| 8 | Completion 与 Agents 迁移 | `ccb-completion`, `ccb-agents` | P2 | #6, #7 |
| 9 | Memory 迁移 | `ccb-memory` | P2 | #2, #8 |
| 10 | Providers 核心与后端迁移 | `ccb-provider-core`, `ccb-provider-hooks`, `ccb-provider-profiles`, `ccb-provider-sessions`, `ccb-providers` | P2 | #6, #8 |
| 11 | 测试对等与 Python 退役 | 全 workspace | P2 | #1-#10 |

## 3. 执行阶段

### Phase 0：基线建立（本 task 已完成规划后启动）

- [ ] 创建所有 child tasks（见第 4 节）。
- [ ] 对每个 P0/P1 crate，运行一次现状审计：列出已实现模块、TODO、测试覆盖缺口。
- [ ] 在 `plans/rust-python-test-parity-matrix.md` 中补充缺失的集群映射。
- [ ] 建立 git tag `python-rust-migration-baseline`。

### Phase 1：核心层（P0）

- [ ] 完成 child task #1：核心类型与存储。
- [ ] 完成 child task #2：Project 与 Workspace。
- [ ] 完成 child task #3：Heartbeat 与 Jobs。
- [ ] 验证：`cargo test -p ccb-types -p ccb-storage -p ccb-storage-classification -p ccb-ui-text -p ccb-project -p ccb-workspace -p ccb-heartbeat -p ccb-jobs -- --test-threads=1`。
- [ ] 回滚点：tag `phase1-core-complete`。

### Phase 2：通信与终端（P1）

- [ ] 完成 child task #4：Mailbox 与 Message Bureau。
- [ ] 完成 child task #5：Terminal 与 Pane Registry。
- [ ] 验证：`cargo test -p ccb-mailbox -p ccb-message-bureau -p ccb-terminal -p ccb-pane-registry -- --test-threads=1`。
- [ ] 回滚点：tag `phase2-comm-terminal-complete`。

### Phase 3：控制平面与 CLI（P1）

- [ ] 完成 child task #6：Daemon 控制平面。
- [ ] 完成 child task #7：CLI。
- [ ] 验证：
  - `cargo test -p ccb-daemon -p ccb-cli -- --test-threads=1`
  - 手动端到端：`ccb start`、`ccb status`、`ccb kill`
- [ ] 回滚点：tag `phase3-daemon-cli-complete`。

### Phase 4：完成、Agents、Memory、Providers（P2）

- [ ] 完成 child task #8：Completion 与 Agents。
- [ ] 完成 child task #9：Memory。
- [ ] 完成 child task #10：Providers 核心与后端。
- [ ] 验证：`cargo test --workspace -- --test-threads=1`。
- [ ] 回滚点：tag `phase4-providers-complete`。

### Phase 5：测试对等与 Python 退役（P2）

- [ ] 完成 child task #11：测试对等与 Python 退役。
- [ ] 更新 `plans/rust-python-test-parity-matrix.md`，所有 partial 集群标记为 complete 或 deferred。
- [ ] 确认 release tarball 不再包含 `lib/` Python 实现（允许的 hook 脚本除外）。
- [ ] 运行 release build：`python scripts/build_linux_release.py`。
- [ ] 验证安装后的二进制可完整运行一次多 agent 会话。
- [ ] 回滚点：tag `phase5-release-complete`。

## 4. 创建 Child Tasks 的命令

```bash
cd /home/agnitum/ccb

python3 ./.trellis/scripts/task.py create "核心类型与存储迁移" --slug py2rust-core --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Project 与 Workspace 迁移" --slug py2rust-project --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Heartbeat 与 Jobs 迁移" --slug py2rust-jobs --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Mailbox 与 Message Bureau 迁移" --slug py2rust-mailbox --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Terminal 与 Pane Registry 迁移" --slug py2rust-terminal --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Daemon 控制平面迁移" --slug py2rust-daemon --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "CLI 迁移" --slug py2rust-cli --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Completion 与 Agents 迁移" --slug py2rust-agents --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Memory 迁移" --slug py2rust-memory --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "Providers 核心与后端迁移" --slug py2rust-providers --parent 06-20-python-rust-migration
python3 ./.trellis/scripts/task.py create "测试对等与 Python 退役" --slug py2rust-parity --parent 06-20-python-rust-migration
```

## 5. 验证命令

每个 child task 完成后必须运行：

```bash
# 格式与 lint
cargo fmt --check
cargo clippy --workspace -- -D warnings

# 目标 crate 测试
cargo test -p <crate> -- --test-threads=1

# 全 workspace 测试（Phase 3 之后每次必须运行）
cargo test --workspace -- --test-threads=1
```

## 6. 审查门

- **Gate 1（Phase 1 结束）**：核心 crate 测试全部通过，parity matrix 更新。
- **Gate 2（Phase 3 结束）**：CLI + daemon 可完成一次完整的 `ccb start` / `ccb kill` 周期。
- **Gate 3（Phase 5 结束）**：release build 成功，安装后的二进制通过端到端冒烟测试，PR 合并。

## 7. 回滚点

每个 Phase 结束后打 tag：

```bash
git tag -a phase1-core-complete -m "Phase 1: core crates migrated"
git tag -a phase2-comm-terminal-complete -m "Phase 2: mailbox, message-bureau, terminal migrated"
git tag -a phase3-daemon-cli-complete -m "Phase 3: daemon and CLI migrated"
git tag -a phase4-providers-complete -m "Phase 4: completion, agents, memory, providers migrated"
git tag -a phase5-release-complete -m "Phase 5: parity and Python retirement"
```

若某 Phase 引入不可接受的回归，回滚到上一 Phase tag 并修复问题后重新推进。

## 8. 首次执行动作

1. 创建 child task #1：核心类型与存储迁移。
2. 启动该 child task 并进入其 Phase 1 规划或执行。
3. 本 parent task 保持 `in_progress`，用于跟踪总进度和协调跨模块问题。
