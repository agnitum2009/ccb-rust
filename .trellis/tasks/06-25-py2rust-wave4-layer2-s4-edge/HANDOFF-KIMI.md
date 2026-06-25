# Handoff — Wave 4 Layer 2 S4 (edge) → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> 测试环境: `/mnt/d/dapro-ass`（agent1/agent2 codex + agent3 claude）

## 背景
A/B 已 live 通过。B 已修 `preview_text` 多字节 UTF-8 截断（char boundary）+ inbox 规范载荷解析（commit `1951583a` P1）。S4 在此基础上加压更多边界。

## 任务范围（逐项 live 验证 + 证据）
1. **空/畸形消息**：`ccbr ask agent3 ""`、带非 JSON / 截断载荷的 ask → 优雅错误
2. **超长 prompt**：构造 ≥10KB 文本 ask → 处理或明确上限
3. **并发**：同一 agent 连发 3 个 ask → `ccbr queue --detail <agent>` 验证排队顺序
4. **cancel**：发起 ask 取 job-id → `ccbr cancel <job-id>` → trace 确认 cancelled、agent idle
5. **timeout**：构造不响应场景（临时断 provider）→ 优雅超时
6. **auth 失败**：临时移除 `$PROJECT/.ccbr/runtime/agent1/home/auth.json` → `ccbr restart agent1` → 优雅错误，无 hang
7. **reload 中途**：ask 处理中 `ccbr reload` → 状态一致
8. **UTF-8 边界**：ask 含 emoji / 4 字节 CJK / 组合字符 → 回复渲染无 panic
9. **特殊字符**：prompt 含 引号/反引号/换行/反斜杠 → 正确传递

## 运行方式（同 S3）
```bash
export CCBR_SOURCE_RUNTIME_OK=1
cd /home/agnitum/ccb/rust
cargo build -p ccbr-cli -p ccbr-daemon -p ccbr-providers
PROJECT=/mnt/d/dapro-ass
timeout 5 ./target/debug/ccbr --project "$PROJECT" shutdown 2>/dev/null; sleep 2
pkill -f 'ccbrd --project /mnt/d/dapro-ass'; sleep 1
rm -f /run/user/0/ccbr-runtime/ccbrd-*.sock /run/user/0/ccbr-runtime/tmux-*.sock
RUST_LOG=info nohup ./target/debug/ccbrd --project "$PROJECT" > /tmp/ccbrd.log 2>&1 & disown
sleep 3
./target/debug/ccbr --project "$PROJECT" start
```

## 关键代码位置
| 组件 | 文件 |
|---|---|
| preview_text UTF-8 截断 | `ccbr-cli/src/render.rs`（commit `1951583a`） |
| inbox/queue parser（--detail 前置） | `ccbr-cli/src/entry.rs` + `render.rs` |
| cancel | `ccbr-cli` cancel 命令 + `ccbr-daemon` job 服务 |
| reload | `ccbr-cli` reload + `ccbr-daemon` config |
| codex auth 物化 | `ccbr-provider-profiles/src/codex_home_config.rs:materialize_auth_file()` |

## 完成判定
9 类边界全部 live 证据 + 优雅处理；发现的新 bug 独立修复并补单测。修复同步 ccb-legacy / 产品仓走既有流程。
