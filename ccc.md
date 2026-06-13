# 外部开发组开发手册与边界守则（Daprov2 分仓候选版）
> Version: v1.4-daprov2-revision
> Date: 2026-06-05
> Default external writable root: `/mnt/f/wt/daprov2/o12p21`
> Internal review/runtime root: `/mnt/g/wt/daprov2/o11`
> Applicable group: 外部开发组 / 外包协作开发组 / 第三方受限实现组
> Engineering baseline: DDD-first, TypeScript-first where applicable, evidence before trust
> UI baseline: `ui-ux-pro-max` for Web UI work
> Verification baseline: duration required; unsupported deterministic `>999ms` fails
> Status: candidate-only; internal Development Group owns correction, formal sync, runtime, and final verification

---

## 0. 本版核心变化
本版替代旧 `/mnt/f/wt/dapro/o12p21` 口径，当前外部候选根目录是：

```text
/mnt/f/wt/daprov2/o12p21
```

旧路径 `/mnt/f/wt/dapro/o12p21`、`/mnt/g/wt/dapro/o11`、`/mnt/d/dapro` 仅可作为历史参考，默认不得用于新开发、编译、运行或交付声明。

外部开发组交付物永远是 `candidate_ready_for_internal_review`。内部开发组负责审核、纠错、修正、验证、正式合并、runtime 刷新、远端推送和最终关闭。外部开发组不接收“返修要求”作为正式流程；内部审查中发现问题由内部组原地修复或吸收。

---

## 1. 一句话总则
外部开发组只在当前任务明确授权的 Daprov2 分仓目录内工作，产出候选补丁和证据；不得打开平台真相、S-series 写入、结算、资金、发票、税务、合同签署、auth 权限、OpenFGA/Keycloak、runtime lease、Testing entry 或正式远端推送。

---

## 2. 当前分仓工作边界
每个任务必须先列明涉及的仓库。Daprov2 当前主要目录包括：

```text
auth_web
sdk
platform-core
platform-admin
platform-foundation-ops
platform-frontend-shared
business-common
annotation-bff
annotation-business-assets
annotation-plaza
memp
meme
memc
settlement-center
business-contract-settlement-legacy
business-integration-assets
business-ops-governance
project-governance-docs
```

`platform-runtime-bridge` 是 `business-common/platform-runtime-bridge` 的兼容 symlink，不是独立仓库。

四个标注业务前端必须保持分离：

```text
memp = 标注师个人端
meme = 标注企业端
memc = 发包企业端
annotation-plaza = 标注平台运营方端
```

不得把 `memp` 和 `meme` 重新合并为旧 Memui 语义；不得把 `memc` 称为旧 Platui；不得把 `annotation-plaza` 变成万能业务审核方。

---

## 3. 默认可写与只读范围
默认可写根：

```text
/mnt/f/wt/daprov2/o12p21
```

但只有当前任务明确授权的仓库、文件或目录可写。常见授权范围包括：

```text
<repo>/src/**
<repo>/tests/**
<repo>/scripts/**
<repo>/docs/external-delivery/**
```

默认只读或禁止改动：

```text
/mnt/g/wt/daprov2/o11
/mnt/d/dapro-ass
/mnt/d/report
/mnt/d/dapro_ops
/mnt/d/gsd
/mnt/d/dapro
/mnt/g/wt/dapro/o11
/mnt/f/wt/dapro/o12p21
```

即使位于可写根内，以下对象也必须有任务明确授权才可修改：schema、migration、seed、OpenFGA、Keycloak、permission contract、SDK 公共契约、API payload/error code、auth/session、runtime/ingress/env/secret、CI workflow、package manager lockfile、共享验证脚本、Testing entry、evidence root 约定。

---

## 4. Git 与同步规则
默认允许：

```bash
git status --short
git diff --stat
git diff
git log --oneline -n 5
```

以下动作必须明确授权：

```text
git add / commit / pull / fetch / rebase / merge / push / reset / clean
```

外部开发组不得推送远端，不得 formal sync，不得更新正式 runtime。交付报告必须写明：

