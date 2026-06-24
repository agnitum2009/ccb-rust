# CCB 剩余 Python→Rust parity 完整性迁移

## Background

CCB v7.5.1 正在将 Python 运行时（`lib/`）迁移到 Rust workspace（`rust/crates/`）。
本会话前已闭环 4 个 Trellis 任务：

- `06-24-py2rust-completion-dispatcher-fastpath`
- `06-24-py2rust-daemon-startup-foreground-wait`
- `06-24-py2rust-providers-catalog-health-restore`
- `06-24-py2rust-cli-ask-install-restart`

根据 `plans/rust-python-test-parity-matrix.md` 与 `.trellis/spec/migration-roadmap.md`，仍有 **6 个 partial 领域** 与 **27 个未匹配 Python 测试** 需要补齐。

## Source-backed current state

- Python reference tests (v7.5.2): **314**
- Rust migration tests: **60**
- Parity matrix clusters: **26 个全部 partial**，0 个 complete
- Workspace `TODO: align with Python` stubs: **1218**
  - `ccb-providers` 463
  - `ccb-daemon` 348
  - `ccb-cli` 173
  - `ccb-agents` 73
  - `ccb-memory` 56
  - `ccb-provider-core` 46
- Highest-risk architecture gap: `Phase2Services` trait **0 个实现**；CLI dispatch→handler→render 链路在 service 层断开。

## Scope

将剩余工作拆分为 4 个 dependency-ordered waves，分别创建子任务：

1. **Wave 1 — CLI Phase2Services 架构解锁**：实现一个 concrete `Phase2Services`，让 `dispatch` 能端到端驱动真实命令（ps/ping/wait/ask/kill/start 等）。
2. **Wave 2 — 核心 parity**：runtime launch 编排（detached fallback / stale / foreign binding / tmux namespace 限制）+ completion/heartbeat 编排。
3. **Wave 3 — stub 削减**：`ccb-providers` 463 stub + `ccb-daemon` 348 stub，按 provider/子系统主题拆分。
4. **Wave 4 — 端到端恢复与边缘 parity**：多 agent 会话持久化/恢复（`test_v2_ccbd_*`）、terminal namespace/pane identity、install/update 系列未匹配测试、MCP/delegation、Windows/WSL 工具链（如明确 out-of-scope 需记录）。

## Out of scope（需明确记录）

- 真实 provider CLI 实时交互测试保留 Python 参考，Rust 侧仅 mock。
- Python wrapper scripts（`bin/ask`、`bin/autonew`、`bin/ctx-transfer`、`ccb`）已由 Rust 原生 binary 替代。
- Windows bootstrap / WSL path utils 如无后续需求，保持无 Rust 等价实现。

## Acceptance criteria

- [ ] 4 个子任务均完成 `prd.md` + `design.md` + `implement.md`。
- [ ] 每个 wave 启动前通过 `task.py start` 进入 `in_progress`。
- [ ] 每个 wave 完成后 `cargo test --workspace -- --test-threads=1` 全绿。
- [ ] 每个 wave 完成后更新 `plans/rust-python-test-parity-matrix.md` 中的 cluster 状态与测试映射。
- [ ] 整体迁移完成后 workspace stub 数量趋近于 0（仅保留 intentionally-out-of-scope）。
- [ ] `Phase2Services` 存在具体实现，CLI 端到端可跑核心命令（ps/wait/ask/kill/start/ping）。
- [ ] 所有任务归档后，`/trellis:finish-work` 可干净收尾。

## References

- `plans/rust-python-test-parity-matrix.md`
- `.trellis/spec/migration-roadmap.md`
- `rust/crates/ccb-cli/src/phase2_runtime/handlers_ops.rs`
- `rust/crates/ccb-cli/src/phase2_services.rs`
