# Implementation Status

Date: 2026-06-13

## Current Phase

Homepage polish implementation patch prepared, with release-surface follow-up
in progress. The first v7 README implementation existed, but maintainer
feedback said the GitHub first screen was too text-heavy, visually plain, and
unfocused. Reviewer1 endorsed the product-first order, the hero strategy was
resolved, and maintainer follow-up selected the newer promo-style CCB image as
the canonical hero composition. The top of both public README files now follows
the stable non-drift contract, including language-specific promo-style heroes
and a supported-CLI badge strip. Archi then blocked the commit because
npm-first README wording requires the `@seemseam/ccb` package surface to exist
in source before release.

## Active TODO

- Preserve
  [decisions/005-readme-design-non-drift-contract.md](decisions/005-readme-design-non-drift-contract.md)
  as the first planning reference before editing `README_zh.md` or `README.md`.
- Re-run release-surface validation after adding npm package metadata,
  npm runner wrappers, and npm publishing workflow.
- Send the updated dirty tree back through review after validation.
- Keep bilingual section order in sync during any follow-up wording changes.
- Preserve the newer promo-style hero pair and supported-CLI strip during any
  follow-up README or release-surface edits.

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
  regenerate real terminal screenshots, use npm-first install plus `ccb update`
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
  Claude Code / Hive / CCB comparison, CCB v7 UI tour, npm-first quick
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
- Added top-level README links to `docs/manuals/user-guide/` and
  `docs/manuals/developer-guide/`.
- Strengthened `ccb_self` README positioning as CCB's built-in
  self-understanding expert for CCB usage, active layout explanation, config
  design, runtime diagnostics, recovery, and workflow repair.
- Updated `README_zh.md` and `README.md` to recommend
  `npm install -g @seemseam/ccb` for new installs and `ccb update` for later
  updates, with GitHub release packages and source checkout installs documented
  as fallbacks.
- Added
  [decisions/005-readme-design-non-drift-contract.md](decisions/005-readme-design-non-drift-contract.md)
  as the stable README homepage design contract covering first-screen order,
  badges, manual links, hero policy, npm-first install, `ccb_self` positioning,
  and drift checks.
- Generated canonical README hero assets from the existing annotated v7
  screenshots: `assets/readme_v7/ccb-hero-zh.png` and
  `assets/readme_v7/ccb-hero-en.png`.
- Rewrote the public README first-read path in both languages:
  product title, quiet badges, manual links, canonical hero, three value points,
  npm new install plus `ccb update`, v7 UI tour, product definition,
  multi-agent rationale, and approach comparison.
- Replaced the older screenshot-derived canonical hero pair with the newer
  promo-style image and matching English version under `assets/readme_v7/`, with
  `ccb_self` callouts preserved in both languages.
- Added a compact supported-CLI logo/badge strip near the README first screen,
  showing Codex, Claude, Gemini, Kimi, OpenCode, Antigravity, and Droid.
- Received reviewer2 README/release-surface review with PASS_WITH_NITS and no
  blocking defects.
- Received archi release review with blocking findings: do not reuse existing
  `v7.4.3`, restore source-controlled npm package surface before npm-first
  README publication, and add public release notes for the Claude
  `stop_reason=end_turn` repair.
- Earlier chose the safer patch path for the next release: bump to `7.4.4`
  instead of moving or recreating the existing `v7.4.3` tag. The current
  combined release candidate now targets `v7.5.0` after the native CLI provider
  scope was added.
- Restored the source npm surface using the previously published package shape:
  `package.json`, Node CLI wrappers, postinstall artifact downloader, and a
  tag-triggered npm Trusted Publishing workflow that waits for GitHub release
  assets.
- Fixed follow-up archi blocker by aligning npm license metadata with the
  repository license: `package.json` now uses SPDX `AGPL-3.0-only`.

## Blockers

- No owner-decision blockers remain for README direction, screenshot style,
  install/update positioning, or platform wording.
