# Python→Rust Migration Roadmap

> 整体迁移的计划基准。驱动后续 Trellis 任务创建与优先级决策。source-backed：所有数字/路径来自 2026-06-22 扫描。

## Current state (2026-06-22)

- **workspace stub**: 1218（含 `TODO: align`）。分布：
  - `ccb-providers` 463, `ccb-daemon` 348（合计 67%）
  - `ccb-cli` 173, `ccb-agents` 73, `ccb-memory` 56, `ccb-provider-core` 46
  - `ccb-mailbox` 19, `ccb-completion` 11, 其余 < 10
- **parity matrix**: 26 集群**全部 `partial`**，0 `complete`（`plans/rust-python-test-parity-matrix.md`）。
- **已闭环 Trellis 任务**: `06-20-py2rust-daemon-lifecycle`（launchers+services）、`06-22-fix-ccb-cli-flaky-env-tests`。
- **daemon_runtime 接口完整**（`crates/ccb-cli/src/services/daemon_runtime/` stub=0）。

## ⚠️ Architecture gap（最高风险）

`Phase2Services` trait（`crates/ccb-cli/src/phase2_runtime/handlers_ops.rs`）定义了 17+ service 方法 + 24 handler + `dispatch`（28 命令），但 **0 个 impl**（`grep -rln "impl Phase2Services" crates/` 为空）。

**后果**: `dispatch → handle_xxx → render_xxx` 链断在 service 层。render（29 函数）+ handlers + launchers 零件齐全，但 CLI 无法端到端运行任何命令。继续做 providers/daemon stub 是「堆零件不组装」。

## Roadmap（4 waves, dependency-ordered）

### Wave 1 — 架构解锁（最高优先级，杠杆点）
- **Task**: `py2rust-cli-services-impl`
- **Scope**: 为一个 service struct 实现 `Phase2Services`，调用 ccb-daemon socket client + ccb-providers launchers，接通 dispatch。
- **依赖**: daemon_runtime（✓ stub=0）、socket client（确认状态）
- **验收**: `dispatch` 能驱动真实命令（如 `ccb ps` / `ccb ping`）端到端，render 输出与 Python 一致。
- **文件**: `crates/ccb-cli/src/phase2_runtime/handlers_ops.rs`（trait）、新增 `services_impl.rs` 或类似。

### Wave 2 — 核心 parity
- `py2rust-runtime-launch-orchestration`: `ensure_agent_runtime` 编排（detached fallback / stale / foreign binding / tmux namespace 限制）。承接 `06-20` 任务 Phase B。文件：`crates/ccb-daemon/src/start_runtime/agent_runtime*.rs`。
- `py2rust-completion`: Job store / 完成编排 / heartbeat classifier。文件：`crates/ccb-completion/`, `crates/ccb-jobs/`。

### Wave 3 — stub 削减（量，67% 在此）
- `py2rust-providers-deep`: `ccb-providers` 463 stub，按 provider 子主题拆分（codex/claude/gemini/droid/agy/opencode 的 execution/comm/session）。
- `py2rust-daemon-deep`: `ccb-daemon` 348 stub（dispatcher_runtime / services 深化）。

### Wave 4 — 端到端 + 边缘
- `py2rust-e2e-recovery`: 多 agent 会话持久化/恢复（`test_v2_ccbd_*` 系列，matrix 标为最大缺口）。
- `py2rust-terminal-namespace`: terminal namespace / pane identity（matrix 步骤 2）。

## Conventions（迁移约定，已验证）

- **payload 风格**: `&serde_json::Value` + `.get("k").and_then(|v| v.as_str())`（与 render/handler 一致）。不为 Python 动态 dict 建强类型 struct。
- **依赖约束**: 禁 `chrono`/`regex`/`reqwest`（用 `std::time`、字符串操作、`curl` 子进程）。`libc` 已用于 `kill(2)`（ccb-cli）。
- **camino**: `PathLayout::new` 接受 `impl Into<Utf8PathBuf>`，非 std `PathBuf`。
- **Trellis 流程**: check（质量门）→ Phase 3.4 commit → finish-work（archive+journal）。每任务必走全闭环。
- **GateGuard**: 每次 Edit/Write 呈现 4 点事实（调用文件/无重复/数据读写/用户指令）。
- **测试**: PRD 验收用 `-- --test-threads=1`（规避并行 env 竞争）；env 测试加 `static Mutex` 串行化（见 `source_guard.rs` 模板）。
- **subagent**: 大文件(>200行)委托 `oh-my-claudecode:executor` + `model=sonnet`，**硬性要求构建零 error 才算完成**。

## Done criteria（整体迁移完成）

- [ ] workspace stub → 0（或仅 intentionally-out-of-scope：Python wrapper scripts、Windows/WSL 工具链）。
- [ ] parity matrix 26 集群 → `complete`（或明确 out-of-scope 并记录理由）。
- [ ] `Phase2Services` impl 存在，CLI 端到端可跑核心命令（ps/wait/ask/kill/start/ping）。
- [ ] `cargo test --workspace -- --test-threads=1` 全绿。
- [ ] `cargo clippy --workspace --all-targets` 0 error，`cargo fmt --check` 干净。

## Out of scope（明确排除）

- Python wrapper scripts（`bin/ask`, `bin/autonew`, `bin/ctx-transfer`, `ccb`）→ 由 Rust 原生 binary 替代。
- 真实 provider CLI 实时交互测试 → 保留 Python 参考，Rust 用 mock。
- Windows bootstrap / WSL path utils → 无 Rust 等价（除非后续需求）。
