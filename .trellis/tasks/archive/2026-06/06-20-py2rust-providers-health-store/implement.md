# Provider health store parity 执行计划

## 实施步骤

1. **增强现有测试**
   - 编辑 `rust/crates/ccb-providers/tests/runtime_tests.rs`。
   - 将原有 `test_provider_health_snapshot_store` 替换为与 Python 测试等价的 `test_provider_health_snapshot_store_tracks_job_history`。
   - 导入 `serde_json::Value`，构造两条 snapshot 并追加，断言 `latest`、`list_job`、`list_all` 的结果。

2. **更新 parity matrix**
   - 在 `plans/rust-python-test-parity-matrix.md` 的 providers 行加入 `test_v2_provider_health_store.py` 与 `runtime_tests.rs`。
   - 在未匹配列表中删除 `test_v2_provider_health_store.py`。

3. **验证**
   - `cargo test -p ccb-providers -- --test-threads=1`
   - `cargo clippy -p ccb-providers -- -D warnings`
   - `cargo fmt -p ccb-providers`
   - `cargo test --workspace -- --test-threads=1`（回归）

## 停止边界

- 不修改 `src/runtime/store.rs` 或 `src/runtime/health.rs` 的实现。
