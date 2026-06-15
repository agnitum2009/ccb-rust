# README Information Architecture

Date: 2026-05-25

Role: Topic
Status: Active planning
Read when: Restructuring `README_zh.md` or `README.md`
Related: [roadmap.md](../roadmap.md), [../decisions/001-task-first-v7-readme.md](../decisions/001-task-first-v7-readme.md)
Concrete blueprint: [readme-implementation-blueprint.md](readme-implementation-blueprint.md)

## Current Inventory

Observed current README shape:

- Front matter and badges.
- "Why CCB" in collapsed details.
- A showcase image and two hidden demo GIFs.
- "Latest highlights".
- Start/exit commands.
- Config control.
- Install.
- How to use ask.
- Editor integration.
- Requirements, uninstall, community.
- Long inline release history.

Observed drift:

- `README_zh.md` is tagged `7.0.2`, but the update section still says `CCB v6`
  and examples target `ccb update 6`.
- `README.md` includes stronger v7 `[windows]` migration wording than
  `README_zh.md`.
- tmux operation guidance is far too small for non-tmux users.
- v7 sidebar and multi-window behavior are explained mostly in changelog terms,
  not as an operating model.

## Target Reader Paths

The new README should serve four explicit paths:

- New user: install, start, understand what appears on screen, perform one ask,
  stop safely.
- Non-tmux user: learn only the tmux operations needed for CCB.
- Existing CCB user: migrate from compact/single-window config to v7
  `version = 2` windows/sidebar.
- Team/project owner: design agent roles, worktree isolation, model/key
  overrides, and shared memory.

Priority: optimize the default path for new users, especially users who do not
already know tmux. Existing-user migration and advanced design details should be
discoverable but not dominate the first screen.

## Proposed Top-Level Structure

1. Why multi agents: compare single-agent work, multi-agent work, and the
   practical coordination problem CCB solves.
2. Multi-agent approaches: compare provider-native implicit orchestration, Hive,
   and CCB at a high level.
3. Hero: one-sentence positioning, one fresh v7 screenshot, and three concrete
   promises.
4. 90-second quick start: install, run `ccb`, ask another agent, stop.
5. CCB v7 interface overview: annotated screenshot explaining Sidebar, window
   list, agent rows, active marker, Comms, agent panes, and basic functions.
6. Daily operation: start/attach, safe mode, rebuild, kill, update, uninstall.
7. tmux survival guide: mouse focus, window switching, scroll/copy/paste,
   detach/reattach, and recovery.
8. Configure your team: compact single-window examples first, then v7
   `[windows]`, sidebar options, worktree, per-agent key/url/model.
9. Agent-to-agent work: `/ask`, `$ask`, implicit delegation, callback chaining,
   and submit-once discipline.
10. Editor workflow: VS Code/Neovim style use with a concise screenshot.
11. Troubleshooting: startup, bad layout, stuck job, stale pane, kill/rebuild,
    support bundle or diagnostics if confirmed stable.
12. Community and release history: keep latest v7 highlights in README and link
    full history to `CHANGELOG.md`.

## Content Rules

- Put the current v7 operating model above changelog history.
- Open with the problem: why serious work often outgrows one agent, and why
  multi-agent systems need visible coordination rather than invisible magic.
- Keep each code block copy-pasteable and avoid real API keys.
- Prefer real screenshots over abstract diagrams for the first screen.
- Keep README media lightweight: screenshots and short clips in git, longer
  walkthrough videos linked from Bilibili.
- Keep intuitive, visual information visible; fold advanced explanation,
  migration detail, and long examples under `<details>` to reduce first-read
  impact.
- Use plain explanations before contract words such as authority, lifecycle, or
  runtime records.
- Author `README_zh.md` first, then update `README.md` for parity.
- Keep bilingual parity; do not let English and Chinese describe different
  config capabilities.
- Replace v6 update examples with v7/current examples.
- State core CCB platform support separately from native sidebar/helper support.
- Avoid documenting tmux internals that users do not need to operate CCB.

## Candidate README Sections To Write

- "What changed in v7": native sidebar, named windows, richer project view, and
  safer project-owned tmux behavior.
- "CCB v7 interface overview": what each visible area does and which basic
  function it maps to.
- "Why multi agents": single agent versus multiple named agents, then official
  implicit orchestration versus Hive versus CCB.
- "If you do not know tmux": a short survival guide with the exact actions
  needed inside CCB.
- "Three config levels": built-in, user, project, with clear precedence.
- "When to use compact config vs windows topology": compact for one screen,
  `[windows]` for named workspaces and sidebar-first teams.
- "Safe cleanup": explain `ccb kill`, `ccb kill -f`, and `ccb -n` without
  making users edit `.ccb` runtime files.

## Folding Policy

Keep visible by default:

- hero screenshot and short positioning;
- a compact single-agent versus multi-agent comparison;
- a compact multi-agent approaches comparison;
- quick start;
- annotated "what you are looking at";
- the five-action tmux survival guide;
- one compact config and one v7 windows config.

Fold under details:

- full rationale for CCB;
- deeper comparison notes and caveats for other multi-agent approaches;
- advanced per-agent API/model examples;
- migration notes from older compact configs;
- troubleshooting detail beyond the first action;
- release history summary beyond the latest v7 highlights.
