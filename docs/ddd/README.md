# ccb DDD 启发式重构规划

> 状态：规划（planning）· 时机：Wave 4 parity 完全稳固（Layer 1+2 绿）之后
> 作者：glm5.2 · 2026-06-24
> 约束来源：`external_development_group_handbook_boundary_rules_v1.4.md`（项目无关原则提炼）

## 结论

**不做全量 classical DDD 重写**；做**选择性 type-driven 状态机/聚合重构**（Rust idiom 表达 DDD 思想）。理由：领域是基础设施/控制面（非业务域）；Rust 类型系统已覆盖大半 DDD 战术价值（enum=状态机、newtype=VO、所有权=invariant）；crate 边界已≈限界上下文；parity 刚达成（重构高风险）。

## 状态机现状评估（2026-06-24 实测散布度）

| 状态机 | enum | 赋值点 | 消费点 | 散布 | 优先级 |
|---|---|---|---|---|---|
| **JobStatus**（job 生命周期） | ✅ | **21** | **11** | 🔴 高 | **P0** |
| **InboundEventStatus**（mailbox 事件） | ✅ | **16** | — | 🔴 高 | **P1** |
| **CompletionStatus**（完成决策） | ✅ | — | **12** | 🟠 中 | **P2** |
| **RecoveryState**（comms_recover） | ❌ 隐式 JSON 字符串 | — | — | 🟠 中 | **P1**（类型化） |
| AttemptState / CallbackEdgeState / LeaseState / MessageState / AgentState | ✅ | 2 | — | 🟢 低 | P3 |

## 重构方案（按优先级）

### P0：JobStatus 状态机 + Job 聚合（最散，最高 ROI）
- 现状：21 处 `.status = JobStatus::X` 散在 dispatcher/submission_service/handlers/mailbox；转移规则各处自行判断，无集中校验。
- 目标：`Job` 聚合 + `fn transition(self, event) -> Result<JobStatus, IllegalTransition>`；非法转移 = 编译期或运行期错误；副作用（持久化/事件/reply delivery）集中。
- Rust idiom：`enum JobStatus` + `impl JobStatus { fn transition(self, e: JobEvent) -> Result<..> }`；newtype `JobId`。

### P1a：InboundEventStatus 状态机（mailbox 事件，16 处）
### P1b：RecoveryState 类型化（comms_recover 隐式字符串 → enum RecoveryOutcome）
### P2：CompletionStatus 决策流 + CompletionDecision 聚合
### P3：AttemptState / CallbackEdgeState / LeaseState / MessageState / AgentState（低散布，顺手统一为 transition fn 模式）
### 横切：防腐层（ACL）剥离 Python-port 结构债 + 统一语言（ubiquitous language）glossary

## 方法（Rust idiom，非 OOP DDD）

- 状态机：`enum State` + `impl State { fn transition(self, event) -> Result<State, IllegalTransition> }`；exhaustive match 保证完备。
- 聚合：持有状态的 struct + 仅通过方法改变状态 + 所有权保证独占（`&mut Job` 才能转移）。
- 值对象：newtype（`pub struct JobId(String)`）。
- 不引入新依赖（优先手写/标准库；可选 strum 但 ccb 现有无新依赖原则）。

---

## 重构约束（外部开发手册边界规则 v1.4，项目无关原则提炼）

以下约束从 handbook 提炼（忽略其 Daprov2 项目/目录/业务域），**每个状态机重构（P0-P3）必须遵守**。

### C1. DDD contract 先锁定（handbook §5）
开工前必须记录：
```yaml
ddd_contract:
  owning_crate:        # ccb-daemon / ccb-mailbox / ccb-completion
  bounded_context:
  aggregate_or_process:
  truth_owner:         # C2
  task_type: refactor
  allowed_files_or_dirs:
  non_goals:
  test_scope:
```

### C2. truth owner 明确（§5）
每个状态机**唯一权威状态持有者**。JobStatus：当前散在 dispatcher.rs（简化）+ mailbox stores + handlers——重构后必须指定**唯一 truth owner**（大概率 mailbox stores + Job 聚合）。不允许两处都是真相。truth owner 不明→停（C8）。

