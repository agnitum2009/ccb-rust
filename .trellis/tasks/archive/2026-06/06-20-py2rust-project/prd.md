# Project 与 Workspace 迁移（py2rust-project）

## 1. 目标

完成 CCB 项目解析与 workspace 管理从 Python 到 Rust 的迁移收尾。这些 crate 已经有较完整的前期实现，本任务以验证、补齐微小缺口、更新测试映射为主。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-project/`：项目发现、项目身份、project id/slug、workspace binding 解析、`ProjectResolver`、`bootstrap_project`。
- `crates/ccb-workspace/`：workspace 模型、workspace binding 持久化、workspace plan 验证、git worktree 处理、物化与协调。

### 2.2 不在范围内

- `lib/project_memory/` 中的内存渲染与格式化（属于 `py2rust-memory`）。
- `lib/agents/config_loader_runtime/` 中的默认项目配置生成（属于 `py2rust-agents`）。
- provider session 路径管理（属于 `py2rust-providers`）。

## 3. 验收标准

1. `cargo test -p ccb-project -p ccb-workspace -- --test-threads=1` 全部通过。
2. `cargo clippy -p ccb-project -p ccb-workspace -- -D warnings` 通过。
3. `ccb-project` 的公共 API 行为与 Python `project.discovery`、`project.identity`、`project.ids`、`project.resolver` 一致。
4. `ccb-workspace` 的 workspace plan、binding、validator 与 Python 中对应行为一致。
5. `plans/rust-python-test-parity-matrix.md` 中 `config_project` 集群的 Notes 更新为：核心 project/workspace 已完成，剩余 provider/project-memory 相关测试由后续 child tasks 覆盖。
6. 编写 `audit.md` 记录现状与任何 deferred 缺口。

## 4. 约束

- 不能改变 project id 计算方式，否则会导致现有项目 socket key 变化。
- 不能改变 `.ccb-workspace.json` 的 schema。
- 不能删除 `ccb-storage/src/project_identity.rs` 对 `ccb_project::identity` 的 re-export。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| Python workspace 逻辑分散在多个文件，Rust 侧重构后语义有细微差异 | 中 | 重点验证 workspace binding、plan validation、git worktree 分支命名 |
| `bootstrap_project` 与 Python 默认配置生成路径不一致 | 低 | 确认 Python `ensure_bootstrap_project_config` 仅创建目录不写入内容 |
