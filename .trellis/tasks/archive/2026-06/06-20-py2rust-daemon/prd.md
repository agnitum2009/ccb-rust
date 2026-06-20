# Daemon 控制平面迁移（py2rust-daemon）

## 1. 目标

完成 CCB daemon（ccbd）控制平面从 Python 到 Rust 的迁移收尾，修复当前测试失败，确保 daemon 在控制平面协议、服务图、健康检查等方面与 Python 参考实现行为一致。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-daemon/`：socket 服务、handlers、service graph、project namespace、reload、health、supervision、start/stop flow 等。

### 2.2 不在范围内

- provider 后端具体实现（属于 `py2rust-providers`）。
- CLI 命令路由（属于 `py2rust-cli`）。
- 安装/升级脚本（属于 `py2rust-parity` 或保留为 source-install 兼容层）。

## 3. 验收标准

1. `cargo test -p ccb-daemon -- --test-threads=1` 全部通过。
2. `cargo clippy -p ccb-daemon -- -D warnings` 通过。
3. daemon 控制平面协议保持与 Python 字节级兼容（不改动 socket/RPC 模型）。
4. `plans/rust-python-test-parity-matrix.md` 中 `daemon_lifecycle` 集群 Notes 更新。
5. 编写 `audit.md` 记录修复项与任何 deferred 缺口。

## 4. 约束

- 不能改变 ccbd socket 路径、协议格式或 handler 路由。
- 不能改变 tmux namespace/pane identity 逻辑。
- 不能改变 project reload 的状态机语义。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| 健康检查测试依赖外部 tmux 环境 | 中 | 修复测试使其独立 |
| daemon 子系统多，隐藏语义差异 | 中 | 依赖集成测试和 parity matrix 逐项验证 |
