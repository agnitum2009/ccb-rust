# Handoff — Wave 5 Python→Rust 1:1 parity 审计 → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> 项目主线：**1:1 用 Rust 替代 Python，确保参考 ccb 原样可运行 ccbr。**

## 两阶段（全量 → 聚焦）

### 阶段 1：全量 parity 扫描
- 基准：Python `lib/` 模块（见 prd 列表）。
- Rust 对照：`rust/crates/ccb-*`（25 crate；HEAD ccbr-* 与 ccb-legacy ccb-* 仅命名不同，行为一致，直接用 HEAD）。
- 既有仪表：`plans/rust-python-test-parity-matrix.md`（36KB，已部分映射 —— 先读它，补全 + 校验，勿重头）。
- **Python → Rust 映射起点**（逐个核实，可能有偏差）：
  | Python lib/ | Rust crate |
  |---|---|
  | agents, rolepacks | ccb-agents |
  | ccbd, heartbeat, maintenance_heartbeat | ccb-daemon / ccb-heartbeat |
  | cli, ask_cli | ccb-cli |
  | completion | ccb-completion |
  | jobs | ccb-jobs |
  | mailbox_kernel, mailbox_runtime | ccb-mailbox |
  | memory, project_memory | ccb-memory |
  | message_bureau | ccb-message-bureau |
  | opencode_runtime, provider_runtime, provider_execution | ccb-providers / ccb-stdio-runtime |
  | pane_registry_runtime | ccb-pane-registry |
  | project | ccb-project |
  | provider_backends/core/hooks/profiles/sessions | ccb-provider-* + ccb-providers |
  | runtime_env | ccb-runtime-env |
  | fault_injection | （定位：ccb-daemon 内 fault 模块？） |
  | provider_model_shortcuts.py / role_aliases.py / release_artifacts.py | 散落，逐一定位 |

- **每模块判定**（入矩阵 + research/parity-gaps.md）：
  - 状态：`done` / `partial` / `missing` / `behavior-drift` / `test-missing`
  - 证据：Rust 符号/文件路径 + 行为差异点 + 测试名

### 阶段 2：聚焦 → 拆任务
- 按 criticality 排序（核心 runtime 优先：daemon lifecycle / provider launch / mailbox+job 路由 / cli / heartbeat+recovery）。
- Top-N 高优先 gap 各拆一个 Wave 5 子任务（`task.py create --parent 06-24-py2rust-remaining-parity`），每个有 prd + 验收。

## 行为 parity 验证方法（"原样可运行"）
- 选关键路径，**同输入**跑 Python ccb 与 Rust ccbr，比对：pane 创建/布局、provider 启动、ask→reply→inbox、job 生命周期、heartbeat/recovery。
- Python 参考运行：本地已装 Python `ccb`（注意与 ccbr 区分）。
- Rust 运行：`CCBR_SOURCE_RUNTIME_OK=1` + 干净状态（同前述 HANDOFF）。

## 完成判定
矩阵全量刷新、gap 排序清单、Wave 5 子任务就绪、journal 记录。同步产品仓（ff-push）+ ccb-legacy（仅当本波改了 rust/ 代码，反向重命名）。
