# Parity 迁移审查

## 审计对象

- `rust/crates/ccb-jobs/src/store.rs`
- `rust/crates/ccb-jobs/tests/store_integration.rs`

## 现状

- Rust job/event/submission store 直接序列化领域对象，JSONL 记录缺少 Python 要求的 `schema_version: 2` 与 `record_type` 头。
- Python `jobs/store.py` 通过 `to_record()` 写入头，并通过 `_validate_record()` 读取校验。

## 已做变更

1. 在 `store.rs` 中新增 `Record<'a, T>` 包装结构体，序列化时附带 `schema_version` 与 `record_type`。
2. `JobStore::append`、`JobEventStore::append`、`SubmissionStore::append` 均使用包装器写入。
3. 读取端保持原类型反序列化，serde 默认忽略未知字段，向后兼容无头旧记录。
4. 在 `store_integration.rs` 新增 3 个测试验证三种记录的 JSONL 输出包含正确 header。
5. 更新 `plans/rust-python-test-parity-matrix.md`。

## 验证结果

- `cargo test -p ccb-jobs -- --test-threads=1`：通过（新增 3 个测试）。
- `cargo clippy -p ccb-jobs -- -D warnings`：通过。

## 剩余缺口

- 读取端尚未主动校验 `schema_version`/`record_type`；当前依赖 serde 忽略未知字段。由于写入已带 header，后续如需严格校验可在此扩展。
- 父任务 `06-20-python-rust-migration` 全部 11 个子任务已完成。
