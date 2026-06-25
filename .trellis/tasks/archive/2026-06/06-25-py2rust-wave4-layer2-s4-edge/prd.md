# Wave 4 Layer 2 S4: edge 边界 live e2e

## Goal
真实环境覆盖 S4 边界与错误路径：空/畸形消息、超长 prompt、并发 ask、cancel/timeout、auth 失败、配置 reload 中途、多字节 UTF-8 边界、特殊字符。

## Requirements
- 空消息/畸形消息 ask → 优雅处理（明确错误，无 panic、无 stuck job）
- 超长 prompt（≥10KB）→ 处理或明确拒绝
- 并发 ask 同一 agent → 正确排队（`ccbr queue --detail`）
- cancel 中途 job（`ccbr cancel <job-id>`）→ job 取消、agent 释放
- timeout → 优雅
- auth 失败（临时移除隔离 HOME `auth.json`）→ 优雅错误，无 hang
- 配置 reload 中途（`ccbr reload`）→ 无状态损坏
- 多字节 UTF-8 边界（emoji、组合字符、4 字节 CJK）→ 无 panic（扩展 B）
- 特殊字符（引号/反引号/换行/反斜杠）→ 正确传递

## Acceptance Criteria
- [ ] 每个边界场景有 live 证据，均优雅处理（无 panic/stuck/丢消息）
- [ ] UTF-8 边界（含 emoji/4 字节）无 panic
- [ ] cancel/timeout 路径验证通过
- [ ] auth 失败与 reload 中途验证通过

## Notes
- PRD-only 可启动。运行方式同 S3 HANDOFF。代码位置见 HANDOFF-KIMI.md。
