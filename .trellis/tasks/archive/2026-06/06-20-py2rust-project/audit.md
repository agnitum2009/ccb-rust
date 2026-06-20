# py2rust-project 审计报告

审计时间：2026-06-20
审计范围：`ccb-project`、`ccb-workspace`

## 1. 总体结论

Project 与 Workspace 的 Rust 实现已经高度完整，与 Python 参考实现行为一致。本任务补充了一个跨语言 project id 一致性测试，并更新了 parity matrix。

## 2. 逐项审计

### 2.1 `ccb-project`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| `project.discovery` | `src/discovery.rs` | ✅ 一致 | 锚点发现、workspace binding、危险根目录检测 |
| `project.ids` | `src/ids.rs` | ✅ 一致 | 路径归一化、project id、slug |
| `project.identity` | `src/identity.rs` | ✅ 一致 | work_dir 归一化、worktree scope id、project root 解析 |
| `project.resolver` | `src/resolver.rs` | ✅ 一致 | `ProjectResolver.resolve`、`bootstrap_project` |
| `project.runtime_paths` | `src/runtime_paths.rs` | ✅ 已审计 | 运行时路径布局完整 |

### 2.2 `ccb-workspace`

| Python 参考 | Rust 实现 | 状态 | 备注 |
|-------------|-----------|------|------|
| workspace binding 解析/持久化 | `src/binding.rs` | ✅ 一致 | schema_version=2, record_type=workspace_binding |
| workspace plan 验证 | `src/validator.rs` | ✅ 一致 | inplace/git-worktree/copy 规则完整 |
| git worktree 处理 | `src/git_worktree.rs` | ✅ 已实现 | |
| workspace 物化与协调 | `src/materializer.rs`, `src/reconcile.rs` | ✅ 已实现 | |
| workspace actor 解析 | `src/actors.rs` | ✅ 已实现 | |

## 3. 补全项

- 新增跨语言一致性测试 `test_project_id_matches_python_reference`，验证 Python `compute_project_id` 与 Rust `compute_project_id` 对相同路径输出一致。

## 4. 发现的问题

- 未发现 `ccb-project` 或 `ccb-workspace` 范围内的功能缺口。
- `ccb-workspace` 比 Python 中分散的 workspace 逻辑更加内聚，属于 Rust 侧合理重构。

## 5. 验证结果

```bash
cargo clippy -p ccb-project -p ccb-workspace -- -D warnings
# 通过

cargo test -p ccb-project -p ccb-workspace -- --test-threads=1
# 全部通过（含新增 project id 一致性测试）
```
