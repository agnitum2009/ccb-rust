# Wave 5: parity sweep — P1/P2 gap 批量闭环 + 矩阵扫雷

## Goal
批量闭环 py→rust parity 中**高体积、机械重复**的剩余项：Wave 5 审计里 6 个未建任务的 P1/P2 gap + `plans/rust-python-test-parity-matrix.md` 的 partial/gap/missing 项。统一用"每项闭环模板"逐项推进。服务北极星：1:1 用 Rust 替代 Python，ccbr 原样可运行如 ccb。

## Requirements
- 用统一模板逐项闭环下列 **6 个 P1/P2 gap**（来自 research/wave5-gap-analysis.md，3 个 P0 已各自单独立任务，不在此 sweep）：
  - **P1** midrun-cancel（Python `lib/ccbd/services/dispatcher_runtime/cancel_runtime.py` → `ccbr-daemon/src/services/dispatcher.rs`）
  - **P1** provider-timeout（`lib/ccbd/services/job_heartbeat_runtime/` → `ccbr-daemon` heartbeat/provider polling）
  - **P1** auth-error-surface（`lib/provider_backends/codex/auth_runtime.py` → `ccbr-provider-profiles/src/codex_home_config.rs`）
  - **P1** rich-ping（`lib/ccbd/handlers/ping_runtime/` → `ccbr-daemon/src/handlers/ping.rs`）
  - **P2** codex-delivery-guard（`test_stability_regressions.py::test_codex_delivery_guard_*` → `ccbr-providers/src/providers/codex.rs`）
  - **P2** start-foreground-service（`lib/cli/start_foreground_runtime/` + `lib/ccbd/start_flow_runtime/service.py` → `ccbr-cli/src/start_foreground.rs` + `ccbr-daemon/src/start_flow/service.rs`）
- **矩阵扫雷**：对 `plans/rust-python-test-parity-matrix.md` 的 6 partial + 6 gap + 2 missing 项，按同一模板闭环；与上述 6 gap 去重（同一项只闭一次）。
- 每项必须留下证据：Rust 符号/测试名 + 行为等价说明 + 矩阵状态更新。

## Acceptance Criteria
- [ ] 6 个 P1/P2 gap 全部按模板闭环（实现+测试+证据）
- [ ] 矩阵 partial/gap/missing 项清零（或显式标注 out-of-scope + 理由）
- [ ] `cargo test --workspace` 全绿
- [ ] 改了 rust/ 的项同步 ccb-legacy（反向重命名）+ 产品仓 ff-push
- [ ] 每项 live/集成验证后跑 `scripts/ccbr-test-cleanup.sh` 回收

## Notes
- 模板见 HANDOFF-KIMI.md。3 个 P0（daemon-restore-jobs/mount-ownership-persist/supervision-loop）不在本 sweep，各自独立任务。
