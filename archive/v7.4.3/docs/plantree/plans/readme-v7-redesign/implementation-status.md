# Implementation Status

Date: 2026-05-26

## Current Phase

First README implementation patch landed. `README_zh.md` and `README.md` now
follow the v7 task-first structure, use real dark terminal screenshots from
`ccb_test2`, and link full release history to `CHANGELOG.md`.

## Active TODO

- Maintainer review of the new README structure and public wording.
- Optional follow-up polish after review: install asset naming, platform wording,
  or comparison phrasing if the maintainer wants a narrower claim.
- Video/demo planning remains deferred until the later media pass.

## Done This Phase

- Read the plan-tree skill and maintenance guidance.
- Inventoried existing README headings, current README top sections, assets,
  source version, project config, and relevant v7 config/sidebar contracts.
- Created the plan-tree root and README v7 plan root.
- Recorded maintainer decisions: use a sanitized demo project, optimize for new
  users, rewrite v6 update examples for v7/current behavior, teach only
  CCB-required tmux operations, use conservative split platform wording, keep
  only v7 highlights in README with full history linked, write Chinese first,
  and use collapsible details to reduce first-read impact.
- Planned the new opening section: single-agent versus multi-agent comparison,
  then provider-native implicit orchestration versus Hive versus CCB.
- Corrected the Hive baseline from the earlier wrong-Hive assumption to
  OpenHive at `github.com/aden-hive/hive`.
- Added maintainer-provided single-agent limitations to the multi-agent
  positioning topic for direct README drafting.
- Researched Claude Code native multi-agent docs and OpenHive README, then
  recorded comparison findings in
  [topics/multi-agent-research-notes.md](topics/multi-agent-research-notes.md).
- Added a plain-language comparison draft focused on model mixing, controllable
  permissions, context/memory, visibility, recovery, and fit boundaries.
- Expanded the comparison plan into visible and folded tables so the README can
  stay readable while preserving detailed tradeoffs.
- Added operation media strategy: README screenshots and short silent clips,
  long walkthrough videos hosted on Bilibili, and subtitles-first narration
  workflow.
- Generated draft `ccb_test2` screenshots in `assets/readme_v7/`, then later
  kept only the full-workspace planning reference and removed the local detail
  crops from the public asset set.
- Inspected the live `ccb_test2` tmux layout and confirmed the screenshot
  regions: sidebar, Comms, `agent1` Codex, active `agent2` Codex, and `agent3`
  Claude.
- Verified CCB-managed tmux defaults in `ccb_test2`: `Ctrl-b` prefix,
  `mouse on`, `set-clipboard on`, vi copy-mode, and fallback bindings for
  pane focus, window switching, copy mode, and detach.
- Surveyed README structures from OpenHands, AutoGen, CrewAI, OpenHive, and
  Claude Squad and documented the patterns to borrow.
- Added
  [topics/readme-implementation-blueprint.md](topics/readme-implementation-blueprint.md)
  as the concrete README modification plan.
- Planned the `tmux 常规操作` README section with a single `<prefix>` convention,
  explicit "press `Ctrl-b`, release, then press the next key" wording, English
  input-method warning, and common pane/window/copy/paste shortcuts.
- Added
  [topics/v7-interface-and-basic-functions.md](topics/v7-interface-and-basic-functions.md)
  to define the `CCB v7 界面速览` README section, including screenshot regions,
  basic user-facing functions, sidebar details, and caveats.
- Added
  [topics/readme-rewrite-execution-plan.md](topics/readme-rewrite-execution-plan.md)
  to turn the blueprint into an edit-ready README patch plan and group
  clarification dependencies.
- Recorded final maintainer decisions in
  [decisions/003-readme-final-publication-choices.md](decisions/003-readme-final-publication-choices.md):
  regenerate real terminal screenshots, use release-first install/update
  wording, and document native Windows as v5-only with newer versions
  unsupported natively.
