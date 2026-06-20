# Daemon 控制平面迁移执行计划

## 1. 执行步骤

### Step 1：运行测试基线

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-daemon -- --test-threads=1
```

### Step 2：修复健康检查测试

修改 `crates/ccb-daemon/src/services/health.rs` 中的 `test_assess_provider_panes_owned_by_namespace_are_healthy`，在测试期间清空 `PATH`。

### Step 3：最终验证

```bash
cd /home/agnitum/ccb/rust
cargo fmt --check
cargo clippy -p ccb-daemon -- -D warnings
cargo test -p ccb-daemon -- --test-threads=1
cargo test --workspace -- --test-threads=1
```

### Step 4：文档更新

- 更新 `plans/rust-python-test-parity-matrix.md` 中 `daemon_lifecycle` 集群 Notes。
- 编写 `audit.md`。

## 2. 审查门

- **Gate A**：`ccb-daemon` 测试全部通过。
- **Gate B**：`ccb-daemon` clippy 无警告。
- **Gate C**：全 workspace 测试无新增失败（原有 baseline 应已消除）。
- **Gate D**：parity matrix 更新。

## 3. 回滚点

- 本任务开始前打 tag：`py2rust-daemon-baseline`。

## 4. 预计产出

- `services/health.rs` 测试修复。
- `audit.md` 与更新的 parity matrix。
