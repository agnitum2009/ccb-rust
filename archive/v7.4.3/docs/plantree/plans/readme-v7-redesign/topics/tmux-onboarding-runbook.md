# tmux Onboarding Runbook

Date: 2026-05-26

Role: Topic
Status: Active planning
Read when: Writing README guidance for users unfamiliar with tmux
Related: [roadmap.md](../roadmap.md), [../../../baseline/runtime-flows.md](../../../baseline/runtime-flows.md)

## Goal

Teach the minimum tmux knowledge needed to operate CCB v7 confidently. The README
should not assume the reader has used tmux before.

Maintainer decision: teach only CCB-required operations, not general tmux
administration.

## Mental Model To Explain

- CCB starts a project-owned tmux workspace for the current project.
- You usually interact with CCB through the visible panes and sidebar, not by
  managing tmux sessions manually.
- Closing a terminal is not the same as intentionally stopping the project
  backend.
- Use CCB commands such as `ccb`, `ccb kill`, `ccb kill -f`, and `ccb -n` for
  lifecycle control instead of editing `.ccb` runtime files.

## Minimum Operations

| Operation | README Should Teach | Needs Verification |
| :--- | :--- | :--- |
| Focus a pane/agent | Mouse click first; `Ctrl-b` plus arrow or `Ctrl-b o` as fallback | Sidebar-specific keyboard behavior only if smoke-tested |
| Switch windows | Sidebar/window list first, tmux fallback second | Fallback verified as `Ctrl-b n`, `Ctrl-b p`, `Ctrl-b 0..9`, and `Ctrl-b w` in `ccb_test2` |
| Scroll output | Mouse wheel first; `Ctrl-b [` copy mode fallback, `q` to leave | Platform terminal scroll behavior still varies |
| Copy text | Mouse selection and terminal copy behavior | Platform-specific clipboard behavior |
| Paste text | `Ctrl+Shift+V`, `Cmd+V`, or terminal paste behavior | macOS/terminal differences |
| Detach/reopen | `Ctrl-b d` detaches; running `ccb` from the project re-enters | None for tmux default in `ccb_test2` |
| Stop safely | `ccb kill` | Current CLI wording |
| Force cleanup | `ccb kill -f` then `ccb -n` when needed | Exact recovery sequence |

## Verified tmux Defaults From `ccb_test2`

Observed on 2026-05-26 using the CCB-managed tmux socket for
`/home/bfly/yunwei/ccb_test2`:

- tmux prefix: `Ctrl-b`
- `mouse on`
- `set-clipboard on`
- `mode-keys vi`
- prefix bindings include pane focus with Vim keys or arrow keys, next/previous window,
  numbered window selection, `Ctrl-b [` copy mode, and `Ctrl-b d` detach.

README wording should still lead with CCB actions and mouse behavior. Raw tmux
keys should be documented as fallbacks, not as the primary mental model.

## tmux Common Keyboard Operations Section

Add a dedicated README section named `tmux 常规操作` after the screenshot
explanation or daily operation section. This section should teach common tmux
keyboard operations only. It should not repeat CCB lifecycle commands.

Recommended Chinese README copy:

```md
## tmux 常规操作

CCB 虽然基本全部可以使用鼠标操作，但是学会 tmux 快捷键可以显著增加便利性。下面列举部分常用的键盘操作快捷键。

约定：本文里的 `<prefix>` 指 tmux 前缀键，默认是 `Ctrl-b`。

按法注意：
先按住 `Ctrl`，再按 `b`；然后两个键都松开；最后再按后面的键。
例如 `<prefix> + z` 不是同时按 `Ctrl-b-z`，而是：

1. 按 `Ctrl-b`
2. 松开
3. 按 `z`

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

Fold optional operations under `<details>`:

| 操作 | 按键 | 说明 |
| :--- | :--- | :--- |
| 搜索历史输出 | copy mode 中 `Ctrl-r` / `Ctrl-s` | 向上或向下搜索输出内容。 |
| 回到历史顶部/底部 | copy mode 中 `Alt-<` / `Alt->` | 快速跳转历史输出。 |
| 退出误入状态 | `q` / `Esc` / `Ctrl-c` | `q`/`Esc` 退出 copy mode；`Ctrl-c` 会发送给当前 pane 中的程序。 |

Important wording:

- Explain `<prefix>` once, then use `<prefix> + key` everywhere.
- Explicitly say the prefix sequence is not pressed all at once.
- Mention English input method before the table because `[` and `]` are common
  failure points under Chinese input methods.
- Keep CCB lifecycle commands out of this table.
- Do not teach destructive tmux commands such as `kill-pane`, `kill-window`, or
  `kill-server` as normal README operations.

## README Section Shape

Use direct, task-first wording:

```text
If you do not know tmux, remember these CCB operations first:
1. Run `ccb` from the project to enter or re-enter the workspace.
2. Click a pane to focus the agent you want to type into.
3. Scroll with the mouse; if that fails, use `Ctrl-b [` and `q`.
4. Paste with your terminal paste shortcut.
5. Use `Ctrl-b d` only to detach without stopping the project.
6. Use `ccb kill` to stop the project intentionally.
7. Use `ccb kill -f` plus `ccb -n` only for force cleanup and rebuild.
```

Avoid expanding this into a general tmux tutorial. Keep sidebar-specific
keyboard behavior out of the visible README unless it is smoke-tested in the
current release.

## Troubleshooting Topics

- "I closed the terminal. Did CCB stop?"
- "I do not see the sidebar."
- "I am in the wrong window."
- "Mouse copy/paste behaves differently in my terminal."
- "An agent pane looks stale."
- "I want to reset the project workspace without deleting config."

## Boundaries

- Do not teach general tmux administration beyond CCB needs.
- Do not ask users to run raw `tmux kill-server` against CCB-managed sockets.
- Do not expose `.ccb/ccbd` internals as normal user workflow.
- Document raw tmux shortcuts only as verified fallbacks.

## Final README Scope

The final README tmux section should cover:

- focus panes and sidebar rows;
- switch CCB windows;
- scroll output;
- copy and paste safely;
- leave and re-enter a project workspace;
- stop with `ccb kill`;
- force cleanup and rebuild with `ccb kill -f` plus `ccb -n` when appropriate.

Everything else should be folded or omitted.
