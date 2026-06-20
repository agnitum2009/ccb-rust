# py2rust-jobs 审计报告

审计时间：2026-06-20
审计范围：`ccb-heartbeat`、`ccb-jobs`

## 1. 总体结论

Heartbeat 与 Jobs 的 Rust 实现已经高度完整。本任务修复了一个 Python 兼容性缺口（`JobEvent` JSON 字段名），并补充了回归测试。

## 2. 逐项审计

### 2.1 `ccb-heartbeat`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `heartbeat.engine_runtime.evaluate_heartbeat` | `src/engine.rs` | ✅ 一致 | 状态机逻辑完全对应 |
| `heartbeat.models` | `src/models.rs` | ✅ 一致 | `HeartbeatAction`、`HeartbeatPolicy`、`HeartbeatState`、`HeartbeatDecision` |
| `heartbeat.store` | `src/store.rs` | ✅ 一致 | `HeartbeatStateStore` |
| `maintenance_heartbeat.*` | `src/{maintenance,classifier,lock}.rs` | ✅ 已覆盖 | maintenance schedule/status/runner/activation 模型完整 |

### 2.2 `ccb-jobs`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `jobs.store.JobStore` | `src/store.rs` | ✅ 一致 | append/list_target/get_latest |
| `jobs.store.JobEventStore` | `src/store.rs` | ✅ 一致 | append/read_since |
| `jobs.store.SubmissionStore` | `src/store.rs` | ✅ 一致 | append/list_all/get_latest |
| `ccbd.api_models_runtime.records.JobRecord` | `src/models.rs` | ✅ 一致 | 字段与 Python 一致 |
| `ccbd.api_models_runtime.records.SubmissionRecord` | `src/models.rs` | ✅ 一致 | 字段与 Python 一致 |
| `ccbd.api_models_runtime.records.JobEvent` | `src/models.rs` | ⚠️ 已修复 | 原使用 `event_type`，已改为 `#[serde(rename = "type")]` |

## 3. 修复项

### 3.1 `JobEvent` JSON 字段名

- **问题**：Python `JobEvent.to_record()` 输出字段 `type`，Rust 原实现输出 `event_type`。
- **修复**：在 `rust/crates/ccb-jobs/src/models.rs` 中为 `JobEvent.event_type` 添加 `#[serde(rename = "type")]`。
- **测试**：新增 `job_event_serializes_type_field_for_python_compatibility` 集成测试。

## 4. 已知但未修复的缺口

### 4.1 Job 记录缺少 schema header

- Python `JobRecord.to_record()`、`JobEvent.to_record()`、`SubmissionRecord.to_record()` 均包含 `schema_version: 2` 和 `record_type`。
- Rust 当前直接通过 `serde_json` 序列化模型，没有 schema header。
- **影响**：双轨运行期间，Python 读取 Rust 生成的 JSONL 可能失败；但 Rust 自洽。
- **建议**：Defer 到 `py2rust-parity` 任务统一处理 schema header 兼容问题，或确认 Rust 成为唯一真相源后不再需要考虑。

## 5. 验证结果

```bash
cargo clippy -p ccb-heartbeat -p ccb-jobs -- -D warnings
# 通过

cargo test -p ccb-heartbeat -p ccb-jobs -- --test-threads=1
# 全部通过
```