### C3. handler/BFF = 薄适配，不承载领域逻辑（§5）
daemon handler / CLI adapter / RPC 层 = 薄适配器：仅解析请求→调用 domain aggregate→序列化响应。**不得**在 handler 里做状态转移/invariant/业务决策。重构后：handler 调 `job.transition(event)?`，不直接 `job.status = X`。

### C4. Simplicity First + Surgical Changes（§6 #2/#3）
最小类型驱动改动；不做 speculative abstraction（不引入 event-sourcing/CQRS/工厂）。Surgical：只改该状态机必须改的 call-site，不顺手重构周边。

### C5. 文件尺寸门（§7）
重构产出的领域文件 **≤400 行 / ≤20480 字节**。超限：拆分（`job/transitions.rs` + `job/invariants.rs` + `job/events.rs`），保持行为一致。测试/fixture/doc 豁免但不隐藏生产逻辑。

### C6. 验证耗时记录（§8）
验证（`cargo test`）记录耗时。unit/domain 测试 **<999ms**（deterministic）。超 999ms 需 justification。不默认跑全量——用最小足够验证集证明该状态机。

### C7. 大型任务→规划+分 scope 执行+审查（§10）
P0（JobStatus）触及 21+ call-site / 多 crate = 大型任务。先规划（本文档）→ 按互不冲突 scope 分执行（每状态机一 scope）→ 主执行者审查+合并+验证+回收。subagent 结论必须落入代码/测试/文档，不只聊天。

### C8. Stop conditions（§13）
遇任一情况**停并报告 `blocked by boundary`**：
- truth owner 无法确认。
- 需修改外部契约（mailbox 线协议、Phase2Services/ExecutionService trait、provider hook/settings）——超出「仅改内部表达」授权。
- 需删除用途不明代码。
- 需修改标准第三方源码（用 adapter/ACL 隔离）。
- 验证 >999ms 且边界内无法修。

### C9. 通用纪律（§6 精选）
- **Read Before Writing**（#8）：改状态机前读其所有 call-site、exports、shared utilities、现有约定。
- **Tests Verify Intent**（#9）：测试证明状态转移的业务意图正确（合法转移通过、非法转移拒绝），不只表面输出。
- **Match Codebase Conventions**（#11）：用 Rust idiom，不另起 OOP 体系。
- **Fail Loud**（#12）：跳过/不确定/未验证不说 completed 或 tests pass。
- **Think Before Coding**（#1）：先说明假设，卡住时问。
- **Surface Conflicts**（#7）：冲突规则选更权威者，记录另一方。
- **Checkpoint**（#10）：每状态机重构后记录已做/已验/未完。

### C10. 交付报告（§12 精简）
每状态机重构附：
```yaml
primary_result: candidate_ready_for_internal_review
owning_crate:
bounded_context:
ddd_contract:
truth_owner:
allowed_files_or_dirs:
verification:   # 命令+结果+耗时+阈值
closed_boundaries:
  mailbox_protocol_changed: false
  ExecutionService_trait_changed: false
  provider_hook_changed: false
  tmux_namespace_core_changed: false
remaining_risks:
```

---

## 时机与风险

- **时机**：Wave 4 parity 完全稳固（Layer 1+2 绿）之后。parity 巩固期重构=自我破坏。
- **风险**：触及核心控制流→必须有强测试网（comms_recover 12 测试、daemon integration tests、Wave 4 e2e）兜底；每步 cargo test 绿。
- **分批**：P0 先行验证模式→再推 P1/P2。每批独立交付、可回退。
- **不碰**：mailbox 线协议语义、provider hook/settings、tmux namespace 核心、Phase2Services/ExecutionService trait 契约。

## 与 Wave 4 的关系

Wave 4 e2e 测试 = 状态机重构的测试网。顺序：**Wave 4 e2e 绿 → DDD 重构（P0→P3）**。

## 后续文档（本目录，Wave 4 绿后写）

- `state-machine-inventory.md`：逐状态机的 call-site 清单 + 迁移目标。
- `job-aggregate-design.md`：P0 Job 聚合 + transition fn 具体设计。
