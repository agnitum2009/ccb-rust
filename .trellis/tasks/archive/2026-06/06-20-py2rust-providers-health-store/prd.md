# Provider health store parity

## Goal

补齐 `ccb-providers` 中 `ProviderHealthSnapshotStore` 的测试覆盖，和 Python `test_v2_provider_health_store.py` 保持 parity。

## Scope

- `rust/crates/ccb-providers/src/runtime/store.rs`（已有实现，仅验证）
- `rust/crates/ccb-providers/src/runtime/health.rs`（已有类型，仅验证）
- `rust/crates/ccb-providers/tests/provider_health_snapshot_store_tests.rs`（新建）

## Requirements

1. 新建 Rust 集成测试，覆盖 `ProviderHealthSnapshotStore` 的核心行为：
   - 向同一 `job_id` 追加两条 `ProviderHealthSnapshot`。
   - `latest(job_id)` 返回最后追加的快照（按 JSONL 顺序）。
   - 返回结果的 `agent_name`、`progress_state`、`completion_state` 与追加值一致（注意 Python 测试第二条 agent_name 为小写 `agent1`）。
   - `list_job(job_id)` 返回该 job 的全部快照（长度为 2）。
   - `list_all()` 返回所有快照（长度为 2）。
2. 测试使用 `tempfile` 创建临时项目目录，通过 `PathLayout` 构造 store。
3. 快照字段通过 `ProviderHealthSnapshot::new(...)` 与 builder 方法构造，diagnostics 使用 `HashMap<String, serde_json::Value>`。

## Acceptance Criteria

- [ ] `cargo test -p ccb-providers -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-providers -- -D warnings` 通过。
- [ ] `cargo fmt -p ccb-providers` 无需产生新 diff。
- [ ] parity matrix 已更新 health store 行。

## Stop Boundary

- 不修改 `ProviderHealthSnapshot` / `ProviderHealthSnapshotStore` 的 API 或存储路径。
- 不进入 provider runtime health 评估逻辑。
