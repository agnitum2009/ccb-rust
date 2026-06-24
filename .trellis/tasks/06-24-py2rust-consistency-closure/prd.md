# PRD — Wave 3 后续：Python↔Rust 功能一致性收尾

## 背景

CBB Python→Rust 迁移的 Wave 1（CLI `Phase2Services`）、Wave 2（runtime launch / completion / heartbeat / jobs 核心 parity）已完成并提交于 `python-rust/rolepacks-versioning-translation`。

Wave 3 原计划为「ccb-providers + ccb-daemon 深度 stub 削减」，但 glm5.2 的 P0 源码级审计（见 `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md` §2/§8）推翻了原计划前提：

- **713 个 `TODO: align with Python` stub 实测全部是「1:1 文件对齐空镜像」**（仅 doc 注释，零 Rust item），不是半成品。
- **功能 parity 已在 canonical 文件实现**（provider adapters、`submission_service.rs`、`services/dispatcher.rs`、`ccb-mailbox`、`materialize_topology`、`reload_apply_service`、`supervision/loop_runner`），且测试全绿（providers 29 / daemon 26）。
- 绿测试 ≠ 完全一致：审计发现了真实功能缺口，其中 **`comms_recover` 已由 glm5.2 关闭（12/12，见 `crates/ccb-daemon/tests/comms_recover_tests.rs`）**。

本任务承接**剩余的一致性缺口**，目标为 **Python 源码级功能 1:1 行为 parity**（作者硬性要求）。

## 真实剩余缺口（按优先级）

### 1. callbacks 子系统（首要，daemon_lifecycle）
- Python `lib/ccbd/services/dispatcher_runtime/callbacks.py`（731 行 / 25 函数）：callback-edge 注册、continuation-job 提交、callback-chain 校验、timeout sweep、delegated-terminal 持久化、nested-ask 路由。
- Rust 现状：`submission_service.rs::validate_callback_request`（浅）+ `ccb-mailbox::CallbackEdgeStore`（完整 API：append/list/get_latest/for_child_job/for_parent_job/get_latest_continuation_for_edge）+ `CallbackEdgeRecord/State` + `MessageBureauFacade` 已暴露 `record_callback_edge`/`callback_edge_for_parent_job`/`callback_edge_for_child_job`/`callback_edge`。**无需扩 ccb-mailbox API**。
- 缺：`register_callback_edge`、`callback_child_edge`、`delegated_parent_edge`、`submit_callback_continuation`、`repair_callback_edges`、`sweep_callback_timeouts`、`fail_callback_edge`、`mark_callback_done`、`mark_parent_message_waiting`、`delegated_terminal_job`、`persist_delegated_terminal_job`、`_validate_callback_chain`、`_callback_chain_for_parent`、`_record_callback_failure_notice`、`_submit_continuation_job`、`_active_parent_job`、`_job_for_reply`、`_latest_child_reply`。
- **验收基线模糊**（风险点）：无专门 callbacks 测试文件；~13 个 callback 相关测试散落在 `test_v2_message_bureau_dispatcher_integration.py`、`test_v2_ask_service.py` 等多文件。**开工前必须先提炼明确的行为验收清单**。
- 工作量：与 comms_recover 相当或略大（~1.0-1.3×），约 6-10 分片。continuation-job 提交可复用 glm5.2 已建的 submit/retry/lineage 机制。

### 2. providers 次要未映射子特性（providers 集群，~90% 已一致）
- claude_registry：`registry_runtime/{cache,events,logs_binding,logs_discovery}`（Python 有 `test_claude_registry_cache/events/logs_binding/logs_discovery.py`）；Rust 仅有 session registry（`claude/registry.rs`）。
- opencode：`comm_sqlite`、`session_ensure_pane`、`communicator_state` 等子特性。
- provider_execution：`active_resume` 等。
- 次要子特性，非核心；逐个核验 Python 源码是否真实有逻辑、Rust 是否在他处提供等价实现，再决定 implement/defer。

### 3. terminal_runtime 集群审计（待审）
- parity matrix 标 partial：「namespace/state 集成 deferred to py2rust-daemon」。需源码级核验真实缺口。

## 验收标准

- 每个被判定为「真缺口」的子主题：Python 源码 ↔ Rust 实现逐函数行为比对，Rust 测试（移植或重表达）覆盖 Python 测试的可观察行为，全绿。
- `cargo test -p ccb-daemon -- --test-threads=1` / `cargo test -p ccb-providers -- --test-threads=1` 全绿。
- `cargo clippy --workspace --all-targets` 0 error；`cargo fmt --check` clean。
- `cargo test --workspace -- --test-threads=1` 全绿。
- `plans/rust-python-test-parity-matrix.md` 按子主题更新（complete/partial + 证据）。

## 方法论（glm5.2 已验证，强烈建议沿用）

1. **先审后写**：对每个子主题，枚举 Python 测试/函数 → 映射 Rust 实现+测试 → 判定 consistent / partial / gap。审计比实现便宜，先摸清全部再动手。
2. **缺口关闭走 TDD + 分片提交**：移植/重表达 Python 测试（失败）→ 实现 → 验证 → 每子主题提交。
3. **comms_recover 是可复用范本**：见 `.trellis/tasks/06-24-py2rust-providers-daemon-deep/research/comms-recover-impl-plan.md` + `consistency-audit-daemon-lifecycle.md`。其 `JobDispatcher` 接线模式（optional mailbox/attempt_store + `lineage_store()` 统一访问 + `ccb-mailbox` 只读 accessor）对 callbacks 同样适用。

## 范围外（stop-rule）

- 不改 ccb-mailbox **线协议/控制面契约**（加 pub 方法/Rust 可见性 OK，如 comms_recover 的 accessor）。
- 不改 provider hook/settings 注入路径、tmux namespace/pane identity 核心逻辑、`Phase2Services`/`ExecutionService` trait 契约。
- 不做 live provider CLI / Windows/WSL 集成（保留 Python 参考）。
- stub 镜像文件**保持不动**（作者要求：功能对齐前不删文件对齐脚手架；详见 stub-triage.md §6 决策）。

## 参考资料

- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/HANDOFF-KIMI.md` — **本任务主入口（必读）**。
- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/stub-triage.md` — P0 stub 真相 + 功能验证 §8 + 决策 §6。
- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/research/consistency-audit-{completion,daemon-lifecycle}.md` — 两集群一致性审计。
- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/research/comms-recover-impl-plan.md` — comms_recover 关闭方法（callbacks 范本）。
- `plans/rust-python-test-parity-matrix.md` — 集群映射与状态。
- Python 参考：`lib/ccbd/services/dispatcher_runtime/callbacks.py`、`lib/provider_backends/`、`lib/ccbd/services/project_namespace_runtime/`。
