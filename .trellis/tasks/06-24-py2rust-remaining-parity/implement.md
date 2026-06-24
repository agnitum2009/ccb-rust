# Implement: CCB 剩余 Python→Rust parity 迁移（父任务）

## Phase 1 规划清单

- [x] 创建父任务 `06-24-py2rust-remaining-parity`
- [x] 创建 4 个子任务并关联到父任务
- [x] 编写父任务 `prd.md` + `design.md` + `implement.md`
- [x] 为每个子任务编写 `prd.md` + `implement.md`（跨 wave 设计决策集中在父任务 `design.md`，子任务引用父设计）
- [x] 为父任务和子任务配置 `implement.jsonl` / `check.jsonl`（引用 migration-roadmap、parity matrix、guides）
- [x] 父任务 review gate：wave 拆分与优先级已确认（4-wave dependency-ordered，Wave 1 为架构杠杆点）

## Phase 2 执行顺序

按 dependency 顺序逐个启动子任务。每个子任务独立走 Trellis 完整闭环。

### Step 1: Wave 1 — CLI Phase2Services 架构解锁 ✅

- 状态：已完成并归档（`06-24-py2rust-cli-services-impl` → `.trellis/tasks/archive/2026-06/`）。
- 实现内容：
  - 补全 `DaemonPhase2Services` 中剩余的未实现方法：`export_diagnostic_bundle`、`doctor_storage_summary`、`doctor_summary`、`validate_config_context`。
  - 新增 `crates/ccb-cli/tests/phase2_services_tests.rs`，用 fake daemon socket 覆盖 ps/ping/wait/kill/start/ask/restart/logs/maintenance/reload/doctor-bundle/config-validate 的 phase2 dispatch 端到端路径。
- 验收结果：
  - `cargo test -p ccb-cli -- --test-threads=1` 全绿（含新增 11 个测试）。
  - `cargo test --workspace -- --test-threads=1` 全绿。
  - `cargo clippy --workspace --all-targets` 无 error（仅既有 warning）。
  - `cargo fmt --check` 干净。
- `plans/rust-python-test-parity-matrix.md` 已更新 `cli_entrypoint` 行，记录 `Phase2Services` 实现与测试映射。

### Step 2: Wave 2 — 核心 parity（进行中）

- 任务状态：`06-24-py2rust-core-parity` 已启动，尚未归档。
- 已完成：
  - `ccb-heartbeat/src/classifier.rs` 空 stub 清理：改为 re-export `maintenance.rs` 的公开分类函数。
  - `ccb-jobs/src/store.rs` `JobEventStore::read_since_target` 增加 `record_type != "job_event"` 跳过逻辑；新增回归测试 `event_store_skips_non_job_event_records`。
- 待完成：
  - runtime launch 编排（detached fallback / stale / foreign / namespace 限制）。
  - completion `SessionRotate` selector reset 端到端断言。
  - CLI maintenance `status/tick/schedule/runner` 完整编排。
- 当前验收：
  - `cargo test -p ccb-heartbeat -- --test-threads=1` 全绿。
  - `cargo test -p ccb-jobs -- --test-threads=1` 全绿（新增 1 个测试）。
  - `cargo check --workspace` 通过。

### Step 3: Wave 3 — stub 削减

- 启动任务：`python3 ./.trellis/scripts/task.py start 06-24-py2rust-providers-daemon-deep`
- 实现目标：
  - 按 provider 子主题拆分并削减 `ccb-providers` 463 stub。
  - 按子主题拆分并削减 `ccb-daemon` 348 stub。
- 验收：
  - 每个 provider/daemon 子主题有独立测试并通过。
  - `cargo test --workspace -- --test-threads=1` 全绿。
- 完成后更新 parity matrix 中 `providers`、`daemon_lifecycle` 状态。

### Step 4: Wave 4 — 端到端恢复与边缘 parity

- 启动任务：`python3 ./.trellis/scripts/task.py start 06-24-py2rust-e2e-terminal-edge`
- 实现目标：
  - 多 agent 会话持久化/恢复（`test_v2_ccbd_*`）。
  - terminal namespace / pane identity 集成。
  - install/update、MCP delegation 等未匹配 Python 测试；明确 out-of-scope 并记录。
- 验收：
  - `cargo test --workspace -- --test-threads=1` 全绿。
  - parity matrix 更新为 26 集群全部 `complete` 或明确 out-of-scope。

## Phase 3 收尾

- [ ] 所有子任务归档：`python3 ./.trellis/scripts/task.py archive <task-dir>`
- [ ] 父任务归档
- [ ] 运行 `cargo test --workspace -- --test-threads=1`
- [ ] 运行 `cargo clippy --workspace --all-targets` 并清零 error
- [ ] 运行 `cargo fmt --check`
- [ ] 更新 `.trellis/spec/migration-roadmap.md` 的 Current state
- [ ] 调用 `/trellis:finish-work` 或等效 finish-work skill

## Validation commands

```bash
cd /home/agnitum/ccb/rust
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --check
```

## Context files

- `.trellis/spec/migration-roadmap.md`
- `plans/rust-python-test-parity-matrix.md`
- `.trellis/spec/guides/cross-layer-thinking-guide.md`
- `.trellis/spec/guides/code-reuse-thinking-guide.md`
