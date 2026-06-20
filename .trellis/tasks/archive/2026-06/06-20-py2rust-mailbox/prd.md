# Mailbox 与 Message Bureau 迁移（py2rust-mailbox）

## 1. 目标

完成 CCB mailbox 内核与 message bureau 控制/追踪队列从 Python 到 Rust 的迁移收尾。这些 crate 已经有较完整的前期实现，本任务以验证、补齐微小缺口、更新测试映射为主。

## 2. 范围

### 2.1 在范围内

- `crates/ccb-mailbox/`：mailbox kernel、inbound event store、delivery lease、mailbox state、transitions、service、queries、facade recording。
- `crates/ccb-message-bureau/`：control queue、control trace、facade、recording、reply metadata/payloads；该 crate 主要 re-export `ccb_mailbox` 的对应实现。

### 2.2 不在范围内

- `ccb-daemon` 对 mailbox 的调用逻辑（属于 `py2rust-daemon`）。
- provider 后端对 message bureau 的使用（属于 `py2rust-providers`）。

## 3. 验收标准

1. `cargo test -p ccb-mailbox -p ccb-message-bureau -- --test-threads=1` 全部通过。
2. `cargo clippy -p ccb-mailbox -p ccb-message-bureau -- -D warnings` 通过。
3. `ccb-mailbox` 的公共 API 与 Python `lib/mailbox_kernel/__init__.py` 的 `__all__` 对齐（已有编译期检查）。
4. `ccb-message-bureau` 的公共 API 与 Python `lib/message_bureau/__init__.py` 的 `__all__` 对齐（已有编译期检查）。
5. `plans/rust-python-test-parity-matrix.md` 中 `mailbox` 集群 Notes 更新。
6. 编写 `audit.md` 记录现状与任何 deferred 缺口。

## 4. 约束

- 不能改变 mailbox schema_version（当前为 2）。
- 不能改变 message bureau 的 record_type/schema_version 约定。
- 保持 `ccb-message-bureau` 作为 `ccb_mailbox` 的薄封装，不重复实现逻辑。

## 5. 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| mailbox 状态机 transition 与 Python 有细微差异 | 中 | 依赖现有集成测试覆盖完整生命周期；必要时补充边界测试 |
| message-bureau re-export 的模块未来漂移 | 低 | 编译期 `__all__` 对齐测试已存在 |
