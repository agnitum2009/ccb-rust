# Heartbeat 与 Jobs 迁移设计

## 1. 现状

- `ccb-heartbeat` 已完整实现：
  - 普通 heartbeat 状态机（`engine.rs`）与 Python `heartbeat.engine_runtime` 一致。
  - heartbeat 状态存储（`store.rs`）。
  - maintenance heartbeat 的 schedule、status、runner、activation 等模型（`models.rs`）。
  - maintenance heartbeat 评估与分类（`maintenance.rs`、`classifier.rs`）。
- `ccb-jobs` 已完整实现：
  - `JobRecord`、`JobEvent`、`SubmissionRecord` 模型（`models.rs`）。
  - `JobStore`、`JobEventStore`、`SubmissionStore`（`store.rs`）。

## 2. 模块映射

| Python 参考 | Rust Crate | 入口 | 状态 |
|-------------|-----------|------|------|
| `heartbeat.engine_runtime` | `ccb-heartbeat::engine` | `src/engine.rs` | ✅ 一致 |
| `heartbeat.models` | `ccb-heartbeat::models` | `src/models.rs` | ✅ 一致 |
| `heartbeat.store` | `ccb-heartbeat::store` | `src/store.rs` | ✅ 一致 |
| `maintenance_heartbeat.*` | `ccb-heartbeat::{maintenance, classifier, lock}` | `src/maintenance.rs` 等 | ✅ 已覆盖 |
| `jobs.store` | `ccb-jobs::store` | `src/store.rs` | ✅ 一致 |
| `ccbd.api_models_runtime.records` | `ccb-jobs::models` | `src/models.rs` | ⚠️ `JobEvent.type` 字段名需修复 |

## 3. 兼容性修复

### 3.1 `JobEvent` JSON 字段名

Python `JobEvent.to_record()` 输出字段 `type`，Rust 原实现输出 `event_type`。修复方式：

```rust
#[serde(rename = "type")]
pub event_type: String,
```

这样 Rust API 仍使用 `event_type`，但序列化/反序列化使用 `type`，与 Python 兼容。

## 4. 测试策略

- 运行目标 crate 单元测试和集成测试。
- 为 `JobEvent` 增加 JSON roundtrip 测试，验证字段名为 `type`。
- 运行全 workspace 测试，确保字段名变更不影响其他 crate。
