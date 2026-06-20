# Heartbeat 与 Jobs 迁移（py2rust-jobs）

## 1. 目标

完成 CCB heartbeat 引擎与 jobs 存储从 Python 到 Rust 的迁移收尾，确保 Rust 实现与 Python 参考实现行为等价，记录并修复发现的兼容性缺口。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-heartbeat/`：普通 heartbeat 引擎（`evaluate_heartbeat`）、heartbeat 状态存储、maintenance heartbeat 相关模型与存储。
- `crates/ccb-jobs/`：JobRecord、JobEvent、SubmissionRecord 模型，JobStore、JobEventStore、SubmissionStore 存储操作。

### 2.2 不在范围内

- `lib/maintenance_heartbeat/classifier.py` 中的复杂分类逻辑（已在 Rust `ccb-heartbeat/src/classifier.rs` 中实现，本任务仅做高层审计）。
- job 模型的上层使用方（如 `ccb-completion`、`ccb-daemon`）的调用调整。

## 3. 验收标准

1. `cargo test -p ccb-heartbeat -p ccb-jobs -- --test-threads=1` 全部通过。
2. `cargo clippy -p ccb-heartbeat -p ccb-jobs -- -D warnings` 通过。
3. Rust `evaluate_heartbeat` 的状态机与 Python `heartbeat.engine_runtime.evaluate_heartbeat` 一致。
4. Rust `JobEvent` 的 JSON 序列化字段名与 Python `JobEvent.to_record()` 一致（使用 `type` 而非 `event_type`）。
5. `plans/rust-python-test-parity-matrix.md` 中 `heartbeat`、`completion`、`jobs` 相关集群 Notes 更新。
6. 编写 `audit.md` 记录现状与修复项。

## 4. 约束

- 不能改变 heartbeat schema_version（当前为 1）。
- 不能改变 job record / submission record 的 schema_version（当前为 2）。
- `JobEvent` 字段名变更必须保持 Rust 侧 API 兼容（使用 `serde(rename)`）。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| `JobEvent` 字段名变更影响已有 Rust 测试或调用方 | 低 | 使用 `serde(rename)`，Rust 字段名不变；运行全 workspace 测试确认 |
| maintenance heartbeat 分类逻辑与 Python 有细微差异 | 中 | 依赖现有集成测试；必要时补充边界测试 |
