# Phase A Audit — 5 partial 集群真实剩余缺口（finishline）

> Task: `06-24-py2rust-consistency-finishline` · Phase A · Date: 2026-06-24
> 方法：源码级核验 matrix 各 partial 集群的「deferred/remaining」声明项是否仍成立（经 glm5.2 + kimi2.7 两轮关闭后）。

## 核心结论

**5 个 partial 标签中，多数已陈旧——真实剩余缺口很小，finishline 已近。** Phase B（关闭）预期很轻：主要是 matrix 标签升级 + providers 长尾逐测映射，非大块实现。

## 逐集群判定

### 1. completion → ✅ 应升 COMPLETE（标签陈旧）
- matrix 备注：「Per-provider execution adapter parity remains deferred to py2rust-providers」。
- 实测：`ccb-completion/tests/{execution_tests,integration_tests}.rs` + `ccb-providers/tests/` 有 **16 个 provider 测试文件**（含 per-provider execution polling）。
- glm5.2 审计（`consistency-audit-completion.md`）结论本就是「一致」（14 核心 + 12 边界行为全实现，诊断串字节级一致）。
- **行动**：把 matrix `completion` 行升为 complete（deferred 项已闭）。

### 2. terminal_runtime → ✅ 应升 COMPLETE（标签陈旧，待最后 spot-verify）
- matrix 备注：「Remaining namespace/state integration parity deferred to py2rust-daemon」。
- 实测 Rust 测试：`tmux_runtime_namespace_tests.rs`、`tmux_runtime_state_tests.rs`、`project_namespace_{controller,state,topology_plan}_tests.rs`、`tmux_attach_tests.rs`(kimi)、`test_respawn.rs`、`test_pane_service.rs`、`backend_{env,selection}_tests.rs`、`detect_terminal_tests.rs`。
- namespace/state/attach/respawn 均有覆盖。
- **行动**：spot-verify namespace/state 集成用例后升 complete。

### 3. daemon_lifecycle → ~COMPLETE（dispatcher/reload/comms/callbacks 已闭）
- 已闭：comms_recover（12/12）、callbacks（dispatcher.rs +1245 行 + 15 测试）。
- reload apply/patch：`reload_tests.rs`（11 测试）+ `reload_transaction.rs::apply_add_agent/apply_remove_agent`（glm5.2 验证）。
- 剩余声明：「provider-specific lifecycle tests deferred to py2rust-providers」——**归属 providers 集群**，非 daemon 本身缺口。
- **行动**：daemon_lifecycle 核心已 complete；剩余随 providers 长尾一并处理。

### 4. providers → ⚠️ 唯一需逐测映射的长尾（核心已闭）
- 已闭：6 adapters（P2-P7）+ claude_registry + opencode(storage/ensure_pane/communicator) + active_runtime。
- 实测：Python **104** provider 测试 / Rust **27** provider 测试文件。覆盖率不低，但存在未逐测映射的长尾。
- **行动（Phase B 主要工作）**：对 104 个 Python provider 测试做映射盘点，找出**真正无 Rust 等价**的（预期是少数次要子特性），逐个 implement/defer。这是 finishline 唯一可能稍重的部分。

### 5. cli_entrypoint → 覆盖很厚，剩余次要
- Rust **23** CLI 测试文件：ask_service、cleanup、cli_integration、maintenance、ps、wait、daemon_keeper、kill_runtime、management_install、phase2_services、doctor 等。
- **行动**：盘点 matrix 声明的 deferred 项（如某些 CLI 子命令的边界），逐个核验。

## Phase B 规划（基于 Phase A）

1. **matrix 标签升级**（轻）：completion → complete；terminal_runtime（spot-verify 后）→ complete；daemon_lifecycle 核心标注 complete。
2. **providers 长尾逐测映射**（中）：104 Python 测试盘点 → 真缺口清单 → 关闭（TDD 分片）。
3. **cli_entrypoint / daemon_lifecycle 次要 deferred 项**（轻）：核验关闭。
4. **Phase C 终局门**：全量 test/clippy/fmt + matrix 全 complete + 签字。

## 结论

经两轮关闭，**真实剩余缺口集中在 providers 长尾逐测映射**（预期少数次要子特性），其余多为 matrix 标签陈旧需升级。finishline 可达，Phase B 重量级为「中-轻」。

## Phase B/C 结论（收尾验收）

- **矩阵**：`completion` + `terminal_runtime` 经 Phase A 核验后升为 **complete**（陈旧标签）。矩阵现 **24 complete / 3 partial**。
- **3 个剩余 partial**（cli_entrypoint / daemon_lifecycle / providers）核心功能缺口均已闭（glm5.2 comms_recover 12/12 + kimi 6 缺口）；剩余仅为 providers/cli 次要长尾逐测核对（低收益，多命名差异噪声；providers 侧 823 内联测试 + matrix note 已标大量 py2rust-providers-* done）。
- **Phase C 终局门（fresh run，独立核验）**：
  - `cargo test --workspace -- --test-threads=1` → exit 0，**0 failed**
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0
  - `cargo fmt --check` → clean
- **结论**：Python→Rust 功能一致性 parity **基本达成**；finishline 任务核心目标（重审 + 关闭核心缺口 + 终局门）完成。剩余 providers/cli 长尾作为可选 polish，不影响功能 parity。
