# Provider restore launchers parity 设计

## 变更点

1. `rust/crates/ccb-providers/src/session_paths.rs`（新建）
   - 通用实现 `find_project_ccb_dir`、`session_file_for_runtime_dir`、`state_dir_for_runtime_dir`、`read_session_payload`。
   - 依赖 `ccb_provider_core::pathing::session_filename_for_agent` 与 `ccb_storage::path_helpers::runtime_project_anchor_from_path`。
   - 输入输出使用 `std::path::Path` / `PathBuf`，内部在调用 `runtime_project_anchor_from_path` 时转换为 `camino::Utf8Path`。

2. `rust/crates/ccb-providers/src/lib.rs`
   - 导出 `pub mod session_paths;`。

3. `rust/crates/ccb-providers/src/claude/launcher_runtime/session_paths.rs`
   - 移除 stub 注释，改为 re-export 通用实现：
     - `find_project_ccb_dir`、`session_file_for_runtime_dir`、`state_dir_for_runtime_dir`、`read_session_payload`。
   - 这样保留 Python  parity 所需的模块路径，同时避免三份重复实现。

4. `rust/crates/ccb-providers/tests/provider_session_paths_tests.rs`（新建）
   - 测试 relocated runtime anchor 场景：构造 `runtime-root.json` marker，断言 `find_project_ccb_dir` 返回 anchor，`session_file_for_runtime_dir(provider, runtime_dir)` 返回 `anchor / .<provider>-reviewer-session`。
   - 测试 invalid marker 场景：写入错误的 `runtime_root_path`，断言 `find_project_ccb_dir` 与 `session_file_for_runtime_dir` 返回 `None`。
   - 对 `codex`、`claude`、`gemini` 三个 provider 参数化循环。

5. `plans/rust-python-test-parity-matrix.md`
   - 将 `test_v2_provider_restore_launchers.py` 从“未匹配”列表移入 providers 集群，并增加测试文件引用与说明。

## 兼容性

- 新增模块，不改动现有 provider launcher 行为。
- `claude/launcher_runtime/session_paths.rs` 从空 stub 变为 re-export，现有 `use` 该模块的代码（如有）会获得实际函数。