- Added maintainer screenshot style preference: use the existing dark terminal
  visual style with sparse annotations and README-side explanation tables.
- Added README planning requirement to thank
  [tmux-agent-sidebar](https://github.com/hiroppy/tmux-agent-sidebar) near the
  v7 sidebar/interface explanation and/or final credits section.
- Resolved remaining first-patch questions: multi-agent comparison wording is
  fixed, detailed troubleshooting commands are not needed for the first README
  patch, and concrete demo scenarios are deferred until the later media/video
  pass.
- Captured real dark terminal screenshots from the live `ccb_test2` tmux
  session through Xvfb + wezterm and added public hero assets:
  `ccb-test2-terminal.png`, `ccb-test2-terminal-annotated.png`, and
  `ccb-test2-terminal-annotated-en.png`.
- Rewrote `README_zh.md` around the agreed structure: multi-agent necessity,
  Claude Code / Hive / CCB comparison, CCB v7 UI tour, release-first quick
  start, daily operations, tmux common shortcuts, config examples,
  `ccb-config` workflow, ask/callback collaboration, platform notes, FAQ, and
  credits.
- Mirrored the new structure and media usage in `README.md`.
- Verified README local links and image paths.
- Removed folded local/detail screenshots from both README files and deleted the
  unused sidebar/Codex/Claude local crop assets.
- Revised Quick Start config guidance to keep the quick-start flow but start
  from a v7 `[windows]` topology example instead of a light single-window team,
  and added visible tables explaining what `.ccb/ccb.config` can configure plus
  when to use `ccb-config` for deeper discussion.
- Folded the longer config format examples and `ccb-config` write-flow details
  under `<details>` blocks so the README keeps the quick-start and config
  capability overview visible without overwhelming first-time readers.
- Simplified the opening multi-agent meaning and solution-comparison sections:
  the visible path now uses shorter summary tables, while single-agent limits
  and detailed Claude Code / Hive / CCB tradeoffs are folded under
  `<details>`.

## Blockers

- No owner-decision blockers remain for README direction.
- No owner-decision blockers remain for screenshot style, install/update
  positioning, or platform wording.
- No first-patch blockers remain.

## Next Commit Target

Commit the rewritten README files, v7 screenshot assets, and updated plan-tree
status after maintainer review.

## Last Verified Commands

- `find docs -maxdepth 3 -path 'docs/plantree*' -type f | sort`
- `grep -n '^#\\|^##\\|^###' README_zh.md`
- `find assets -maxdepth 3 -type f | sort`
- `find plans -maxdepth 3 -type f | sort`
- `cat VERSION`
- `sed -n '1,260p' README_zh.md`
- `sed -n '260,360p' README_zh.md`
- `sed -n '1,260p' docs/ccb-config-layout-contract.md`
- `sed -n '1,220p' docs/ccb-agent-sidebar-integration-plan.md`
- `git status --short`
- `file assets/readme_v7/*.png`
- `tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock list-panes -a -F ...`
- `tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock show-options -g prefix`
- `tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock show-options -g mouse`
- `tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock show-options -g set-clipboard`
- `tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock show-window-options -g mode-keys`
- `Xvfb :99 -screen 0 1900x1250x24 ... wezterm -n ... tmux -S /home/bfly/yunwei/ccb_test2/.ccb/ccbd/tmux.sock attach -t ccb-ccb_test2-777d80ce`
- `convert assets/readme_v7/ccb-test2-terminal-real.png -crop 1548x1100+0+0 +repage assets/readme_v7/ccb-test2-terminal.png`
- `python - <<'PY' ... generate annotated/cropped README screenshots ... PY`
- `python - <<'PY' ... README local links/images OK ... PY`
- `git diff --check -- README.md README_zh.md docs/plantree/plans/readme-v7-redesign`

## Handoff Notes

The repository already had unrelated useful_tools deletions/untracked files
before this plan was created. Do not modify or revert those while working on the
README plan unless the maintainer asks.
