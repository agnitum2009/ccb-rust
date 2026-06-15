# Architecture Optimization Plan

Date: 2026-05-18

## Purpose

This planning tree turns the full Architec analysis into an implementation
roadmap for reducing structural risk without drifting from the current CCB
runtime contracts.

Baseline evidence:

- Architec full run generated at `2026-05-18T02:45:38Z`
- Overall score: `50.11`
- Structure score: `64.83`
- Governance/full score: `35.39`
- Primary risk shape: complexity hotspots and over-wide runtime components, not
  package topology failure

## File Map

- [roadmap.md](roadmap.md) tracks phase-level optimization work.
- [implementation-status.md](implementation-status.md) records the active
  handoff state for the first implementation pass.
- [open-questions.md](open-questions.md) lists unresolved choices only.
- [topics/architec-baseline-diagnosis.md](topics/architec-baseline-diagnosis.md)
  summarizes the score, hotspots, and interpretation.
- [topics/provider-materialization-boundary-plan.md](topics/provider-materialization-boundary-plan.md)
  defines the shared provider-home refactor direction.
- [topics/release-checker-complexity-plan.md](topics/release-checker-complexity-plan.md)
  scopes the highest-ranked single-file hotspot.
- [topics/storage-classification-and-cleanup-plan.md](topics/storage-classification-and-cleanup-plan.md)
  handles cleanup, fallback, archive, and storage-classification work.
- [topics/repository-cleanup-and-filesystem-plan.md](topics/repository-cleanup-and-filesystem-plan.md)
  records file movement and archive/delete safety rules.
- [topics/post-phase-architec-results.md](topics/post-phase-architec-results.md)
  compares the post-phase `archi .` output with the baseline.
- [decisions/001-prioritize-behavior-preserving-boundary-extraction.md](decisions/001-prioritize-behavior-preserving-boundary-extraction.md)
  records the initial optimization strategy.
- [decisions/002-split-release-checker-with-explicit-skill-sync.md](decisions/002-split-release-checker-with-explicit-skill-sync.md)
  records the release checker split and skill sync tracking decision.
- [decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md](decisions/003-keep-reviewed-install-and-watch-fallback-surfaces.md)
  records the Phase 3 cleanup review decision for `install.ps1` and
  `watch_fallback.py`.

## Authority Boundaries

This plan is subordinate to the existing runtime contracts. When a planned
change touches startup, provider state, managed provider homes, completion, or
diagnostics, apply the relevant contract first:

- [../../docs/ccbd-startup-supervision-contract.md](../../docs/ccbd-startup-supervision-contract.md)
- [../../docs/ccb-provider-state-storage-boundary-plan.md](../../docs/ccb-provider-state-storage-boundary-plan.md)
- [../../docs/codex-session-isolation-contract.md](../../docs/codex-session-isolation-contract.md)
- [../../docs/claude-session-isolation-contract.md](../../docs/claude-session-isolation-contract.md)
- [../../docs/gemini-session-isolation-contract.md](../../docs/gemini-session-isolation-contract.md)
- [../../docs/codex-plugin-projection-plan.md](../../docs/codex-plugin-projection-plan.md)
- [../../docs/managed-provider-completion-reliability-plan.md](../../docs/managed-provider-completion-reliability-plan.md)

## Reading Order

Start with the baseline diagnosis, then the roadmap. Implementation work should
read the topic file for the active phase and the decision record before editing
code.
