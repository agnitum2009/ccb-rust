# Parity 迁移执行计划

## 实施步骤

1. **修改 `ccb-jobs/src/store.rs`**
   - 添加 `SCHEMA_VERSION` 常量。
   - 添加 `Record<'a, T>` 包装结构体。
   - 替换三个 `append` 方法中的序列化对象。

2. **扩展 `ccb-jobs/tests/store_integration.rs`**
   - 新增 3 个 header 验证测试。
   - 测试通过直接读取 JSONL 文件内容来断言 header。

3. **验证**
   - `cargo test -p ccb-jobs -- --test-threads=1`
   - `cargo clippy -p ccb-jobs -- -D warnings`

4. **更新 parity matrix**
   - 在 `completion` 或 `memory` 等关联行说明 job record header parity 已关闭；或新增注释行。

## 停止边界

- 不修改 JSONL store 本身。
- 不强制要求读取时校验 schema_version/record_type。
