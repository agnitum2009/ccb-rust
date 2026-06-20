# Provider restore launchers parity 执行计划

## 实施步骤

1. **新建 `rust/crates/ccb-providers/src/session_paths.rs`**
   - 实现 `find_project_ccb_dir(runtime_dir: impl AsRef<Path>) -> Option<PathBuf>`：
     - 遍历 `runtime_dir` 及其祖先，若目录名为 `.ccb` 则返回。
     - 否则将路径转为 `Utf8Path` 并调用 `runtime_project_anchor_from_path`。
   - 实现 `session_file_for_runtime_dir(provider: &str, runtime_dir: impl AsRef<Path>) -> Option<PathBuf>`：
     - 调用 `find_project_ccb_dir`。
     - 取 `runtime_dir` 的 `parent()?.parent()?.file_name()` 作为 agent_name。
     - 调用 `session_filename_for_agent(provider, agent_name)` 得到文件名，返回 `ccb_dir / filename`。
   - 实现 `state_dir_for_runtime_dir(runtime_dir: impl AsRef<Path>) -> Option<PathBuf>`：
     - provider = `runtime_dir.file_name()?.to_str()?.trim().to_lowercase()`。
     - 检查 `runtime_dir.parent()?.file_name()? == "provider-runtime"`。
     - agent_dir = `runtime_dir.parent()?.parent()?`。
     - 返回 `agent_dir / "provider-state" / provider`。
   - 实现 `read_session_payload(session_path: impl AsRef<Path>) -> Option<Map<String, Value>>`。

2. **更新 `rust/crates/ccb-providers/src/lib.rs`**
   - 添加 `pub mod session_paths;`。

3. **更新 `rust/crates/ccb-providers/src/claude/launcher_runtime/session_paths.rs`**
   - 改为 `pub use crate::session_paths::{find_project_ccb_dir, read_session_payload, session_file_for_runtime_dir, state_dir_for_runtime_dir};`

4. **新建 `rust/crates/ccb-providers/tests/provider_session_paths_tests.rs`**
   - 使用 `tempfile::tempdir()` 创建临时目录。
   - 编写 `test_session_file_for_runtime_dir_follows_relocated_runtime_anchor`：
     - 构造 `project_root/.ccb`、relocated_root、`relocated_root/runtime-root.json` marker。
     - runtime_dir = `relocated_root/agents/reviewer/provider-runtime/<provider>`。
     - 断言 `find_project_ccb_dir(runtime_dir) == Some(anchor)`。
     - 断言 `session_file_for_runtime_dir(provider, runtime_dir) == Some(anchor / ".<provider>-reviewer-session")`。
   - 编写 `test_session_file_for_runtime_dir_rejects_invalid_runtime_marker`：
     - marker 中 `runtime_root_path` 指向不同目录。
     - 断言 `find_project_ccb_dir(runtime_dir) == None`。
     - 断言 `session_file_for_runtime_dir(provider, runtime_dir) == None`。
   - 使用 `[("codex"), ("claude"), ("gemini")]` 循环。

5. **更新 parity matrix**
   - 在 providers 行加入 `test_v2_provider_restore_launchers.py` 与 `provider_session_paths_tests.rs`。
   - 在未匹配列表中删除 `test_v2_provider_restore_launchers.py`。

6. **验证**
   - `cargo test -p ccb-providers -- --test-threads=1`
   - `cargo clippy -p ccb-providers -- -D warnings`
   - `cargo fmt -p ccb-providers`
   - `cargo test --workspace -- --test-threads=1`（回归）

## 停止边界

- 不实现 `resolve_claude_restore_target`、history locator、`build_start_cmd` resume/continue 逻辑。
- 不修改 `ccb-daemon` 或 `ccb-storage` 的 marker 格式。
