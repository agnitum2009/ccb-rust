# Handoff — Python↔Rust 功能一致性收尾 → `kimi2.7`

> Prepared for: **kimi2.7** (next implementer)
> Handoff written by: glm5.2
> Base branch: `python-rust/rolepacks-versioning-translation`
> Last commit (glm5.2 session): comms_recover Slice 6 — 12/12 parity + parity matrix update
> Trellis entry task: `.trellis/tasks/06-24-py2rust-consistency-closure/` (assignee: kimi2.7)

---

## 0. TL;DR — 先读这一段

CCB 的 Python→Rust 迁移：Wave 1/2 已完成；Wave 3（原计划「stub 削减」）经 glm5.2 **源码级审计推翻了原计划前提**——713 个 `TODO: align with Python` stub 实测全是**空镜像**（功能已在 canonical 文件实现、测试全绿），不是半成品。真正剩余的是少数**功能一致性缺口**，其中最大的 `comms_recover` 已由 glm5.2 **关闭（12/12）**。你的任务：沿用 glm5.2 的方法，关闭**剩余缺口**（callbacks 子系统首要）。

**入口**：读 `.trellis/tasks/06-24-py2rust-consistency-closure/prd.md`（你的任务规格），再读本文件 §3-§6（现状/缺口/方法/架构）。comms_recover 的关闭过程是你的**可复用范本**。

---

## 1. 背景：迁移现状

| 波次 | 内容 | 状态 |
|---|---|---|
| Wave 1 | CLI `Phase2Services` 架构解锁 | ✅ 完成并提交 |
| Wave 2 | runtime launch / completion / heartbeat / jobs 核心 parity | ✅ 完成并提交 |
| Wave 3 | ccb-providers + ccb-daemon 深度 stub 削减（原计划） | ⚠️ **前提被推翻** → 转为「一致性审计 + 缺口关闭」 |

**Wave 3 原计划为何被推翻**（见 `stub-triage.md` §2）：
- 原以为 368(providers)+345(daemon)=713 个 `TODO: align with Python` stub 是待填充的半成品。
- 实测：**711/713 是 3 行空占位符**（两行 doc + 一行 TODO，零 Rust item），是 Python 文件树的 1:1 结构镜像。
- **功能 parity 早已在 canonical 文件实现**：provider adapters（`providers/{codex,claude,gemini,droid,agy,opencode}.rs`，均 445–2260 行真实代码）、`submission_service.rs`、`services/dispatcher.rs`、`ccb-mailbox`、`materialize_topology.rs`、`reload_apply_service.rs`、`supervision/loop_runner.rs`。
- `cargo check` 全绿；现有 parity 测试 **providers 29 / daemon 26 全过**。
- 证明手段（`stub-triage.md` §3.2）：空模块无法满足任何 `name::item` 引用（会编译错误），故所有 stub 作为「提供 item 的模块」**零真实引用**；高引用计数经路径限定证明是跨 crate 名称碰撞（`ccb_terminal::env` 等）。

---

## 2. glm5.2 本会话已完成的工作（已提交）

按提交顺序（均在 `python-rust/rolepacks-versioning-translation`）：

1. **P0 stub triage**（`stub-triage.md`）：基线 + 方法 + A(删除)/B(实现)/C(延期) 分类 + §8 逐子主题功能验证矩阵。
2. **P1 registry 契约测试**：`test_execution_registry_has_all_wave3_adapters`（`runtime_tests.rs`）。
3. **completion 集群一致性审计**（`research/consistency-audit-completion.md`）：**结论 CONSISTENT**（14 核心 + 12 边界行为全实现，诊断串字节级一致）。
4. **daemon_lifecycle 集群一致性审计**（`research/consistency-audit-daemon-lifecycle.md`）：**结论 PARTIAL**——发现 `comms_recover`/`retry`/`resubmit` 占位 + `callbacks` 部分。
5. **comms_recover 实现计划**（`research/comms-recover-impl-plan.md`）：Option B、依赖图、5 分片。
6. **ccb-mailbox API 扩展**（A1 step1）：`MessageBureauControlService` 加 4 个只读 accessor（`inbound_store`/`lease_store`/`mailbox_kernel`/`attempt_store`），零契约变更。
7. **comms_recover Slice 1-6**：`JobDispatcher::comms_recover` 从占位替换为真实实现——recoverability + noop + stale-running 恢复(cancel+retry+lineage) + reply-delivery 恢复 + 终端重试 + mailbox head 释放(`mailbox_kernel.abandon`) + `comms_recoverability_view`。**12/12 测试通过**（`tests/comms_recover_tests.rs`）。
8. **parity matrix 更新**：daemon_lifecycle 行 comms_recover → complete。

