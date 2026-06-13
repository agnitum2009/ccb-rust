# README v7 Redesign Plan

Date: 2026-05-25

## Purpose

Plan a full public README refresh for the v7 release line, including updated
screenshots, demo videos, operation guidance, and tmux onboarding for users who
do not already know tmux.

## Scope

In scope:

- [README_zh.md](../../../../README_zh.md) and
  [README.md](../../../../README.md) information architecture.
- Public screenshot and video asset requirements.
- v7 sidebar, `version = 2` windows topology, ask/callback, start/rebuild/kill,
  update, and config examples.
- A tmux survival guide written from a CCB user perspective.

Out of scope for this plan:

- Implementing runtime behavior changes.
- Rewriting provider-specific contracts.
- Moving legacy plan roots under `plans/`.

## Authority

This plan is subordinate to the project baseline and runtime contracts:

- [../../baseline/README.md](../../baseline/README.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../../../ccbd-diagnostics-contract.md](../../../ccbd-diagnostics-contract.md)
- [../../../ccb-agent-sidebar-integration-plan.md](../../../ccb-agent-sidebar-integration-plan.md)

## File Map

- [roadmap.md](roadmap.md) tracks the README redesign phases.
- [implementation-status.md](implementation-status.md) records current handoff
  state.
- [open-questions.md](open-questions.md) lists unresolved choices only.
- [topics/readme-information-architecture.md](topics/readme-information-architecture.md)
  defines the target README structure.
- [topics/readme-implementation-blueprint.md](topics/readme-implementation-blueprint.md)
  defines the concrete README section order, screenshot placement, tmux
  guidance, basic config coverage, and `ccb-config` skill copy plan.
- [topics/readme-rewrite-execution-plan.md](topics/readme-rewrite-execution-plan.md)
  defines the edit-ready rewrite strategy, target section order, visible/folded
  split, and clarification dependencies.
- [topics/homepage-showcase-polish.md](topics/homepage-showcase-polish.md)
  defines the 2026-06-12 homepage polish pass after maintainer feedback that
  the public README first screen is too text-heavy, visually weak, and
  unfocused.
- [topics/multi-agent-positioning-and-comparison.md](topics/multi-agent-positioning-and-comparison.md)
  defines the opening multi-agent necessity discussion and comparison against
  provider-native orchestration and Hive.
- [topics/multi-agent-research-notes.md](topics/multi-agent-research-notes.md)
  records source-backed research on Claude Code native multi-agent options and
  OpenHive before the public comparison is written.
- [topics/media-capture-and-asset-plan.md](topics/media-capture-and-asset-plan.md)
  defines screenshot and demo video requirements.
- [topics/v7-interface-and-basic-functions.md](topics/v7-interface-and-basic-functions.md)
  defines the user-facing v7 workspace interface introduction and basic
  function explanation.
- [topics/operation-demo-video-and-audio-plan.md](topics/operation-demo-video-and-audio-plan.md)
  defines CCB operation demo scenes, Bilibili video strategy, and audio/narration
  recommendations.
- [topics/tmux-onboarding-runbook.md](topics/tmux-onboarding-runbook.md)
  defines the user-facing tmux guidance to write.
- [decisions/001-task-first-v7-readme.md](decisions/001-task-first-v7-readme.md)
  records the primary README direction.
- [decisions/002-readme-publication-defaults.md](decisions/002-readme-publication-defaults.md)
  records maintainer decisions for audience, media, changelog, platform wording,
  and folding policy.
- [decisions/003-readme-final-publication-choices.md](decisions/003-readme-final-publication-choices.md)
  records final maintainer decisions for real terminal screenshots,
  npm-first install wording, and native Windows v5-only support wording.
- [decisions/004-homepage-hero-asset-strategy.md](decisions/004-homepage-hero-asset-strategy.md)
  records the homepage polish decision to use canonical language-specific hero
  images under `assets/readme_v7/` instead of directly referencing the generated
  `assets/ccb-promo.png` in public READMEs.
- [decisions/005-readme-design-non-drift-contract.md](decisions/005-readme-design-non-drift-contract.md)
  records the stable README homepage design contract that future edits must
  preserve unless an explicit replacement decision is made.
- [history/reviewer1-homepage-polish-2026-06-12.md](history/reviewer1-homepage-polish-2026-06-12.md)
  records the reviewer1 design review that unblocked first-screen ordering and
  identified hero asset strategy as the blocking issue.

## Reading Order

Read the latest non-drift decision first, then the hero asset decision, then the
information architecture topic, then the implementation blueprint, then the
roadmap. Use implementation status only when resuming active edits.
