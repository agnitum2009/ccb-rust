# Memory 迁移（py2rust-memory）

## Goal

补齐 `ccb-memory` 中 context-transfer 会话发现与 Python `test_memory_transfer_session_binding.py` 的 parity。

## Scope

- `rust/crates/ccb-provider-sessions/src/files.rs`：新增 `find_bound_session_file`，支持 workspace binding 中的 `agent_name` 解析与会话文件后缀匹配。
- `rust/crates/ccb-memory/src/transfer.rs`：`load_session_data` 与 `auto_source_candidates` 切换到新的绑定感知发现逻辑。
- `rust/crates/ccb-memory/tests/integration_tests.rs`：新增 workspace binding 会话发现测试。

## Requirements

1. 当 `work_dir` 是 workspace binding 目录且包含 `.ccb-workspace.json` 时，
   `load_session_data` 与 `auto_source_candidates` 应根据 `agent_name` 查找形如 `.codex-agent4-session` 的会话文件。
2. 保持非 workspace binding 场景下的既有行为不变。
3. 新增单元测试覆盖：
   - 通过 workspace binding 的 `agent_name` 解析到目标项目中的会话文件。
   - `auto_source_candidates` 按目标项目会话文件 mtime 排序。

## Acceptance Criteria

- [ ] `cargo test -p ccb-memory -- --test-threads=1` 通过。
- [ ] `cargo test -p ccb-provider-sessions -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-memory -- -D warnings` 通过。
- [ ] `cargo clippy -p ccb-provider-sessions -- -D warnings` 通过。
- [ ] parity matrix 已更新。

## Notes

- Python 参考实现由 `provider_core.session_binding_runtime` 完成；本次 Rust 实现聚焦最小可用 parity。
