#!/usr/bin/env bash
# ccbr 测试资源回收 — 测试完毕后运行。
# 铁律：只清 ccbr(Rust) 测试产物，绝不碰 ccb(Python 生产: n14/o13/e-contract + /tmp/ccb-*)。
# 用法: bash scripts/ccbr-test-cleanup.sh
set -u
RUNTIME=/run/user/0/ccbr-runtime
REPO="${CCBR_REPO:-/home/agnitum/ccb}"

# 1. kill 所有 ccbr 测试 tmux server（runtime socket 上的会话）
for s in "$RUNTIME"/tmux-*.sock; do [ -S "$s" ] && tmux -S "$s" kill-server 2>/dev/null; done
# 2. 删死 ccbrd / tmux socket 文件
rm -f "$RUNTIME"/ccbrd-*.sock "$RUNTIME"/tmux-*.sock 2>/dev/null
# 3. 删 /tmp 下 ccbr 测试日志/pid（不含 ccb-* —— 那是 Python 生产）
rm -f /tmp/ccbrd*.log /tmp/ccbrd-*.pid /tmp/ccbr-*-check.log /tmp/ccblegacy-check.log 2>/dev/null
rm -rf /tmp/ccbr-ensure-test 2>/dev/null
# 4. 删含 ccbrd.sock 的 mktemp scratch 目录
for d in /tmp/.tmp*; do [ -e "$d/ccbrd.sock" ] && rm -rf "$d"; done
# 5. prune 陈旧 git worktree
git -C "$REPO" worktree prune 2>/dev/null
echo "[ccbr-test-cleanup] ccbr 测试残留已回收（ccb 生产未触碰）。"