- Release-surface blocker has a working-tree repair with local validation
  passing; final status depends on follow-up reviewer/archi sign-off.

## Next Commit Target

Commit the homepage README rewrite, canonical hero assets, npm release surface,
Claude P0 runtime fix, and updated plan-tree notes after validation and
follow-up review.

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
- `sed -n '1,260p' .ccb/ccbd/artifacts/text/completion-reply/job_6092320a91e1-art_89d691bcc11a49a2.txt`
- `test -d docs/manuals/user-guide && test -d docs/manuals/developer-guide`
- `rg -n "release-first|Release first|Release 优先|seemseam@ccb|@seemseam/ccb@latest|New users should start from a release package|首次安装推荐使用 \\[GitHub Releases\\]" README.md README_zh.md docs/plantree/plans/readme-v7-redesign/README.md docs/plantree/plans/readme-v7-redesign/roadmap.md docs/plantree/plans/readme-v7-redesign/topics`
- `rg -n '^#{1,3} ' README.md README_zh.md`
- `test -f assets/readme_v7/ccb-hero-en.png && test -f assets/readme_v7/ccb-hero-zh.png && test -d docs/manuals/user-guide && test -d docs/manuals/developer-guide`
- `npm pack --dry-run`
- `npm view @seemseam/ccb version dist-tags --json`
- `git ls-remote --tags origin refs/tags/v7.4.4`
- `git tag --list 'v7.5.0'`
- `python -m pytest -q test/test_claude_assistant_events.py test/test_v2_completion_detectors.py test/test_v2_completion_tracker.py test/test_v2_completion_orchestration.py test/test_v2_execution_service.py test/test_provider_hook_transcript.py test/test_provider_finish_hook_script.py test/test_claude_hook_results.py test/test_claude_execution_polling.py`
- `python -m compileall -q lib bin ccb`
- `git diff --check`
- Markdown local link check over public READMEs and active plan roots
- `/home/bfly/yunwei/ccb_source/ccb_test --diagnose` from
  `/home/bfly/yunwei/test_ccb2`
- `HOME=/home/bfly/yunwei/test_ccb2/source_home CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home /home/bfly/yunwei/ccb_source/ccb_test --version`
- `HOME=/home/bfly/yunwei/test_ccb2/source_home CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home /home/bfly/yunwei/ccb_source/ccb_test config validate`
- `python - <<'PY' ... license_metadata_ok AGPL-3.0-only ... PY`
- `file assets/readme_v7/ccb-hero-zh.png assets/readme_v7/ccb-hero-en.png`
- `rg -n "release-first|Release first|Release 优先|seemseam@ccb|@seemseam/ccb@latest|New users should start from a release package|首次安装推荐使用 \\[GitHub Releases\\]" README.md README_zh.md docs/plantree/plans/readme-v7-redesign/README.md docs/plantree/plans/readme-v7-redesign/roadmap.md docs/plantree/plans/readme-v7-redesign/topics`
- `rg -n "Supported CLIs|支持的 CLI|docs/manuals/user-guide|docs/manuals/developer-guide|ccb_self|assets/readme_v7/ccb-hero" README.md README_zh.md docs/plantree/plans/readme-v7-redesign/decisions docs/plantree/plans/readme-v7-redesign/topics docs/plantree/plans/readme-v7-redesign/roadmap.md docs/plantree/plans/readme-v7-redesign/implementation-status.md`
- `git diff --check -- README.md README_zh.md assets/readme_v7/ccb-hero-en.png assets/readme_v7/ccb-hero-zh.png docs/plantree/plans/readme-v7-redesign`
- Shields badge URL smoke check with `curl -L -s -o /dev/null -w '%{http_code}'`.

## Handoff Notes

The repository already had unrelated useful_tools deletions/untracked files
before this plan was created. Do not modify or revert those while working on the
README plan unless the maintainer asks.
