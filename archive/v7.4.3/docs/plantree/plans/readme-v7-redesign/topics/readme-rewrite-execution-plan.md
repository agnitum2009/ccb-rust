# README Rewrite Execution Plan

Date: 2026-05-26

Role: Topic
Status: Active planning
Read when: Preparing the concrete `README_zh.md` and `README.md` rewrite patch
Related: [readme-implementation-blueprint.md](readme-implementation-blueprint.md), [v7-interface-and-basic-functions.md](v7-interface-and-basic-functions.md), [tmux-onboarding-runbook.md](tmux-onboarding-runbook.md), [open-questions.md](../open-questions.md)

## Purpose

Turn the v7 README redesign plan into an edit-ready scheme. This file is the
working checklist for the next patch that will rewrite `README_zh.md` first and
then update `README.md` for parity.

## Rewrite Strategy

Use a staged rewrite rather than small local edits. The current README top half
is v6/v7 mixed and the release history dominates the page, so local patching
would preserve too much old structure.

Recommended approach:

1. Rewrite the top-level navigation and first-read path in `README_zh.md`.
2. Keep install, uninstall, community, and editor sections only after they are
   moved into the new order and updated for v7 wording.
3. Replace the old showcase image reference with regenerated real terminal
   screenshots under `assets/readme_v7/`.
4. Move older release history out of the default reading path; keep latest v7
   highlights and link to `CHANGELOG.md`.
5. Mirror the final Chinese structure into `README.md`.

## Target Chinese README Patch Shape

| Current Area | Action | Target Content |
| :--- | :--- | :--- |
| Header/nav | Replace | New title, badges, language links, and navigation matching the new sections. |
| `为什么 CCB` | Replace | First explain why multi agents are needed, then compare multi-agent approaches. |
| Old showcase image/GIFs | Replace | Use regenerated real terminal hero screenshots under `assets/readme_v7/`; do not keep separate local/detail screenshots in the first pass. |
| `最新亮点` | Compress | Keep only current v7 highlights and link full history to `CHANGELOG.md`. |
| `启动和退出` | Rewrite | Rename to daily operation or quick start; separate CCB commands from tmux shortcuts. |
| one-line tmux copy/paste | Replace | Add `tmux 常规操作` with `<prefix>` convention, English input-method note, copy/paste, pane/window operations, and folded advanced keys. |
| `配置控制` | Rewrite | Explain config precedence, compact layout, v7 `[windows]`, sidebar options, worktree, model/key/url, and `ccb config validate`. |
| `配置设计 Skill` | Rewrite | Use current `ccb-config` spelling; explain propose/confirm/write/validate/restart flow. |
| `如何安装` | Rewrite around release-first path | Use release as the default recommended path; keep source checkout install as development/fallback guidance. |
| `如何使用` | Rewrite | Keep `/ask`, `$ask`, callback, and `--silence`; add short practical examples. |
| `编辑器集成` | Keep but compress | Keep one screenshot and one concise workflow paragraph. |
| `新版本记录` | Move/link | Keep latest v7 summary plus `CHANGELOG.md`; do not keep old history as the main README body. |
| `社区 / 致谢` | Update | Keep community contact and add explicit thanks to `tmux-agent-sidebar` with link. |

## Proposed Final Section Order

1. Header and navigation.
2. `为什么需要多 agents`
3. `不同多 agents 方案怎么选`
4. `CCB 是什么`
5. `CCB v7 界面速览`
6. `90 秒快速开始`
7. `日常操作`
8. `tmux 常规操作`
9. `配置你的 agent 团队`
10. `使用 ccb-config skill 生成配置`
11. `Agent 之间如何协作`
12. `编辑器工作流`
13. `常见问题和排障`
14. `安装、环境要求和卸载`
15. `社区和版本记录`

Rationale:

- Multi-agent value comes before product commands.
- Screenshot and v7 interface explanation appear before tmux/config details.
- New users get quick start before advanced config.
- tmux is taught as useful keyboard operations, not as a prerequisite wall.
- Install can stay visible in quick start while the full install/platform notes
  live later.

## Visible Versus Folded Split

Keep visible:

