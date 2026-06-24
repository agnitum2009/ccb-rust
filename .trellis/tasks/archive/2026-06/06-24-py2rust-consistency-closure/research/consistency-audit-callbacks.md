# callbacks 子系统一致性审计

> Scope: Python `lib/ccbd/services/dispatcher_runtime/callbacks.py` ↔ Rust `ccb-daemon::services::dispatcher` + `ccb-mailbox`
> Auditor: kimi2.7
> Date: 2026-06-24

## 1. 总体结论

**状态：存在真实功能缺口（PARTIAL / GAPS）**

Rust 侧具备底层数据能力（`CallbackEdgeStore` 完整、`MessageBureauFacade` 已暴露读写 callback edge 的方法、`record_notice` / `set_message_state` / `record_retry_attempt` 已存在），但 `JobDispatcher` 完全没有实现 callback 路由、注册、校验、continuation 提交、repair、sweep、timeout、delegated terminal 等高层行为。

| 类别 | 数量 |
|---|---|
| Python 公开函数 | 18 个（`__all__`）+ 17 个内部辅助 |
| 相关 Python 测试 | 13 个（集中在 `test_v2_message_bureau_dispatcher_integration.py`） |
| 已一致 | `record_callback_edge` / `callback_edge_for_*` / `CallbackEdgeRecord` 模型 |
| 真实缺口 | 15+ 个函数需移植到 Rust dispatcher |

## 2. Python 公开 API 与 Rust 映射

| Python 函数 | 用途 | Rust 现状 | 缺口级别 |
|---|---|---|---|
| `request_callback_route` | 判断 `route_options.mode == 'callback'` | 不存在 | P1 |
| `validate_nested_ask_request` | 阻止 active parent 下发普通 ask（非 callback 且非 silent） | 不存在 | P1 |
| `register_callback_edge` | 在 submit callback child 时创建 edge 并记录 event | 不存在 | P0 |
| `validate_callback_request` | 校验 single target、active parent、无重复 edge、depth/cycle | 不存在 | P0 |
| `delegated_parent_edge` | 查询 job 是否为 callback parent | 不存在 | P1 |
| `callback_child_edge` | 查询 job/message 是否为 callback child | 不存在 | P1 |
| `submit_callback_continuation` | child 完成时创建 continuation job | 不存在 | P0 |
| `repair_callback_edges` | tick 时修复崩溃后未提交 continuation 的 edge | 不存在 | P1 |
| `sweep_callback_timeouts` | tick 时清理超时 edge | 不存在 | P1 |
| `fail_callback_edge` | 将 edge 标记为 FAILED/TIMED_OUT 并通知 original caller | 不存在，但可复用 `record_notice` | P1 |
| `mark_callback_done` | continuation 完成时把 edge 标 DONE | 不存在 | P1 |
| `mark_parent_message_waiting` | child 完成后把 parent message 置 RUNNING | 不存在，可复用 `set_message_state` | P1 |
| `delegated_terminal_job` | 构造带 delegated 标记的 terminal decision | 不存在 | P1 |
| `persist_delegated_terminal_job` | 持久化 delegated terminal job 并记录 event | 不存在 | P1 |
| `_submit_continuation_job` | 实际入队 continuation job | 不存在 | P0 |
| `_existing_continuation_job` | 查找某 edge 已存在的 continuation job | 不存在 | P1 |
| `_latest_child_reply` | 从 reply_store 找 child 最新 reply | 不存在，需加 mailbox accessor | P1 |
| `_job_for_reply` | 通过 reply.attempt_id → attempt → job | 不存在 | P1 |
| `_active_parent_job` | 根据 actor 找当前 active job | 可复用 `DispatcherState::active_job` + `get` | P1 |
| `_message_for_job` | 通过 job 找 message | 不存在，需加 mailbox accessor | P1 |
| `_validate_callback_chain` / `_callback_chain_for_parent` | depth/cycle 校验 | 不存在 | P1 |
| `_record_callback_failure_notice` | callback 失败时给 original caller 发 notice | 可复用 `record_notice` | P1 |
| `_callback_timeout_at` / `_callback_edge_expired` / `_callback_timeout_s` / `_max_callback_depth` | timeout/depth 配置 | 不存在，需加 dispatcher 配置字段 | P1 |
| `_continuation_request` / `_continuation_body` | 构造 continuation message body | 不存在 | P0 |
| `_decision_from_reply` | 从 reply + child terminal_decision 构造 CompletionDecision | 不存在 | P1 |
| `_callback_body_summary` / `_reply_summary` / `_strip_ccb_guidance` | body 摘要/artifact 摘要 | 不存在，可复用 `ccb_storage::text_artifacts` | P2 |

