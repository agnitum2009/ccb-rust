# README v7 Redesign Roadmap

Date: 2026-05-26

## Done

- Inventoried current README structure, version marker, and public media assets.
- Confirmed current source version marker is `7.0.2`.
- Identified current README drift:
  - Chinese update section still says `CCB v6`.
  - Chinese README lacks the English README's newer `[windows]` migration
    explanation.
  - tmux onboarding is limited to copy/paste guidance.
  - v7 sidebar/window operation is present mainly in changelog and contracts,
    not in the user journey.
- Created the `docs/plantree/` planning entrypoint and registered this plan.
- Promoted the user's README redesign idea from the ideas inbox into this plan.
- Recorded maintainer decisions for README scope, audience, media defaults,
  changelog placement, v7 update examples, tmux scope, and conservative
  platform wording in
  [decisions/002-readme-publication-defaults.md](decisions/002-readme-publication-defaults.md).
- Added the opening multi-agent necessity and approach-comparison plan in
  [topics/multi-agent-positioning-and-comparison.md](topics/multi-agent-positioning-and-comparison.md).
- Corrected the Hive comparison target to OpenHive at
  `https://github.com/aden-hive/hive`.
- Added maintainer-specified single-agent limitations to the opening narrative:
  role mixing, context focus loss, complexity ceiling, cost pressure,
  management/tool concentration, and serial waiting.
- Added source-backed research notes for Claude Code native multi-agent and
  OpenHive in
  [topics/multi-agent-research-notes.md](topics/multi-agent-research-notes.md).
- Expanded the multi-agent comparison into a two-layer README design: a visible
  decision table plus folded detail table.
- Added the operation demo video/audio plan, including screenshots, short clips,
  Bilibili-hosted walkthroughs, and a subtitles-first audio recommendation.
- Generated draft `ccb_test2` screenshot assets under `assets/readme_v7/` and
  mapped them to README hero/detail uses.
- Surveyed high-star adjacent README structures from OpenHands, AutoGen,
  CrewAI, OpenHive, and Claude Squad, then translated the useful patterns into
  a CCB-specific implementation blueprint.
- Verified CCB-managed tmux defaults in `ccb_test2`: `Ctrl-b` prefix,
  `mouse on`, `set-clipboard on`, and fallback bindings for pane focus,
  window switching, copy mode, and detach.
- Added the concrete
  [topics/readme-implementation-blueprint.md](topics/readme-implementation-blueprint.md)
  covering section order, visible/folded split, screenshot captions, tmux
  guidance, config examples, and `ccb-config` skill copy.
- Recorded final maintainer decisions: regenerate real terminal screenshots for
  public README media, use release-first install/update wording, and document
  native Windows support as v5-only with newer versions unsupported natively.
- Captured real dark terminal screenshots from `ccb_test2` and generated
  annotated Chinese/English README hero images under `assets/readme_v7/`.
- Rewrote `README_zh.md` and `README.md` around the agreed v7 task-first
  structure, including the opening multi-agent comparison, v7 UI tour, tmux
  onboarding, config examples, `ccb-config` workflow, platform notes, credits,
  and changelog link.
- Verified README local links and image paths.
- Removed the folded sidebar/Codex/Claude local detail screenshot blocks and
  deleted their unused crop assets.
- Kept Quick Start but changed the config starting point to a v7 `[windows]`
  topology example, then added config-capability tables and explicit
  `ccb-config` discussion guidance.
- Folded long config examples and `ccb-config` write-flow details to preserve a
  lighter first-read path.
- Simplified the opening multi-agent meaning/comparison copy and folded the
  detailed tradeoff tables to reduce first-read weight.

## In Progress

- Maintainer review of the first README implementation patch.

## Next

- Apply maintainer wording corrections, if any.
- Optionally tighten release asset naming after the release packaging path is
  confirmed against the final public assets.
- Later pass: resume deferred demo/video design.

## Deferred

- Adding a separate documentation website. This plan keeps the immediate target
  to GitHub README files and repo-local assets.
- Creating raw recording archives in git. Commit optimized public media only.

## Phase Gates

Phase 1 is complete when:

- Remaining open questions are verified or explicitly accepted as follow-up
  risks.
- The fold/visible section split is agreed.
- The sanitized demo project scenario is agreed.

Phase 2 is complete when:

- Required media assets exist in a stable folder, likely `assets/readme_v7/`.
- Each asset has a documented scene purpose, alt text, and privacy check.
- Animated assets have acceptable file size and a static fallback.

Phase 3 is complete when:

- Both public READMEs teach the same v7 workflow.
- v7 windows/sidebar configuration is no longer buried only in the changelog.
- A non-tmux user can start, switch focus/windows, copy/scroll/paste, detach or
  recover, ask another agent, and stop/rebuild from README guidance.

Phase 4 is complete when:

- README links and asset paths resolve.
- Public examples avoid secrets and local-only paths.
- Config examples match the current config contract.
- Any command examples that require current CLI behavior have been smoke-tested
  or explicitly marked as examples.
