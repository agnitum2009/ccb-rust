# Agent3 Review: Provider Memory Ownership

Date: 2026-06-07

## Summary

Agent3 completed a read-only review of the provider memory ownership plan and
confirmed that source ownership manifest governance should be the main solution.
The review found no better alternative than ownership-based policy because
text-level deduplication cannot safely distinguish user-authored similar rules
from duplicated CCB-managed rules.

Artifact:

- `.ccb/ccbd/artifacts/text/completion-reply/job_203f5ca1b725-art_9f1dca0af5654612.txt`
- `.ccb/ccbd/artifacts/text/completion-reply/job_b1a5d6c2980f-art_95b68d795d11453f.txt`

## Findings Accepted Into Plan

- A second reverse review confirmed the plan is not implementation-ready until
  Codex generated memory and OpenCode project `AGENTS.md` policy are reconciled
  with the source ownership manifest.
- Codex contract currently conflicts with the target policy because it still
  says managed `CODEX_HOME/AGENTS.md` includes provider-native project
  `AGENTS.md`; update the contract before changing implementation.
- Codex implementation currently still includes provider-native project
  `AGENTS.md` in the generated memory path, so the target policy is not yet
  implemented.
- OpenCode project `AGENTS.md` loading is a blocking audit item because
  excluding it without evidence could drop user project memory, while including
  it could duplicate native loading.
- OpenCode contract currently says project `AGENTS.md` remains an input source,
  while the manifest marks that policy as audit-dependent.
- Claude contract drift is known: implementation already excludes project
  `CLAUDE.md`, but the contract still says it is included.
- Gemini should have explicit deferred rows in the manifest so it is not
  forgotten while Claude, Codex, and OpenCode are fixed.
- Provider-user-memory filtering needs a released-marker inventory and
  conservative tests for complete marker pairs, isolated markers, unrelated
  markers, and legacy collaboration sections.
- Claude route-mode `~/.claude/rules/ccb-config.md` and Codex source-home
  `AGENTS.md` need explicit ownership classification before implementation.
- Old generated `.ccb/ccb_memory.md` files should be auto-upgrade candidates
  only when they still match a known generated seed; user-edited files should
  not be overwritten.

## Outcome

The roadmap, source ownership manifest topic, and open questions were updated
to reflect these review findings.