```yaml
formal_sync_status: not_synced_by_external_group
handoff_required: Internal Development Group review required
```

---

## 5. DDD 与 truth-promotion 硬规则
非平凡业务任务必须先锁定：

```yaml
ddd_contract:
  owning_repository:
  bounded_context:
  task_type: command | query | event | infra | refactor | doc-only
  write_or_read: write | read | read-only-doc | helper-only
  business_goal:
  aggregate_or_process:
  idempotency_strategy:
  observability_scope:
  test_scope:
  allowed_files_or_dirs:
  non_goals:
```

业务事实进入生产有效状态必须满足链路：

```text
BFF field -> business meaning -> truth owner -> evidence -> canonical read model -> candidate -> admission -> command -> production state -> receipt/audit
```

缺任一环节时，UI 只能显示：`candidate only`、`pending authorization`、`pending platform confirmation`、`blocked`、`read-only` 或 `not open`。不得伪装为生产可用。

BFF/view model 只做表面适配，不得承载可复用业务真相。BFF 不得直接访问 DB/ORM/repository/internal schema，不得重写 permission、billing、lifecycle、risk、approval、quota、settlement、contract 或 audit 规则。

---

## 6. 12 条通用开发纪律
1. Think Before Coding：先说明假设，卡住时问，不确定时停。
2. Simplicity First：最小实现，不做 speculative abstraction。
3. Surgical Changes：只改任务必须改的内容，不顺手重构。
4. Goal-Driven Execution：定义成功标准，验证后再声称完成。
5. Model for Judgment Only：模型做判断、归纳、草拟；确定性转换交给代码/工具。
6. Token Budgets Are Binding：接近预算要 checkpoint，不静默透支。
7. Surface Conflicts：冲突规则选更新/更权威/更可验证者，并记录另一方。
8. Read Before Writing：改代码前读 exports、callers、shared utilities、conventions。
9. Tests Verify Intent：测试要证明业务意图，不只证明表面输出。
10. Checkpoint Significant Steps：重要步骤后记录已做、已验、未完。
11. Match Codebase Conventions：遵守现有风格，不私自另起体系。
12. Fail Loud：跳过、不确定、未验证时不得说 completed 或 tests pass。

---

## 7. 代码影响集与尺寸门槛
每个代码包必须列出：

```yaml
code_impact_set:
  touched_source_files:
  adjacent_existing_source_files:
  split_result_files:
  exempt_docs_or_tests:
  third_party_standard_code_readonly:
```

`code impact set` 内每个项目自研生产/源代码文件必须满足：

```text
line_count <= 400
byte_size <= 20480
```

测试、fixture、mock、proof、文档、报告可豁免尺寸，但不得隐藏生产逻辑。若授权边界内的影响集生产文件超限，必须拆分并保持行为一致；若超限文件在未授权路径，报告 `blocked by boundary` 并给出拆分方案。

标准第三方代码只读：不得修改 `node_modules`、vendor、第三方 SDK 源码、框架标准代码、上游生成代码。需要适配时只能在项目自研 adapter/wrapper/ACL/config/type declaration 中处理。

---

## 8. 999ms 验证耗时规则
所有验证命令必须记录耗时：

```yaml
verification:
  - command:
    result: pass | fail
    duration_ms:
    duration_source: shell-time | script-output | test-runner-output | manual-timer
    threshold_status: pass | fail | justified-over-999ms
    justification_if_over_999ms:
```

包级 deterministic verifier、unit test、domain test 超过 `999ms` 且没有权威或理论支撑，视为失败，必须拆分、缩小或修复。

以下类型可以 `justified-over-999ms`，但必须写明原因：type-check、build、browser/Playwright/Chrome DevTools、runtime smoke、Docker/container、DB/network/provider/cross-service check。

不得默认跑全量测试。使用最小足够验证集证明本任务声明。

---

## 9. Web 前端与 UI/UX
涉及页面、组件、导航、布局、字体、间距、表单、交互、可访问性、响应式、浏览器 proof 的任务，默认使用 `ui-ux-pro-max` 或等价 UI/UX 检查清单。

前端页面必须使用人能理解的业务语言，不得直接暴露：

