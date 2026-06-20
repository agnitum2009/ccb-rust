# 测试对等与 Python 退役（py2rust-parity）

## Goal

补齐 Rust job/event/submission 持久化记录与 Python JSONL 记录的格式 parity，确保 Rust 写入的记录可被 Python 参考实现识别，反之亦然。

## Scope

- `rust/crates/ccb-jobs/src/store.rs`
- `rust/crates/ccb-jobs/tests/store_integration.rs`

## Requirements

1. `JobStore::append`、`JobEventStore::append`、`SubmissionStore::append` 写入的 JSONL 行必须包含：
   - `"schema_version": 2`
   - `"record_type": "job_record" | "job_event" | "submission_record"`
2. 记录的其他字段保持平铺在顶层（与 Python `to_record()` 输出一致）。
3. 读取端保持向后兼容：既能读取带 header 的新记录，也能读取无 header 的旧记录。
4. 新增测试验证三种记录类型的 JSON 输出包含 header。

## Acceptance Criteria

- [ ] `cargo test -p ccb-jobs -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-jobs -- -D warnings` 通过。
- [ ] parity matrix 已更新，标记 job record header 缺口已关闭。

## Notes

- 这是 `06-20-python-rust-migration` 父任务最后一个子任务。
