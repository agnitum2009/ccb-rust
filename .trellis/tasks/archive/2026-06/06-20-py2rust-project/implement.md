# Project 与 Workspace 迁移执行计划

## 1. 执行步骤

### Step 1：运行测试基线

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-project -p ccb-workspace -- --test-threads=1
```

记录通过/失败情况。

### Step 2：Python/Rust project id 一致性验证

```bash
cd /home/agnitum/ccb
python3 - <<'PY'
import sys
sys.path.insert(0, 'lib')
from project.ids import compute_project_id
print('py:', compute_project_id('/home/user/demo-project'))
PY
```

```bash
cd /home/agnitum/ccb/rust
cargo test -p ccb-project test_project_id_deterministic_and_64_chars -- --nocapture
```

对比两者输出（可以手动或写一个小型集成测试）。

### Step 3：审计 ccb-project 公共 API

- 打开 `rust/crates/ccb-project/src/lib.rs`，确认所有 `pub` 项都有对应 Python 功能。
- 检查 `runtime_paths.rs` 是否与 Python `project.runtime_paths` 一致。

### Step 4：审计 ccb-workspace

- 检查 `binding.rs` 的 schema_version/record_type 校验。
- 检查 `validator.rs` 的 workspace mode 规则。
- 检查 `git_worktree.rs` 的分支命名和创建逻辑。

### Step 5：补全与测试

- 若发现缺失，按最小改动原则补全。
- 为每个补全点增加测试。

### Step 6：最终验证

```bash
cd /home/agnitum/ccb/rust
cargo fmt --check
cargo clippy -p ccb-project -p ccb-workspace -- -D warnings
cargo test -p ccb-project -p ccb-workspace -- --test-threads=1
```

### Step 7：文档更新

- 更新 `plans/rust-python-test-parity-matrix.md` 中 `config_project` 集群的 Notes。
- 编写 `audit.md`。

## 2. 审查门

- **Gate A**：目标 crate 测试全部通过。
- **Gate B**：目标 crate clippy 无警告。
- **Gate C**：Python/Rust project id 对标准路径输出一致。
- **Gate D**：parity matrix 更新。

## 3. 回滚点

- 本任务开始前打 tag：`py2rust-project-baseline`。

## 4. 预计产出

- `audit.md`：现状与任何 deferred 缺口。
- 少量补全代码 + 测试（如有）。
- 更新的 `plans/rust-python-test-parity-matrix.md`。
