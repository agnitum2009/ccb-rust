# Ask Parameter Policy Plan

Date: 2026-06-07

## Purpose

Define a stable policy for how inherited `ask` skills choose CCB ask flags.
The goal is to turn the current flag list into a result-intent decision guide:
first decide whether the caller needs no result, a compact result, or a full
text result; then decide whether request text needs artifact-backed
preservation.

This plan does not change `ccbd`, the CLI parser, or callback routing behavior.
It only plans documentation, skill wording, examples, and validation.

## Current Policy Summary

The ask skill should answer three questions before submitting:

1. What result does the caller need?
2. Does the current task depend on the child result?
3. Does either side need artifact-backed content preservation?

Those answers choose the parameters:

| Question | Answer | Flag Choice |
| --- | --- | --- |
| Result intent | Publish or execute; successful result not needed | `--silence` |
| Result intent | Concise status, findings, risks, blockers, or next actions | `--compact` |
| Result intent | Consultation, analysis, report, generated doc, or full text needed | `--artifact-reply` |
| Result intent | Short question or short handoff; inline answer is enough | plain `ask` |
| Dependency | Active parent cannot finish until child result arrives | add `--callback` |
| Request fidelity | Exact transient logs, diff, JSON/YAML, table, or copied content | add `--artifact-request` |
| Bidirectional fidelity | Exact request and full reply both need preservation | use `--artifact-io` |

Important boundaries:

- `--callback` and `--silence` describe task relationship and result delivery.
- Artifact flags describe content preservation and are orthogonal to routing.
- The 4 KiB automatic spill is only a fallback, not the primary policy.
- Plain `ask` is intentionally narrow after this update.

## How To Use This Plan

Start with [topics/parameter-usage-matrix.md](topics/parameter-usage-matrix.md)
for the operational list, then use
[topics/task-relationship-decision-tree.md](topics/task-relationship-decision-tree.md)
when dependency routing is unclear.

Use [topics/parameter-semantics.md](topics/parameter-semantics.md) to verify
the meaning of an individual flag. Use
[topics/artifact-transport-policy.md](topics/artifact-transport-policy.md)
when deciding whether input or output needs artifact-backed preservation.

Use [topics/test-and-validation-notes.md](topics/test-and-validation-notes.md)
when validating changes in the real source-under-test lane.

## Validation Status

The current policy has recorded static, unit, and external runtime validation:

- inherited ask skill template tests cover result-intent anchors, artifact
  policy wording, command shape, and no-Chinese drift;
- ask route mapping tests cover callback and artifact route options;
- external `ccb_test` pressure tests in `/home/bfly/yunwei/test_ccb2` verified
  Codex-to-Claude and Claude-to-Codex decisions for direct, silent, callback,
  artifact-request, artifact-reply, and artifact-io scenarios;
- targeted result-intent smoke verified proactive `--silence`, `--compact`,
  and `--artifact-reply` choices.

## File Map

- [roadmap.md](roadmap.md): current implementation sequence and validation
  gates.
- [open-questions.md](open-questions.md): unresolved presentation and rollout
  questions only.
- [topics/parameter-semantics.md](topics/parameter-semantics.md): stable
  meaning of each ask flag.
- [topics/parameter-usage-matrix.md](topics/parameter-usage-matrix.md):
  scenario-by-scenario flag selection tips and combinations.
- [topics/task-relationship-decision-tree.md](topics/task-relationship-decision-tree.md):
  result-intent and dependency routing choices.
- [topics/artifact-transport-policy.md](topics/artifact-transport-policy.md):
  when request and reply artifact transport should be selected.
- [topics/callback-silence-boundaries.md](topics/callback-silence-boundaries.md):
  boundary examples for callback chains and silent execution chains.
- [topics/skill-update-draft.md](topics/skill-update-draft.md): text intended
  to be migrated into inherited ask skills.
- [topics/test-and-validation-notes.md](topics/test-and-validation-notes.md):
  static, unit, and external `ccb_test` validation plan.
- [decisions/001-keep-routing-explicit-in-skill.md](decisions/001-keep-routing-explicit-in-skill.md):
  record for keeping callback and silence as explicit skill decisions.
- [decisions/002-treat-artifact-as-content-transport.md](decisions/002-treat-artifact-as-content-transport.md):
  record for treating artifact flags as content-preservation transport.
- [decisions/003-preserve-silence-as-independent-execution.md](decisions/003-preserve-silence-as-independent-execution.md):
  record for the silent-on-success interpretation.
- [decisions/004-use-result-intent-as-primary-selector.md](decisions/004-use-result-intent-as-primary-selector.md):
  record for making silence, compact, and artifact-reply the primary result
  intent choices while narrowing plain ask.

## Related Sources

- [../../../managed-provider-completion-reliability-plan.md](../../../managed-provider-completion-reliability-plan.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)
- [../../baseline/test-and-release-gates.md](../../baseline/test-and-release-gates.md)
- [../../../../inherit_skills/codex_skills/ask/SKILL.md](../../../../inherit_skills/codex_skills/ask/SKILL.md)
- [../../../../inherit_skills/claude_skills/ask/SKILL.md](../../../../inherit_skills/claude_skills/ask/SKILL.md)
- [../../../../inherit_skills/droid_skills/ask/SKILL.md](../../../../inherit_skills/droid_skills/ask/SKILL.md)

## Scope

In scope:

- Inherited ask skill wording for Codex, Claude, and Droid.
- Parameter decision rules and examples.
- Static tests that keep inherited skill templates aligned.
- External `ccb_test` validation that source-managed skill projection still
  starts and exposes the updated ask skill text.

Out of scope:

- Automatic callback insertion by `ccbd`.
- New CLI warnings or parser behavior.
- Changes to callback edge, mailbox, reply delivery, or artifact storage
  semantics.
- Provider-specific delegation policies that would make ask behavior drift by
  provider.
