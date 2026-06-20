# Provider health store parity 设计

## 变更点

1. `rust/crates/ccb-providers/tests/runtime_tests.rs`
   - 已有 `test_provider_health_snapshot_store` 基本覆盖，将其增强为与 Python `test_v2_provider_health_store.py` 等价的 `test_provider_health_snapshot_store_tracks_job_history`。
   - 使用 `tempfile::TempDir` 创建临时目录，构造 `PathLayout`。
   - 调用 `ProviderHealthSnapshotStore::new(layout)` 创建 store。
   - 构造两条 snapshot：
     - 第一条：`Agent1`、`ACCEPTED`、`NOT_COMPLETE`、`runtime_alive=true`、`session_reachable=true`。
     - 第二条：`agent1`、`OUTPUT_ADVANCING`、`TERMINAL_COMPLETE`、`runtime_alive=true`、`session_reachable=true`。
   - 断言 `latest('job-1')` 为第二条；`list_job('job-1')` 长度为 2；`list_all()` 长度为 2。

2. `plans/rust-python-test-parity-matrix.md`
   - 将 `test_v2_provider_health_store.py` 从“未匹配”列表移入 providers 集群，并增加 `runtime_tests.rs` 引用。

## 兼容性

- 仅新增测试，不改动生产代码。
- 依赖 `tempfile` 已在 `ccb-providers` dev-dependencies 中声明。
