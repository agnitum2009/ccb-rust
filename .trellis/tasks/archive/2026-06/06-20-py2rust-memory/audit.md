# Memory 迁移审查

## 审计对象

- `rust/crates/ccb-memory/src/transfer.rs`
- `rust/crates/ccb-provider-sessions/src/files.rs`
- `rust/crates/ccb-memory/tests/integration_tests.rs`

## 现状

- `ccb-memory` 已覆盖 context-transfer 主流程，但 `load_session_data` 与 `auto_source_candidates` 直接使用 `find_project_session_file`，未处理 workspace binding 中的 `agent_name`。
- Python `test_memory_transfer_session_binding.py` 验证：当工作目录是 workspace binding 时，应查找 `.ccb/.codex-agent4-session` 这类带 agent name 后缀的会话文件。

## 已做变更

1. 在 `ccb-provider-sessions/src/files.rs` 新增：
   - `find_bound_session_file`：优先根据 `.ccb-workspace.json` 中的 `agent_name` 生成带后缀的会话文件名并查找；无 binding 时回退到原 `find_project_session_file`。
   - 辅助函数 `session_filename_for_instance`、`workspace_binding_agent_name`、`session_filename_matches`、`env_bound_session_file`。
2. 在 `ccb-memory/src/transfer.rs`：
   - `load_session_data` 与 `auto_source_candidates` 改为调用 `find_bound_session_file`。
   - 移除不再使用的 `ProviderClientSpec` 导入。
3. 在 `ccb-memory/tests/integration_tests.rs` 新增两个 workspace binding 发现测试。
4. `ccb-memory/Cargo.toml` 添加 `filetime` dev-dependency 用于测试中的 mtime 控制。
5. 更新 `plans/rust-python-test-parity-matrix.md`。

## 验证结果

- `cargo test -p ccb-memory -- --test-threads=1`：通过（新增 2 个测试）。
- `cargo test -p ccb-provider-sessions -- --test-threads=1`：通过。
- `cargo clippy -p ccb-memory -- -D warnings`：通过。
- `cargo clippy -p ccb-provider-sessions -- -D warnings`：通过。

## 剩余缺口

- 未完整复刻 Python `provider_core.session_binding_runtime` 的全部能力（如 env override 的实例名匹配、instance resolution 等），但已覆盖当前测试所需的最小 parity。
