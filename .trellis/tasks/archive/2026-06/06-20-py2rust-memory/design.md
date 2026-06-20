# Memory 迁移设计

## 变更点

1. `rust/crates/ccb-provider-sessions/src/files.rs`
   - 新增 `find_bound_session_file(work_dir, provider, base_filename)`：
     - 检查 `CCB_SESSION_FILE` 环境变量（可选，简单实现）。
     - 读取 `.ccb-workspace.json` 中的 `agent_name`。
     - 若存在 agent name，将会话文件名转换为 `<base>-<agent>-session` 形式，再通过 `find_project_session_file` 查找。
     - 否则回退到 `find_project_session_file` 的原始行为。
   - 辅助函数：`session_filename_for_instance`、`workspace_binding_agent_name`、`session_filename_matches`。

2. `rust/crates/ccb-memory/src/transfer.rs`
   - `load_session_data` 与 `auto_source_candidates` 不再直接调用 `find_project_session_file`，改为调用 `find_bound_session_file`。
   - 移除不再需要的 `ProviderClientSpec` 中间构造。

3. `rust/crates/ccb-memory/tests/integration_tests.rs`
   - 新增两个测试，模拟 workspace binding 目录与目标项目会话文件，验证绑定发现。

## 兼容性

- 无 workspace binding 时，`find_bound_session_file` 等价于原 `find_project_session_file`。
- 现有 `ccb-memory` 测试继续通过。