## 3. 测试驱动的验收清单

源文件：`test/test_v2_message_bureau_dispatcher_integration.py`

| # | 测试名 | 关键断言 | 需实现的 Rust 行为 |
|---|---|---|---|
| 1 | `test_dispatcher_callback_routes_child_result_as_parent_continuation` | callback child → edge；parent complete → delegated terminal；child complete → continuation submitted；continuation complete → edge DONE；parent message COMPLETED；root watch reply == continuation reply | submit 注册 edge；complete parent 标记 delegated；complete child 提交 continuation；complete continuation 标记 edge done |
| 2 | `test_dispatcher_callback_continuation_uses_artifact_for_large_child_reply` | 子回复 >4KiB 时 continuation body ≤4KiB 且包含 artifact 路径；`reply_artifact` 非空 | `_reply_summary` 支持 artifact；`maybe_spill_text` 写 artifact |
| 3 | `test_dispatcher_callback_continuation_uses_forced_artifact_for_short_child_reply` | `route_options.artifact_reply=True` 时强制 spill 到 artifact | 识别 `artifact_reply` flag |
| 4 | `test_dispatcher_callback_rejects_without_active_parent` | 无 active parent 时 submit callback 报错 | `validate_callback_request` 检查 active parent |
| 5 | `test_dispatcher_rejects_plain_nested_ask_from_active_parent` | 普通 ask 从 active parent 发出时报错 | `validate_nested_ask_request` |
| 6 | `test_dispatcher_allows_silent_nested_ask_from_active_parent` | `silence_on_success=True` 时允许 | 同上 |
| 7 | `test_dispatcher_callback_child_failure_still_continues_parent` | child failed 也能产生 continuation，body 包含 `Child status: failed` | `_decision_from_reply` 处理失败；continuation body 包含 status |
| 8 | `test_dispatcher_callback_chain_waits_for_nested_child_message` | 多层 callback chain：外层 edge 在里层完成前保持 PENDING | chain 等待逻辑 |
| 9 | `test_dispatcher_callback_repair_submits_missing_continuation_once` | 模拟崩溃后 edge 回退到 CHILD_COMPLETED，tick 修复一次且幂等 | `repair_callback_edges` + `tick` 调用 |
| 10 | `test_dispatcher_callback_repair_uses_persisted_child_reply_from_pending_edge` | edge 仍为 PENDING 但 child reply 已存在，tick 也能修复 | repair 从 pending edge 提取 reply |
| 11 | `test_dispatcher_callback_repair_reuses_existing_continuation_job` | 修复时若 continuation job 已存在则复用 | `_existing_continuation_job` |
| 12 | `test_dispatcher_callback_timeout_fails_parent_message_and_notifies_original_caller` | timeout 后 edge TIMED_OUT，parent message FAILED，reply_store 有 FAILED notice | `sweep_callback_timeouts` + `tick` 调用 |
| 13 | `test_dispatcher_callback_rejects_depth_limit` | depth 超过 `max_callback_depth` 报错 | `_validate_callback_chain` depth 检查 |
| 14 | `test_dispatcher_callback_rejects_actor_cycle` | callback actor 成环报错 | `_validate_callback_chain` cycle 检查 |
| 15 | `test_dispatcher_callback_continuation_submit_failure_marks_edge_failed` | continuation submit 失败（如 agent 被移除）时 edge FAILED 并通知 caller | `fail_callback_edge` + 错误处理 |

## 4. 架构依赖与可复用点

### 4.1 已存在的 Rust API（无需新增）

- `ccb_mailbox::bureau::MessageBureauFacade`
  - `record_callback_edge(&CallbackEdgeRecord)`
  - `callback_edge_for_child_job` / `callback_edge_for_child_message` / `callback_edge_for_parent_job` / `callback_edge`
  - `record_notice`
  - `set_message_state`
  - `record_retry_attempt`
