# Parity 迁移设计

## 变更点

1. `rust/crates/ccb-jobs/src/store.rs`
   - 新增 `SCHEMA_VERSION = 2`。
   - 新增泛型 `Record<'a, T: Serialize>`，包含 `schema_version`、`record_type` 与 `#[serde(flatten)] payload`。
   - `JobStore::append` 写入 `Record<JobRecord>`（`record_type = "job_record"`）。
   - `JobEventStore::append` 写入 `Record<JobEvent>`（`record_type = "job_event"`）。
   - `SubmissionStore::append` 写入 `Record<SubmissionRecord>`（`record_type = "submission_record"`）。
   - 读取仍直接反序列化为原类型；serde 默认忽略未知字段，天然兼容 header。

2. `rust/crates/ccb-jobs/tests/store_integration.rs`
   - 新增 `job_store_record_has_header`、`submission_store_record_has_header`、`job_event_store_record_has_header`。
   - 读取生成的 JSONL 文件，验证 header 字段存在且正确。

## 兼容性

- 不修改 `JobRecord`/`JobEvent`/`SubmissionRecord` 结构体，避免破坏现有读取路径。
- 旧记录无 header 仍可正常读取；新记录带 header 可被 Python `_validate_record` 接受。
