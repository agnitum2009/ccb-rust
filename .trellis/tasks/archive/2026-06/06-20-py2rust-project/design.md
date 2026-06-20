# Project 与 Workspace 迁移设计

## 1. 现状

- `ccb-project` 已完整实现 Python `project` 包的核心功能：
  - `discovery.rs`：项目锚点发现、workspace binding 发现、危险根目录检测。
  - `ids.rs`：`normalize_project_path`、`compute_project_id`、`project_slug`。
  - `identity.rs`：`normalize_work_dir`、`compute_worktree_scope_id`、`resolve_project_root`、`compute_ccb_project_id`。
  - `resolver.rs`：`ProjectResolver`、`bootstrap_project`。
  - `runtime_paths.rs`：运行时路径。
- `ccb-workspace` 是 Rust 侧对 workspace 概念的重构实现，包含：
  - `models.rs`：`WorkspacePlan`、`WorkspaceRef`、`WorkspaceBinding`。
  - `binding.rs`：workspace binding 的读写。
  - `validator.rs`：workspace plan 验证。
  - `planner.rs`：根据 agent 拓扑生成 workspace plan。
  - `git_worktree.rs`：git worktree 创建与切换。
  - `materializer.rs`：workspace 物化。
  - `reconcile.rs`：workspace 状态协调。
  - `actors.rs`：workspace actor 解析。

## 2. 模块映射

| Python 参考 | Rust Crate | 入口 | 状态 |
|-------------|-----------|------|------|
| `project.discovery` | `ccb-project::discovery` | `src/discovery.rs` | ✅ 一致 |
| `project.ids` | `ccb-project::ids` | `src/ids.rs` | ✅ 一致 |
| `project.identity` | `ccb-project::identity` | `src/identity.rs` | ✅ 一致 |
| `project.resolver` | `ccb-project::resolver` | `src/resolver.rs` | ✅ 一致 |
| `project.runtime_paths` | `ccb-project::runtime_paths` | `src/runtime_paths.rs` | 待确认 |
| 分散的 workspace 逻辑 | `ccb-workspace::*` | `src/{binding,validator,planner,git_worktree,materializer,reconcile,actors}.rs` | ✅ 已重构实现 |

## 3. 审计重点

1. **project id 稳定性**：确保 Rust `compute_project_id` 与 Python 对相同路径产生相同 digest。
2. **workspace binding schema**：Rust 使用 `schema_version=2` + `record_type=workspace_binding`，与 Python 一致。
3. **危险根目录检测**：home、temp root、filesystem root 行为一致。
4. **bootstrap_project**：确认 Rust 与 Python 都只创建 `.ccb` 目录和 `ccb.config` 占位，不写入默认配置内容。

## 4. 兼容性

- project id 是 socket key 和存储路径的基础，必须保持字节级一致。
- `.ccb-workspace.json` 格式必须保持向前/向后兼容。

## 5. 测试策略

- 运行目标 crate 测试。
- 补充一个跨语言 project id 一致性测试：对同一虚拟路径，Python `compute_project_id` 与 Rust `compute_project_id` 输出相同。
- 补充 workspace binding 读写 roundtrip 测试（如尚未覆盖）。