---

## 3. 当前真实状态（一致性全景）

| 集群 | 一致性 | 证据 |
|---|---|---|
| completion | ✅ 一致 | `consistency-audit-completion.md`（诊断串字节级一致） |
| daemon_lifecycle | ⚠️ 部分 | submit/cancel/queue/inbox/ack/trace/reload/namespace/supervision/health/api-models/fault-injection/reply-formatting/client-resolution 一致；**comms_recover 已关闭**；**callbacks 仍部分缺口** |
| providers (6 adapters) | ✅ 一致 | P2-P7 canonical adapters 真实 + 测试绿 |
| providers (广义 infra) | ✅ ~90% 一致 | 次要未映射：claude_registry(cache/events/logs)、opencode(comm_sqlite/session_ensure_pane)、provider_execution(active_resume) |
| terminal_runtime | 🔍 待审 | matrix 标 partial（namespace/state 集成 deferred） |

**关键认知**：
- **绿测试 ≠ 完全一致**。comms_recover 的发现证明：未被测试覆盖的占位方法（`comms_recover`/`retry`/`resubmit` 返回写死 JSON）能让套件全绿却藏着整块未实现。**必须源码级核验**。
- **stub 计数是误导指标**（数的是死镜像，不是缺口）。别再用 stub 数衡量进度。

---

## 4. 你的任务：剩余缺口（按优先级）

### 4.1 callbacks 子系统（首要）
见 `prd.md` §1.1。要点：
- Python `lib/ccbd/services/dispatcher_runtime/callbacks.py`（731 行 / 25 函数）。
- **利好**：`MessageBureauFacade` 已暴露 `record_callback_edge`/`callback_edge_for_parent_job/child_job`/`callback_edge`；`CallbackEdgeStore` API 完整；`CallbackEdgeRecord/State` 已存在。**无需扩 ccb-mailbox API**。
- **风险**：无专门测试文件，~13 个 callback 测试散落多文件 → **开工前先提炼验收清单**（从 `test_v2_message_bureau_dispatcher_integration.py` / `test_v2_ask_service.py` 等提取 callback 行为）。
- continuation-job 提交可**复用 glm5.2 的 submit/retry/lineage**（`JobDispatcher::retry_job`/`record_attempt`/`lineage_store`）。
- 工作量 ≈ comms_recover（~1.0-1.3×），约 6-10 分片。

### 4.2 providers 次要子特性
见 `prd.md` §1.2。逐个核验 Python 是否真有逻辑 + Rust 是否在他处等价提供，再 implement/defer。

### 4.3 terminal_runtime 审计
见 `prd.md` §1.3。先审（同 completion/daemon_lifecycle 方法）。

---

## 5. 方法论（glm5.2 已验证，务必沿用）

### 5.1 一致性审计（先审后写）
对每个子主题：
1. 枚举 Python 测试 / 函数。
2. 映射 Rust 实现 + 测试（codegraph/grep/read）。
3. 逐行为/逐函数判定 **consistent / partial / gap**，产出审计 note（见范本 `consistency-audit-{completion,daemon-lifecycle}.md`）。
4. 审计比实现便宜——**先摸清全部缺口再动手**。

### 5.2 缺口关闭（TDD + 分片）
1. 移植或重表达 Python 测试（先失败）。
2. 实现 1:1 行为 parity。
3. `cargo test -p <crate> -- --test-threads=1` + clippy + fmt 验证。
4. 每子主题/分片**单独提交**。
5. 更新 `plans/rust-python-test-parity-matrix.md`。

### 5.3 comms_recover 是范本
读 `research/comms-recover-impl-plan.md` + `services/dispatcher.rs`（comms_recover/retry_job/release_blocking_head/comms_recoverability_view）+ `tests/comms_recover_tests.rs`。callbacks 的 continuation 提交、edge 状态机可照此模式。

---

## 6. 关键架构知识（避免踩坑）

