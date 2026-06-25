# Handoff — ccb-legacy 非 rust/ 同步 → kimi2.7

> Prepared for: **kimi2.7** | From: glm5.2 | Branch: `python-rust/rolepacks-versioning-translation`
> ccb-legacy 分支 tip: `547e91e5`（rust/ 已同步）。独立血系，永不与 ccbr 合并。

## 背景（必读）
用户明确：**ccbr 与 ccb-legacy 只是 ccb 命名不同、其他代码一致；ccb-legacy 跟 ccbr 永远不合并；是"与源系统保持高度兼容 + 模块合并"的前提。** 故 ccb-legacy = ccbr 的反向重命名双生血系。rust/ 已同步（`547e91e5`，反向重命名 ccbr→ccb，`cargo check` 干净）。本任务补**非 rust/**。

## 已验证的反向重命名方法（glm5.2 实测，rust/ 用过）
```bash
git worktree add --detach /tmp/ccbr-twin HEAD
cd /tmp/ccbr-twin
# 内容（文本文件，排除 .git/target）
find . -type f -not -path './.git/*' -not -path './target/*' \
  -exec grep -Il '' {} + | xargs -r sed -i 's/ccbr/ccb/g; s/CCBR/CCB/g'
# 路径（深度优先，先文件后目录）
find . -depth -not -path './.git/*' -not -path './target/*' | while IFS= read -r p; do
  d=$(dirname "$p"); b=$(basename "$p"); nb=${b//ccbr/ccb}; nb=${nb//CCBR/CCB}
  [ "$b" != "$nb" ] && mv "$p" "$d/$nb"
done
```
安全点：反向 `s/ccbr/ccb/g` 对"旧 ccb 保留区"（rolepacks.rs `adapters/ccb`、`hosts=ccb`、`CCB.md`）无害——它们从未变 ccbr。

## 本任务范围（只做非 rust/）
1. **列差异**（排除 rust/、.trellis/）：
   ```bash
   git diff --stat ccb-legacy..HEAD -- docs/ scripts/ plans/ '*.md' '*.sh' '*.toml'
   ```
2. **逐类判定是否同步**：
   - docs/scripts/plans（通用工程文档/脚本）→ 同步（反向重命名）
   - 产品仓专属（`ccb-rust-prod-workflow.md`、`agnitum2009/ccb-rust` 引用、产品仓 README）→ 保留 ccb-legacy 现状或适配，**不盲目同步**
   - `.trellis/` → **不同步**（ccb-legacy 自有任务管理）
3. **应用**：把判定的非 rust/ 文件从 HEAD 反向重命名后落到 ccb-legacy worktree
4. **验证**：`git grep -n "ccbr" ccb-legacy -- docs/ scripts/ plans/` 应为零（或仅产品仓白名单）
5. **提交** ccb-legacy 分支

## ccb-legacy worktree 操作
```bash
cd /home/agnitum/ccb
git worktree add /tmp/ccb-legacy-port ccb-legacy   # 或复用现有
# …同步操作…
git -c user.name="CCB Codex" -c user.email="ccb-codex@local" \
  commit -m "sync(ccb-legacy): non-rust reverse-renamed ccbr->ccb"
git worktree remove /tmp/ccb-legacy-port --force
```

## 完成判定
非 rust/ 对齐（适用部分）、零 ccbr 泄漏、ccb-legacy 独立血系保持（`git merge-base --is-ancestor ccb-legacy HEAD` = false）、提交完成。
