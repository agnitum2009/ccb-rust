# PRD — Wave 3 收尾：Python↔Rust 一致性 finishline

## 背景

Wave 1/2 完成；Wave 3 经 glm5.2 源码级审计转为「一致性审计 + 缺口关闭」。glm5.2 关闭 `comms_recover`（12/12）；kimi2.7 关闭 6 缺口（callbacks/active_runtime/opencode/terminal_runtime/claude_registry）。全量 `cargo test --workspace` + clippy `-D warnings` + fmt 已绿并核验。

parity matrix 现仍有 **5 个集群标 partial**：`cli_entrypoint`、`daemon_lifecycle`、`providers`、`terminal_runtime`、`completion`。**这些标签大概率已陈旧**（两轮大缺口已闭；如 `completion` glm5.2 审计结论本就是「一致」）。本任务先**重审确认真实剩余缺口**，再关闭，最后终局门。

## 三阶段

### Phase A — 重审 5 partial 集群（先做，便宜）
对每个集群：源码级 Python↔Rust 审计 → 判定真实剩余缺口（consistent / partial / gap）→ 更新 matrix 标签。重点核对：
- `completion`：glm5.2 审计结论「一致」，为何 matrix 仍 partial？（matrix 备注「Per-provider execution adapter parity deferred」——adapters 已验证完成，应升 complete）。
- `terminal_runtime`：kimi 闭小缺口后是否已 complete？（原 deferred：「namespace/state 集成」）。
- `daemon_lifecycle`：comms_recover+callbacks 已闭；原 deferred「full apply/reload patch flows」「provider-specific lifecycle tests」是否还在？
- `providers`：claude_registry/opencode/active_runtime 已闭；剩余 minor 项？
- `cli_entrypoint`：原 deferred 项？
产出：`research/finishline-audit.md`（真实缺口清单）。

### Phase B — 关闭确认的真缺口（TDD 分片）
仅对 Phase A 判定为「真缺口」的项，按 comms_recover/callbacks 范本（先审→TDD→分片提交→matrix 更新）。若 Phase A 发现多数已是 0，则 B 很轻。

### Phase C — 终局门
`cargo test --workspace -- --test-threads=1` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --check` 全绿；matrix 全 complete；签字。最后再决定 697 个 stub 镜像去留（作者指令：功能对齐后再动）。

## 验收标准

- 5 partial 集群逐个有审计判定（consistent/partial/gap）+ 证据。
- 真缺口全部关闭（Python↔Rust 行为 parity + Rust 测试覆盖，全绿）。
- `cargo test --workspace -- --test-threads=1` 全绿；clippy `-D warnings` 0；fmt clean。
- `plans/rust-python-test-parity-matrix.md` 标签与真实状态一致（该升 complete 的升）。

## 范围外（stop-rule）

- 不改 ccb-mailbox 线协议/控制面契约（加 pub 方法 OK）。
- 不改 provider hook/settings 注入、tmux namespace/pane identity 核心、`Phase2Services`/`ExecutionService` trait 契约。
- stub 镜像文件保持不动（功能对齐后再决定）。
- **不碰 luck 的并行任务**：`cli-ask-install-restart`、`daemon-startup-foreground-wait`、`providers-catalog-health-restore`、`e2e-terminal-edge`。

## 方法论（glm5.2/kimi 已验证）

先审后写（枚举 Python 测试/函数 → 映射 Rust 实现+测试 → 判定 → 审计 note）→ 缺口关闭 TDD 分片。范本：`research/consistency-audit-{completion,daemon-lifecycle}.md` + `research/comms-recover-impl-plan.md`（在 providers-daemon-deep 任务 dir）。

## 参考

- `.trellis/tasks/06-24-py2rust-providers-daemon-deep/HANDOFF-KIMI.md` + `stub-triage.md` + `research/*`。
- `plans/rust-python-test-parity-matrix.md`。
- Python 参考：`lib/ccbd/`、`lib/provider_backends/`、`lib/cli/`、`lib/completion/`。