- `ccb_mailbox::stores::CallbackEdgeStore`
  - `append`, `get_latest*`, `list_all`
  - `update(&record, CallbackEdgeChanges)` — 已存在，但需从 facade 暴露 thin wrapper。
- `ccb_storage::text_artifacts::maybe_spill_text` — 用于大 body spill。

### 4.2 需补充的 Rust API（只允许 additive）

1. `MessageBureauFacade::update_callback_edge`（wrap store.update）。
2. `MessageBureauControlService::{reply_store, message_store}` 只读 accessor（仿照已有的 `attempt_store`）。
3. `JobDispatcher` callback 配置字段：
   - `callback_timeout_s: f64`（默认 1800.0）
   - `max_callback_depth: u32`（默认 5）
   - builder 方法或从外部 config 注入。

### 4.3 需接入的 JobDispatcher 行为

- `submit`：在创建 job 后，若 `route_options.mode == 'callback'` 则调用 `register_callback_edge`。
- `submit`：普通 ask 从 active parent 发出且非 silent 时，报错。
- `complete`：若 job 是 callback parent 且完成时带 delegated 意图，调用 `delegated_terminal_job` / `persist_delegated_terminal_job`。
- `complete`：若 job 是 callback child 且完成，调用 `submit_callback_continuation`。
- `complete`：若 job 是 callback continuation 且完成，调用 `mark_callback_done`。
- `tick`：每次 tick 调用 `sweep_callback_timeouts` 和 `repair_callback_edges`。

## 5. 推荐实现切片（按依赖顺序）

### Slice 1 — 基础设施
- 加 `MessageBureauFacade::update_callback_edge`。
- 加 `MessageBureauControlService::reply_store` / `message_store` accessor。
- 加 `JobDispatcher` callback 配置字段 + builder。
- 加 `JobDispatcher` 内 helper：`_active_parent_job`、`_message_for_job`、`_callback_child_edge`、`_delegated_parent_edge`、`_latest_child_reply`、`_job_for_reply`、`_existing_continuation_job`。

### Slice 2 — 注册与校验
- `request_callback_route`、`validate_callback_request`、`validate_nested_ask_request`、`register_callback_edge`。
- 在 `submit` 中接入 validation + registration。
- 加测试：#4、#5、#6、#13、#14。

### Slice 3 — continuation 提交
- `_continuation_request`、`_continuation_body`、`_submit_continuation_job`、`submit_callback_continuation`。
- 在 `complete` 中接入 child complete → continuation。
- 加测试：#1、#7。

### Slice 4 — delegated terminal / continuation done
- `delegated_terminal_job`、`persist_delegated_terminal_job`、`mark_callback_done`、`mark_parent_message_waiting`。
- 在 `complete` 中接入 parent complete → delegated；continuation complete → edge DONE。
- 加测试：#1 完整流程。

### Slice 5 — artifact / large reply
- `_reply_summary`、`_callback_body_summary`、artifact flag 处理。
- 加测试：#2、#3。

### Slice 6 — repair
- `repair_callback_edges`。
- 在 `tick` 中接入。
- 加测试：#9、#10、#11。

### Slice 7 — timeout sweep
- `sweep_callback_timeouts`、`fail_callback_edge`、`_record_callback_failure_notice`、`_callback_failure_reply`。
- 在 `tick` 中接入。
- 加测试：#12。

### Slice 8 — error path
- continuation submit 失败处理。
- 加测试：#15。

## 6. Stop-rules

- 不改 ccb-mailbox 线协议/控制面契约；只允许 additive pub accessor。
- 不改 provider hook/settings 注入、tmux namespace/pane identity、`Phase2Services`/`ExecutionService` trait。
- stub 镜像文件保持不动。

## 7. 参考文件

- Python: `lib/ccbd/services/dispatcher_runtime/callbacks.py`
- Python: `lib/message_bureau/facade.py`
- Python: `lib/message_bureau/callback_edges.py`
- Python tests: `test/test_v2_message_bureau_dispatcher_integration.py`
- Rust: `rust/crates/ccb-daemon/src/services/dispatcher.rs`
- Rust: `rust/crates/ccb-mailbox/src/bureau.rs`
- Rust: `rust/crates/ccb-mailbox/src/stores.rs`
- Rust: `rust/crates/ccb-storage/src/text_artifacts.rs`