### 6.1 简化 JobDispatcher vs 真实 MessageBureau
- Python `JobDispatcher` **拥有** message_bureau（jobs/attempts/inbound/lease/kernel 统一）。
- Rust **拆分**：`services/dispatcher.rs::JobDispatcher`（简化：`job_store` + optional `attempt_store`）+ 真实 `MessageBureauFacade`/`ControlService`（在 `app.rs` 单独构造）。
- **glm5.2 的解法**（comms_recover 已用）：给 `JobDispatcher` 加 optional `mailbox: Option<MessageBureauFacade>` + `mailbox_control: Option<ControlService>` + `attempt_store`，用 `lineage_store()` 统一访问（接了 mailbox 用真实 store，否则用简化 store）。**测试 1-7/9-11 不接线（走简化路径不变）；需要真实 mailbox 状态的测试（8/12）才接线。**
- callbacks 可同法：optional 接 MessageBureau（facade 已有 callback API），不接线时走简化路径。

### 6.2 类型转换（daemon ↔ ccb_jobs/ccb_mailbox）
`crates/ccb-daemon/src/adapters/mailbox.rs`：
- `to_mailbox_job_record(daemon JobRecord) -> ccb_jobs::JobRecord`（保留 job_id）
- `to_mailbox_envelope(daemon MessageEnvelope) -> ccb_mailbox MessageEnvelope`
- `to_mailbox_completion_decision`
- submit 经 `facade.record_submission(&mb_env, &[mb_job], ...)` 建真实 mailbox 状态。

### 6.3 ccb-mailbox API 扩展是允许的
- stop-rule 禁止改 **线协议/控制面契约**，但**加 pub 方法/Rust 可见性 OK**（comms_recover 加了 4 个 accessor，零行为变更）。
- 若 callbacks 需要 ControlService 内部状态访问，照 comms_recover accessor 模式加（薄包装既有 pub stores/kernel）。

---

## 7. Stop-rules / 护栏（遇歧义升级而非猜测）

不要触碰：
- ccb-mailbox **线协议/控制面契约**（加 pub 方法 OK）。
- provider hook/settings 注入路径。
- tmux namespace/pane identity 核心逻辑。
- `Phase2Services` / `ExecutionService` trait 契约。
- **stub 镜像文件**（作者要求：功能对齐前不删文件对齐脚手架，见 `stub-triage.md` §6 决策）。

遇歧义：先查 `prd.md` → 再查 `stub-triage.md` → 未覆盖则**升级问作者**，不要猜。

---

## 8. 测试 / 提交约定

- 每子主题有专门测试文件或集成测试目标。**TDD（先失败测试）**。
- 定向测试：`cargo test -p ccb-{providers,daemon} -- --test-threads=1`。
- 每子主题后：`cargo check --workspace` + `cargo clippy -p <crate> --all-targets` + `cargo fmt -- --check`。
- 最终门：`cargo test --workspace -- --test-threads=1` + `cargo clippy --workspace --all-targets` + `cargo fmt --check`。
- 提交格式：`feat(daemon): callbacks <sub> parity` 等；**每子主题一提交**。
- 提交信息尾注：`Co-Authored-By: Claude <noreply@anthropic.com>`（仓库惯例）。

---

## 9. 从哪里开始

1. **读** `.trellis/tasks/06-24-py2rust-consistency-closure/prd.md`（你的任务）。
2. **读** `stub-triage.md` §2/§6/§8（为何 stub 不是缺口 + 决策 + 功能验证）。
3. **读** `research/consistency-audit-daemon-lifecycle.md` + `comms-recover-impl-plan.md`（审计 + 关闭范本）。
4. **callbacks 第一步**：提炼验收清单（从分散的 Python 测试提取 callback 行为），写入 `research/consistency-audit-callbacks.md`，再开工实现。
5. `python3 ./.trellis/scripts/task.py start .trellis/tasks/06-24-py2rust-consistency-closure` 激活任务。

---

## 10. 速查命令

```bash
cd /home/agnitum/ccb
# 一致性审计：stub 真相（别用它衡量进度，只作路线图）
grep -rln 'TODO: align with Python' rust/crates/ccb-{providers,daemon}/src/ | wc -l
# 构建/测试
cd rust
cargo check --workspace
cargo test -p ccb-daemon -- --test-threads=1
cargo test -p ccb-providers -- --test-threads=1
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets
cargo fmt --check
# codegraph（结构问题首选，见 ~/.claude/CLAUDE.md）
```

---

祝顺利。`comms_recover` 12/12 是前例，callbacks 照此推进即可。
