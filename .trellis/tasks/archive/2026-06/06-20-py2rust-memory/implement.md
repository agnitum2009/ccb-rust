# Memory 迁移执行计划

## 实施步骤

1. **扩展 `ccb-provider-sessions/src/files.rs`**
   - 在 `find_project_session_file` 后添加 `find_bound_session_file` 及辅助函数。

2. **修改 `ccb-memory/src/transfer.rs`**
   - 导入 `find_bound_session_file`。
   - 在 `load_session_data` 与 `auto_source_candidates` 中替换发现调用。
   - 移除未使用的 `ProviderClientSpec` 导入（如不再使用）。

3. **新增 `ccb-memory` 测试**
   - 在 `tests/integration_tests.rs` 添加 workspace binding 发现测试。

4. **验证**
   - `cargo test -p ccb-memory -- --test-threads=1`
   - `cargo test -p ccb-provider-sessions -- --test-threads=1`
   - `cargo clippy -p ccb-memory -- -D warnings`
   - `cargo clippy -p ccb-provider-sessions -- -D warnings`

5. **更新 parity matrix**
   - 在 `memory` 行说明 workspace binding 会话发现已覆盖。

## 停止边界

- 不重构 `provider_core.session_binding_runtime` 的完整能力（env override、instance resolution 等）。
- 不修改会话内容解析逻辑。