```text
read model
readback
BFF
package
candidate_ready_for_internal_review
```

应表达为：当前状态、下一步、缺失证据、等待授权、等待平台确认、未生效、未开放、关闭边界。

---

## 10. 大型任务与 subagent
出现任一情况即为大型任务：涉及 3 个以上源文件、2 个以上层级、2 个以上仓库、前后端联动、文件拆分、复杂 UI、runtime/browser proof、权限/结算/契约/S-series 边界、或任一验证预计超过 999ms。

大型任务必须先规划，再按互不冲突的写入范围调用 subagent。subagent 只能在授权文件内工作，不得扩权、不得改变业务目标、不得把最终责任转移给主执行者。主执行者必须审查、合并、验证并回收 subagent 结果。

subagent 完成后必须及时关闭或释放会话资源；其结论必须落入交付报告、验证脚本、治理文档或代码注释中的一种，不得只停留在聊天记录。

---

## 11. 安全与运行时
不得读取、复制、打印、提交 live secret、token、cookie、session、credential、`.env`。安全开关必须显式注入；未注入必须 fail closed。

外部开发组默认不得启动、保持、清理共享 runtime；不得把本地服务声明为正式 runtime；不得解锁 Testing Group。若任务允许本地验证服务，必须声明为：

```text
local external-worktree verification only
```

runtime 相关报告必须写明 port、PID、command、cwd、source path、smoke endpoint。

---

## 12. 外部交付报告最低字段
```yaml
primary_result: candidate_ready_for_internal_review
package:
bounded_context:
task_type:
write_or_read:
allowed_writable_root: /mnt/f/wt/daprov2/o12p21
repositories_touched:
created_or_modified_files:
ddd_contract:
truth_ownership: # required for BFF/view model/read model work
code_impact_set:
verification:
closed_boundaries:
  schema_migration_opened: false
  OpenFGA_changed: false
  Keycloak_changed: false
  permission_contract_changed: false
  SDK_contract_changed: false
  S_series_write_opened: false
  platform_truth_write_opened: false
  settlement_write_opened: false
  payment_invoice_tax_opened: false
  contract_signing_opened: false
  runtime_lease_opened: false
  testing_entry_unlocked: false
SP:
RC:
TV:
PR:
formal_sync_status: not_synced_by_external_group
handoff_required: Internal Development Group review required
remaining_risks:
```

---

## 13. Stop Conditions
立即停止并报告 `blocked by boundary`：

- truth owner 无法确认；
- BFF 需要自行计算权限、计费、生命周期、风控、审批、审计、结算或契约状态；
- 需求依赖 platform canonical read model，但该 read model 不存在；
- 需要修改 schema、OpenFGA、Keycloak、permission contract、SDK contract、S-series、settlement ledger、payment、invoice、tax、contract signing、secret、runtime lease 或 Testing entry；
- 需要删除用途不明代码；
- 需要修改标准第三方源码；
- unsupported deterministic verification `>999ms` 且无法在授权边界内修复。

---

## 14. Preferred External Task Shape
外部任务应是“块级闭环任务”，不是碎片页面修补。推荐格式：

```yaml
package:
objective:
owning_repositories:
allowed_files_or_dirs:
required_local_references:
required_external_references:
non_goals:
ddd_contract:
truth_ownership_expected:
ui_ux_scope:
verification_required:
report_path:
```

外部组可参考 CVAT、Label Studio、OpenSign/Kaifangqian、Odoo、Microsoft/Azure、Google Cloud 等成熟系统，但只能学习模式并转译到 Daprov2 truth-owner 语义；外部系统不是 Daprov2 的业务真相权威。

---

## 15. Experience and Recommendations
- 外部开发越快，边界越要硬；候选交付不能绕过内部 truth-owner 与 verification。
- 不要再用旧 Dapro 路径生成新任务。
- UI 能操作必须有 command/authorization/receipt；否则明确未开放。
- 结算、契约、认证、S-series 是平台中心能力，不是标注业务页面的散装按钮。
- 先闭合冷启动、常态任务、QA、结算依据这条生产链，再扩展新功能。
