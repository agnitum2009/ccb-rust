# README Implementation Blueprint

Date: 2026-05-26

Role: Topic
Status: Active planning
Read when: Turning the v7 README plan into concrete `README_zh.md` and `README.md` edits
Related: [readme-information-architecture.md](readme-information-architecture.md), [media-capture-and-asset-plan.md](media-capture-and-asset-plan.md), [v7-interface-and-basic-functions.md](v7-interface-and-basic-functions.md), [tmux-onboarding-runbook.md](tmux-onboarding-runbook.md), [multi-agent-positioning-and-comparison.md](multi-agent-positioning-and-comparison.md)

## Purpose

Define the concrete README rewrite plan before editing the public README files.
This blueprint incorporates:

- the `ccb_test2` screenshot set;
- the opening multi-agent necessity and approach comparison;
- a minimal tmux survival section for new users;
- basic config and `ccb-config` skill guidance;
- structure patterns borrowed from high-star adjacent projects.

## External README Structure Survey

Snapshot date: 2026-05-26. Star counts were scraped from GitHub repository HTML
for rough prioritization, not as product claims.

| Project | Stars Observed | Relevant Structure Pattern To Borrow |
| :--- | ---: | :--- |
| [OpenHands](https://github.com/OpenHands/OpenHands) | 74,851 | Strong centered hero, badges, clear product entry paths, and docs links instead of placing all detail in the README. |
| [Microsoft AutoGen](https://github.com/microsoft/autogen) | 58,390 | Installation and quickstart appear early, followed by concrete examples and "where next" links. Important warnings are visible, not buried. |
| [CrewAI](https://github.com/crewAIInc/crewAI) | 52,183 | Uses a table of contents, "Why", "Getting Started", examples, comparison, and FAQ. It also teaches AI-coding-agent usage directly. |
| [OpenHive](https://github.com/aden-hive/hive) | 10,435 | Opens with visual branding, then "Who is this for?", "When should you use it?", quick links, and quick start. This is useful for CCB's multi-agent positioning. |
| [Claude Squad](https://github.com/smtg-ai/claude-squad) | 7,616 | Most relevant terminal/tmux reference: screenshot immediately, short highlights, install, prerequisites, usage, menu keys, config, FAQ, and how-it-works. |

Borrow:

- Put a current visual near the top, then explain it.
- Make the new-user path visible before deep configuration detail.
- Add "who/when/why" framing before commands.
- Keep usage keys and tmux operations task-based, similar to Claude Squad's menu section.
- Move long history and advanced details behind links or `<details>`.

Do not borrow blindly:

- Do not let badges, enterprise positioning, or long FAQ push the first working
  path too far down.
- Do not keep the full release history inline as the dominant lower half of the
  README; keep latest v7 highlights and link `CHANGELOG.md`.
- Do not depend on remote video alone; every video should have a static
  screenshot fallback.

## Target README Order

Write `README_zh.md` first, then update `README.md` for parity.

| Order | Section | Visible By Default | Folded Under `<details>` |
| :--- | :--- | :--- | :--- |
| 1 | Hero | Name, one-line positioning, version/platform badges, language links, top navigation. | None. |
| 2 | Why Multi Agents | Single-agent versus multi-agent table and the user's plain-language limitations. | Longer examples: builder/reviewer, research/implementation, parallel worktrees. |
| 3 | Which Multi-Agent Approach | Short comparison: Claude Code native multi-agent, Hive/OpenHive, CCB. | Expanded comparison: model mixing, control, context, visibility, recovery, lifecycle, wrong-fit cases, source links. |
| 4 | CCB In One Screen | Regenerated real terminal screenshot and three promises: visible, provider-mixed, project-scoped. | Screenshot capture notes if needed. |
| 5 | 90-Second Quick Start | Install or update, run `ccb`, send one `/ask`, re-enter with `ccb`, stop with `ccb kill`. | Platform-specific install notes. |
| 6 | CCB v7 界面速览 | Region/function table explaining Sidebar, agent rows, active marker, Comms, agent panes, windows, and pane titles. | Sidebar state details, what is not shown, and provider-specific caveats. |
| 7 | Daily Operation | `ccb`, `ccb -s`, `ccb -n`, `ccb kill`, `ccb kill -f`, `ccb update`. | Source/dev install update behavior and older-project migration notes. |
| 8 | tmux 常规操作 | Explain `<prefix>` once, emphasize "press `Ctrl-b`, release, then press the next key" and English input method, then list common pane/window/copy/paste shortcuts. | Search/history-top commands and mode-recovery notes. |
| 9 | Configure Your Team | Config precedence, compact config, v7 `[windows]`, worktree, per-agent model/key/url, `ccb config validate`. | Advanced provider profile and migration examples. |
| 10 | Use `ccb-config` Skill | What it changes by default, example prompts, confirm-before-write flow, restart reminder. | Memory-design and skill-inheritance caveats. |
| 11 | Agent-to-Agent Work | `/ask`, `$ask`, callback, `--silence`, submit-once discipline. | Chained callback explanation and examples. |
| 12 | Editor Workflow | One editor screenshot and concise workflow. | Editor-specific notes. |
| 13 | Troubleshooting | Small "first action" table. | Detailed diagnostics and support bundle once stable commands are confirmed. |
| 14 | Community, Credits, And Release History | Contact, community, `tmux-agent-sidebar` thanks, latest v7 highlights, link to `CHANGELOG.md`. | Older inline release notes only if a later decision keeps them. |

## Screenshot Placement

Current draft assets from `ccb_test2`:

| Asset | README Use | Caption / Alt Text |
| :--- | :--- | :--- |
| `assets/readme_v7/ccb-test2-terminal-annotated.png` | Chinese public README hero and "what you are looking at" overview. | "CCB v7 workspace showing the sidebar, Comms area, two Codex agents, one Claude agent, and the active pane." |
| `assets/readme_v7/ccb-test2-terminal-annotated-en.png` | English public README hero. | "CCB v7 workspace showing the sidebar, Comms area, two Codex agents, one Claude agent, and the active pane." |
| `assets/readme_v7/ccb-test2-workspace-annotated.png` | Older planning reference only. | Not referenced by public README. |

Important note: maintainer decision requires regenerating real terminal
screenshots for public README publication. The current images are useful
planning assets rendered from tmux capture text and should guide the final
annotations, not replace final screenshots.

## Region Explanation For The Annotated Screenshot

Use a simple table directly under the screenshot. The complete section plan
lives in [v7-interface-and-basic-functions.md](v7-interface-and-basic-functions.md).

| Region | What It Means | What A New User Should Do |
| :--- | :--- | :--- |
| Sidebar | Shows managed windows and named agents. In `ccb_test2`, it shows window `main` and `agent1`, `agent2`, `agent3`. | Use it as the map of the workspace before reading pane details. |
| Active marker / pane header | Indicates where keyboard input goes. In the screenshot, `agent2` is active. | Click another pane if input is going to the wrong agent. |
| Comms | Shows recent ask/job communication and status. | Check here after delegating work to another agent. |
| Agent panes | Each pane is a real provider CLI session. The screenshot shows Codex plus Claude side by side. | Treat each pane as a named teammate with its own context and tools. |
| Project lifecycle | CCB owns start, attach, rebuild, recovery, and shutdown for the project workspace. | Use `ccb`, `ccb -n`, `ccb kill`, and `ccb kill -f`; do not edit `.ccb/ccbd` runtime files by hand. |

Add a visible "basic functions" table after this region table:

| Function | User-Facing Explanation |
| :--- | :--- |
| Visible multi-agent workspace | Multiple CLI agents stay visible in one project-owned tmux workspace. |
| Named agents | Agents can be referenced by stable names such as `main`, `worker1`, `reviewer`, or `agent2`. |
| Mixed providers | Different panes can run different providers, for example Codex and Claude in the same project. |
| Active input target | Only one pane receives keyboard input at a time; click or switch focus before typing. |
| Ask communication | `/ask` and `$ask` route work to a named agent; Comms shows the communication state. |
| Windows grouping | v7 `[windows]` can group agents by workflow area such as planning, implementation, review, or research. |
| Worktree isolation | Agents configured with `(worktree)` can work in isolated git worktrees; explain details in the config section. |

Add a short visible credit near the sidebar explanation:

```md
Sidebar 相关实现基于/借鉴了 [tmux-agent-sidebar](https://github.com/hiroppy/tmux-agent-sidebar) 的思路，在此表示感谢。
```

## tmux Section Design

Visible wording should teach useful tmux keyboard operations without turning the
README into a full tmux manual. Use this Chinese copy in `README_zh.md`:

```md
## tmux 常规操作

CCB 虽然基本全部可以使用鼠标操作，但是学会 tmux 快捷键可以显著增加便利性。下面列举部分常用的键盘操作快捷键。

约定：本文里的 `<prefix>` 指 tmux 前缀键，默认是 `Ctrl-b`。

按法注意：先按住 `Ctrl`，再按 `b`；然后两个键都松开；最后再按后面的键。例如 `<prefix> + z` 不是同时按 `Ctrl-b-z`，而是先按 `Ctrl-b`，松开，再按 `z`。

请在英文输入法下按这些快捷键，避免中文输入法拦截符号键，例如 `[`、`]`、数字或字母快捷键。
```

Visible table:

| 操作 | 按键 | 说明 |
| :--- | :--- | :--- |
| 切换 pane | `<prefix> + 方向键` | 在上下左右 pane 之间移动焦点。 |
| 切换到下一个 pane | `<prefix> + o` | 按顺序切换到下一个 pane。 |
| 放大/还原当前 pane | `<prefix> + z` | 当前 agent 内容太小时很有用，再按一次恢复。 |
| 下一个 window | `<prefix> + n` | 切到下一个 tmux window。 |
| 上一个 window | `<prefix> + p` | 切到上一个 tmux window。 |
| 按编号切 window | `<prefix> + 0..9` | 跳到指定编号 window。 |
| 打开 window/pane 列表 | `<prefix> + w` | 在列表中选择目标 window 或 pane。 |
| 进入滚动/copy mode | `<prefix> + [` | 查看历史输出，也用于 tmux 内复制。 |
| 退出滚动/copy mode | `q` 或 `Esc` | 如果输入没有反应，先试这个。 |
| 滚动历史 | copy mode 中用方向键 / `PageUp` / `PageDown` | 查看更早的输出。 |
| 鼠标复制 | copy mode 中拖选文本 | 当前 tmux 配置下拖选结束会复制。 |
| 复制单词/整行 | copy mode 中双击 / 三击 | 双击选词，三击选行。 |
| 终端原生复制 | `Shift + 鼠标拖选` | 当鼠标被 tmux 接管时，用这个绕过 tmux。 |
| 粘贴系统剪贴板 | `Ctrl+Shift+V` / `Cmd+V` | Linux 终端常用前者，macOS 常用后者。 |
| 粘贴 tmux buffer | `<prefix> + ]` | 粘贴 tmux copy mode 复制的内容。 |
| detach | `<prefix> + d` | 离开 tmux session，但不关闭里面的程序。 |

Fold search and advanced copy-mode navigation under `<details>`:

| 操作 | 按键 | 说明 |
| :--- | :--- | :--- |
| 搜索历史输出 | copy mode 中 `Ctrl-r` / `Ctrl-s` | 向上或向下搜索输出内容。 |
| 回到历史顶部/底部 | copy mode 中 `Alt-<` / `Alt->` | 快速跳转历史输出。 |
| 退出误入状态 | `q` / `Esc` / `Ctrl-c` | `q`/`Esc` 退出 copy mode；`Ctrl-c` 会发送给当前 pane 中的程序。 |

Do not include destructive tmux commands such as `kill-pane`, `kill-window`, or
`kill-server` in the README common-operations table.

## Basic Config Section Design

Visible config content should teach three decisions in order:

1. **Where config lives**: built-in default < `~/.ccb/ccb.config` < project
   `.ccb/ccb.config`; project config wins and is the normal README target.
2. **Which layout grammar to use**: compact/hybrid for a single visible
   workspace and optional `cmd`; `version = 2` `[windows]` when the user wants
   named windows and sidebar-first grouping.
3. **Which agent needs isolation or override**: `(worktree)` for isolated git
   worktrees; `[agents.<name>]` for `model`, `key`, `url`, `description`, and
   other overrides.

Visible examples:

```text
cmd; main:codex, reviewer:claude; worker1:codex(worktree)
```

```toml
version = 2
entry_window = "main"

[windows]
main = "main:codex"
work = "worker1:codex(worktree), worker2:claude(worktree)"
review = "reviewer:claude"

[ui.sidebar]
mode = "every_window"
width = "15%"
bottom_height = 20
```

Add `ccb config validate` after examples so users can check the effective
configuration layer before restarting CCB.

## `ccb-config` Skill Section Design

The Chinese README currently uses legacy `ccb_config` spelling in the config
skill section. The rewrite should use the current skill name: `ccb-config`.

Visible flow:

1. Tell the current agent the target workflow.
2. The skill reads the active config authority and proposes one complete config.
3. User confirms or adjusts the proposal.
4. The skill edits `.ccb/ccb.config` by default.
5. It validates the config and tells the user to restart CCB.

Example prompts:

```text
$ccb-config 为一个 Python library 设计团队：main 负责规划，worker1 和 worker2 用 worktree 并行实现，reviewer 用 Claude 做评审。
```

```text
$ccb-config 把当前单窗口配置迁移到 v7 多窗口：main 放 coordinator，work 放三个实现 agent，review 放 reviewer 和 qa。
```

Visible guardrails:

- It edits `.ccb/ccb.config` by default, not `.ccb_config/ccb.config`.
- It does not edit `.ccb/ccb_memory.md` or per-agent memory unless the user
  explicitly asks for workflow/role memory design.
- It should not restart CCB from inside the active pane; restart manually after
  validation.

Folded detail:

- skill inheritance behavior;
- provider model/key/url caveats;
- one-agent-only skill injection caveat if the user asks about it.

## Implementation Sequence

1. Finalize the visible/folded split in this blueprint.
2. Regenerate real terminal screenshots based on the `ccb_test2` layout.
3. Rewrite `README_zh.md` first using the regenerated assets and current v7
   command/config language.
4. Replace the current top showcase reference with the new real terminal hero
   screenshot under `assets/readme_v7/`.
5. Move long release history out of the main reading path or fold it behind a
   short latest-v7 summary.
6. Update `README.md` to match the Chinese structure and current skill spelling.
7. Run Markdown link checks and verify referenced image paths.
8. Validate documented config snippets with `ccb config validate` in temporary
   projects where practical.

## Acceptance Criteria

- A first-time user can understand why multi-agent work exists before seeing CCB
  commands.
- A non-tmux user can enter, focus, scroll, paste, detach/re-enter, stop, and
  recover without reading generic tmux docs.
- The README visibly explains the v7 sidebar and window model.
- Config examples cover compact, `[windows]`, worktree, per-agent model/API
  overrides, validation, and `ccb-config` skill usage.
- The README no longer uses `ccb_config` as the current skill name outside
  historical changelog text.
- The first screen uses regenerated real terminal v7 screenshots, with region
  explanation nearby.
- Advanced detail is available but folded so the README remains approachable.
