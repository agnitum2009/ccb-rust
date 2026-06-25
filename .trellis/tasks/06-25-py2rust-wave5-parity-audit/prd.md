# Wave 5: Python→Rust 1:1 parity 全量审计 + 聚焦

## Goal
项目主线推进：**1:1 用 Rust 替代 Python，确保参考 ccb（Python 源）的原样可运行 ccbr（Rust）**。本任务做全量 parity 审计，产出 gap 清单并聚焦成下一波可执行任务。

## Requirements
- **全量扫**：以 Python `lib/` 各模块（agents/ask_cli/ccbd/cli/completion/fault_injection/heartbeat/jobs/mailbox_kernel/mailbox_runtime/memory/message_bureau/opencode_runtime/pane_registry_runtime/project/project_memory/provider_backends/provider_core/provider_execution/provider_hooks/provider_profiles/provider_runtime/provider_sessions/rolepacks/runtime_env + 根级 .py）为基准，逐个比对 Rust 对应 crate（rust/crates/ccb-*），刷新 `plans/rust-python-test-parity-matrix.md`。
- **每模块判定**：Rust 是否已实现 / 行为是否等价 / 测试是否 parity。标注 gap 类型（missing / partial / behavior-drift / test-missing）+ 证据。
- **聚焦**：按 criticality 排序（核心 runtime 路径优先：daemon lifecycle、provider launch、mailbox/job 路由、cli 命令、heartbeat/recovery），把高优先 gap 拆成 Wave 5 子任务（挂本任务或父任务下）。
- 验收基准：ccbr 的运行时行为与 ccb（Python）一致（同等输入→同等 pane/provider/mailbox/job 行为）。

## Acceptance Criteria
- [ ] `plans/rust-python-test-parity-matrix.md` 刷新为全量现状（每 Python 模块一行：Rust 对应 + 状态 + 证据）
- [ ] gap 清单按 criticality 排序产出（research/parity-gaps.md）
- [ ] Top-N 高优先 gap 已拆成 Wave 5 子任务（有 prd）
- [ ] 审计结论 + 下一步入 journal

## Notes
- 参考血系：ccb-legacy（ccb 命名，与 ccbr 仅命名不同、代码一致）——行为 parity 与命名无关，审计可直接用 HEAD（ccbr-*）。方法与样表见 HANDOFF-KIMI.md。
