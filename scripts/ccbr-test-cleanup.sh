#!/usr/bin/env bash
# ccbr 测试资源回收 — 测试完毕后运行。
# 铁律：只清 ccbr(Rust) 测试产物，绝不碰 ccb(Python 生产: n14/o13/e-contract + /tmp/ccb-*)。
# 用法: bash scripts/ccbr-test-cleanup.sh
set -u
RUNTIMES=(/run/user/0/ccbr-runtime /tmp/ccbr-runtime)
REPO="${CCBR_REPO:-/home/agnitum/ccb}"
TEST_ROOTS="${CCB_TEST_ROOTS:-${CCBR_TEST_ROOTS:-}}"

# 1. kill 所有 ccbr 测试 tmux server（runtime socket 上的会话）
for runtime in "${RUNTIMES[@]}"; do
  for s in "$runtime"/tmux-*.sock; do [ -S "$s" ] && tmux -S "$s" kill-server 2>/dev/null; done
done
# kill orphan tmux servers when the socket path is already stale/missing.
for pid in $(ps -eo pid=,comm=,args= | awk '$2 == "tmux" && $0 ~ /\/ccbr-runtime\/tmux-/ {print $1}'); do
  kill "$pid" 2>/dev/null || true
done
# kill leaked debug ccbrd test daemons.  Match the actual process name so the
# caller shell is not killed when its command line mentions target/debug/ccbrd.
for pid in $(ps -eo pid=,comm=,args= | awk '$2 == "ccbrd" && $0 ~ /target\/debug\/ccbrd/ {print $1}'); do
  kill "$pid" 2>/dev/null || true
done
# 2. 删死 ccbrd / tmux socket 文件
for runtime in "${RUNTIMES[@]}"; do
  rm -f "$runtime"/ccbrd-*.sock "$runtime"/tmux-*.sock 2>/dev/null
  rm -rf "$runtime"/locks 2>/dev/null
done
# 3. 删 /tmp 下 ccbr 测试日志/pid（不含 ccb-* —— 那是 Python 生产）
rm -f /tmp/ccbrd*.log /tmp/ccbrd-*.pid /tmp/ccbr-*-check.log /tmp/ccblegacy-check.log 2>/dev/null
rm -rf /tmp/ccbr-ensure-test 2>/dev/null
# 4. 删含 ccbrd.sock 的 mktemp scratch 目录
for d in /tmp/.tmp*; do [ -e "$d/ccbrd.sock" ] && rm -rf "$d"; done
# 5. 清测试项目的 ccbr 状态根；绝不碰 .ccb / ccb / ccb-legacy。
IFS=':' read -r -a test_roots <<< "$TEST_ROOTS"
for root in "${test_roots[@]}"; do
  [ -n "$root" ] || continue
  project_id="$(ROOT_PATH="$root" python3 - <<'PY'
import hashlib, os
from pathlib import Path
root = Path(os.environ["ROOT_PATH"]).expanduser()
try:
    raw = str(root.resolve())
except Exception:
    raw = str(root.absolute())
norm = raw.replace("\\", "/")
if norm.startswith("/mnt/") and len(norm) > 6 and norm[6] == "/":
    drive = norm[5].lower()
    norm = f"{drive}:/{norm[7:]}"
if len(norm) >= 2 and norm[1] == ":":
    norm = norm.lower()
print(hashlib.sha256(norm.encode()).hexdigest())
PY
)"
  [ -n "$project_id" ] || continue
  rm -rf "$HOME/.local/state/ccbr/projects/$project_id" "$HOME/.cache/ccbr/projects/${project_id:0:16}" 2>/dev/null
  # Project-local runtime/session artifacts created by live smoke tests.  Keep
  # .ccbr/ccbr.config and .ccbr/bin intact so explicit test workspaces remain
  # reusable, and never touch Python .ccb state.
  rm -rf "$root/.ccbr/runtime" "$root/.ccbr/shared-cache" 2>/dev/null
  rm -f "$root/.ccbr"/.codex-*-session "$root/.ccbr"/.claude-*-session "$root/.ccbr"/.gemini-*-session 2>/dev/null
done
# 6. prune 陈旧 git worktree
git -C "$REPO" worktree prune 2>/dev/null
echo "[ccbr-test-cleanup] ccbr 测试残留已回收（ccb 生产未触碰）。"
