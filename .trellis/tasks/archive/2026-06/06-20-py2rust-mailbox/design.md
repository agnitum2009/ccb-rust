# Mailbox 与 Message Bureau 迁移设计

## 1. 现状

- `ccb-mailbox` 已完整实现：
  - `kernel.rs`：mailbox kernel service。
  - `models.rs`：mailbox 与 inbound event 模型。
  - `stores.rs`：delivery lease、inbound event、mailbox store。
  - `transitions.rs`、`claiming.rs`、`leasing.rs`：状态转换。
  - `service.rs` 及其 runtime 子模块：mailbox 服务实现。
  - `facade_recording.rs`、`facade_state.rs`：facade 录制与状态。
  - `control_queue.rs`、`control_trace.rs`：控制队列与追踪。
  - `bureau.rs`：message bureau facade 和控制服务。
- `ccb-message-bureau` 是 `ccb_mailbox` 的薄封装，模块结构和公共 API 与 Python `lib/message_bureau/` 对齐。

## 2. 模块映射

| Python 参考 | Rust Crate | 入口 | 状态 |
|-------------|-----------|------|------|
| `mailbox_kernel.*` | `ccb-mailbox::*` | `src/{kernel,models,stores,transitions,service,...}.rs` | ✅ 已覆盖 |
| `message_bureau.*` | `ccb-message-bureau::*` | `src/{control,control_queue,control_trace,facade,...}.rs` | ✅ re-export 自 ccb-mailbox |

## 3. 审计策略

- 依赖编译期 `__all__` 对齐测试确认公共 API 边界。
- 依赖 `tests/integration.rs` 中的完整消息生命周期测试确认核心行为。
- 对 control queue、trace、facade recording 做抽样代码审查，确认与 Python 结构一致。

## 4. 兼容性

- mailbox 记录使用 schema_version=2，与 Python 一致。
- `ccb-message-bureau` 保持对 `ccb-mailbox` 的 re-export，避免逻辑分叉。

## 5. 测试策略

- 运行目标 crate 单元测试和集成测试。
- 全 workspace 测试确认无新增回归。
