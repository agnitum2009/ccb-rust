# Terminal 与 Pane Registry 迁移（py2rust-terminal）

## 1. 目标

完成 CCB terminal 后端（tmux）与 pane registry 从 Python 到 Rust 的迁移收尾，修复当前测试和 lint 失败，确保核心 tmux 辅助函数在 tmux 和非 tmux 环境下都稳定。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-terminal/`：tmux backend、pane 管理、layout、detect、identity、logs、theme、respawn 等。
- `crates/ccb-pane-registry/`：pane 查找、写入、provider/project 匹配。

### 2.2 不在范围内

- `lib/cli/services/` 中使用 tmux 的上层 CLI 逻辑（属于 `py2rust-cli`）。
- `lib/ccbd/services/health_assessment/tmux*` 中的健康评估（属于 `py2rust-daemon`）。

## 3. 验收标准

1. `cargo test -p ccb-terminal -p ccb-pane-registry -- --test-threads=1` 全部通过。
2. `cargo clippy -p ccb-terminal -p ccb-pane-registry -- -D warnings` 通过。
3. `ccb-terminal` 的 tmux 检测测试在 tmux 和非 tmux 环境下均通过。
4. `plans/rust-python-test-parity-matrix.md` 中 `terminal_runtime` 集群 Notes 更新。
5. 编写 `audit.md` 记录修复项与任何 deferred 缺口。

## 4. 约束

- 不能改变 tmux pane/session/window 命名约定，避免运行时状态漂移。
- 对 stub 模块添加的 lint 抑制是临时措施，需在后续实现中移除。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| 测试在 tmux 环境中失败 | 中 | 修复测试使其不依赖外部环境 |
| clippy 警告批量抑制隐藏真实问题 | 低 | 标记为 TODO，后续实现时移除 |