- short single-agent versus multi-agent table;
- short Claude Code native / OpenHive / CCB comparison;
- one v7 screenshot;
- v7 interface region/function table;
- 90-second quick start;
- common CCB commands;
- `tmux 常规操作` primary shortcut table;
- one compact config example and one v7 `[windows]` example;
- one `ccb-config` prompt example;
- `/ask` / `$ask` / callback summary.
- brief `tmux-agent-sidebar` credit near the sidebar explanation or credits
  section.

Fold:

- expanded multi-agent comparison;
- source links for external projects;
- screenshot capture notes;
- sidebar state detail and "what is not shown";
- advanced tmux search/history commands;
- per-agent API/model details beyond one example;
- migration from old compact config to `[windows]`;
- detailed troubleshooting and diagnostics;
- old release history.

Credits:

- Add thanks to [tmux-agent-sidebar](https://github.com/hiroppy/tmux-agent-sidebar)
  in the v7 sidebar/interface explanation and/or final credits section.
- Keep the credit concise; do not explain sidebar internals in the public README.

Release-history placement is already decided in
[../decisions/002-readme-publication-defaults.md](../decisions/002-readme-publication-defaults.md):
the README keeps current v7 highlights and links full history to `CHANGELOG.md`.

## Required README Text Blocks

Use or adapt these blocks when writing `README_zh.md`:

- `为什么需要多 agents`: from
  [multi-agent-positioning-and-comparison.md](multi-agent-positioning-and-comparison.md).
- `CCB v7 界面速览`: from
  [v7-interface-and-basic-functions.md](v7-interface-and-basic-functions.md).
- `tmux 常规操作`: from
  [tmux-onboarding-runbook.md](tmux-onboarding-runbook.md).
- `配置你的 agent 团队`: from
  [readme-implementation-blueprint.md](readme-implementation-blueprint.md#basic-config-section-design).
- `使用 ccb-config skill`: from
  [readme-implementation-blueprint.md](readme-implementation-blueprint.md#ccb-config-skill-section-design).

## Clarification Dependency

Do not block all writing on every open question. Split them like this:

Already decided by
[../decisions/003-readme-final-publication-choices.md](../decisions/003-readme-final-publication-choices.md):

- final README media should use real terminal screenshots, not the current
  text-rendered annotated draft;
- release install/update is the default recommended path;
- native Windows support only applies to the v5 line; newer versions are not
  supported natively on Windows.

Resolved for the first README patch:

- Multi-agent comparison wording is fixed: OpenHive is described as a generated
  workflow harness, while CCB also supports complex workflows through explicit
  configuration of agents, windows, worktrees, memory, model/API choices, and
  ask/callback routes.
- Detailed troubleshooting commands are not required in the first README patch.
  Keep troubleshooting lightweight and avoid publishing unstable diagnostics.
- A concrete demo scenario is deferred; the first README patch can use interface
  screenshots and explanatory text without a task demo.
- Bilibili/video/audio planning is deferred because no video is required for
  this patch.

## Install And Platform Wording

Use this direction in the README draft:

- Default path: install or update to the latest stable release.
- Source checkout path: development/fallback path, folded or placed after the
  recommended release path.
- Linux/macOS/WSL: current recommended environment for new versions.
- Native Windows: legacy v5-only support; current/new v7 line is not supported
  natively on Windows. Recommend WSL for Windows users who want current versions.
- Platform badge should not say `Windows` without qualification for v7.

## Verification Plan For The README Patch

After editing public README files:

1. Check local Markdown links and asset paths.
2. Verify image paths under `assets/readme_v7/`.
3. Validate documented compact and `[windows]` config snippets with
   `ccb config validate` in temporary projects where practical.
4. Search for stale current-language references:
   - `CCB v6`
   - `ccb_config` outside historical changelog text
   - `.ccb_config/ccb.config` as current user guidance
   - `ccb update 6` in current examples
5. Confirm Chinese and English README section parity.

## Risks

- If the README keeps too much old release history inline, the new v7 onboarding
  will still be buried.
- If the hero screenshot remains text-rendered, some users may read it as a
  mockup rather than a live terminal screenshot. This can be solved with a
  caption or later replacement.
- If install/update wording is too optimistic across native Windows, WSL, and
  sidebar helper compatibility, the README may overpromise platform support.
