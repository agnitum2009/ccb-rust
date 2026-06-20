# Provider restore launchers parity

## Goal

补齐 `ccb-providers` 中 provider runtime session path 查找与 relocated runtime anchor 的 parity，对应 Python `test_v2_provider_restore_launchers.py` 中的 `test_session_file_for_runtime_dir_*` 两组参数化测试。

## Scope

- `rust/crates/ccb-providers/src/session_paths.rs`（新建通用模块）
- `rust/crates/ccb-providers/src/lib.rs`（导出模块）
- `rust/crates/ccb-providers/src/claude/launcher_runtime/session_paths.rs`（从 stub 改为复用通用实现或本地 thin wrapper）
- `rust/crates/ccb-providers/tests/provider_session_paths_tests.rs`（新建）

## Requirements

1. 提供与 Python `session_paths.py` 行为一致的通用函数：
   - `find_project_ccb_dir(runtime_dir)`：向上查找 `.ccb` 目录；若找不到则通过 `runtime_project_anchor_from_path` 读取 relocated runtime marker 返回 anchor。
   - `session_file_for_runtime_dir(provider, runtime_dir)`：基于 `find_project_ccb_dir` 与 `session_filename_for_agent` 构造会话文件路径；agent_name 取 `runtime_dir` 向上第 2 级目录名（`.../agents/<agent>/provider-runtime/<provider>`）。
   - `state_dir_for_runtime_dir(runtime_dir)`：返回 `.../agents/<agent>/provider-state/<provider>`。
   - `read_session_payload(session_path)`：读取 JSON 文件并返回 `serde_json::Map<String, Value>`（字典）或 `None`。
2. 新建测试覆盖以下 Python 断言：
   - 对 `codex`、`claude`、`gemini` 三个 provider：
     - `session_file_for_runtime_dir` 在 relocated runtime 场景下返回 anchor 目录中的 `.<provider>-<agent>-session`。
     - `find_project_ccb_dir` 在 invalid runtime marker（`runtime_root_path` 不匹配）时返回 `None`。
     - `session_file_for_runtime_dir` 在 invalid marker 时返回 `None`。
3. 保持 `src/claude/launcher_runtime/session_paths.rs` 的 API 表面可用（可改为 re-export 通用实现）。

## Acceptance Criteria

- [ ] `cargo test -p ccb-providers -- --test-threads=1` 通过。
- [ ] `cargo clippy -p ccb-providers -- -D warnings` 通过。
- [ ] `cargo fmt -p ccb-providers` 无需产生新 diff。
- [ ] parity matrix 已更新 restore launchers 行。

## Stop Boundary

- 本次仅补齐 session path / relocated runtime anchor 部分；不进入 restore target、history locator、`build_start_cmd` 的 resume/continue 逻辑（这些将另开子任务）。
- 不修改 `ccb-storage` 的 marker 解析逻辑。
