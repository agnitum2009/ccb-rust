# Sidebar Provider Activity Plan

Date: 2026-05-27

## Purpose

Plan the provider-native activity layer that makes sidebar agent status accurate
for manual pane work, CCB-managed jobs, provider failures, and interrupted turns.

The sidebar remains a `project_view` client. Provider hooks, session logs, and
runtime artifacts may enrich `project_view`, but they must not become separate
authority for project identity, agent identity, pane ownership, job state, or
layout.

## File Map

- [roadmap.md](roadmap.md): current implementation sequence and release gates.
- [open-questions.md](open-questions.md): unresolved technical questions only.
- [topics/codex-provider-activity.md](topics/codex-provider-activity.md):
  Codex hook/app-server findings, activity artifact shape, ProjectView
  precedence, and phased Codex integration.
- [topics/phase-1-provider-status-to-sidebar.md](topics/phase-1-provider-status-to-sidebar.md):
  first implementation slice for provider-native activity monitoring exposed to
  sidebar through `project_view`, without changing mailbox, Comms, reply, or
  retry behavior.
- [topics/test-matrix.md](topics/test-matrix.md): automatic, manual, API fault
  lane, and release-gate validation plan.
- [topics/current-ccbd-comms-and-retry.md](topics/current-ccbd-comms-and-retry.md):
  current Comms status, message-bureau lineage, automatic retry, manual retry,
  and recovery behavior.
- [topics/mailbox-internal-design-references.md](topics/mailbox-internal-design-references.md):
  existing mailbox-kernel, message-bureau, timeout/retry, summary read-model,
  and diagnostics references that should guide future provider-activity and
  mailbox-status integration.
- [decisions/001-project-view-owns-provider-activity.md](decisions/001-project-view-owns-provider-activity.md):
  decision record for keeping provider activity evidence behind `project_view`.
- [decisions/002-dedicated-api-lanes-for-status-testing.md](decisions/002-dedicated-api-lanes-for-status-testing.md):
  decision record for separate stable/fault API lanes.
- [decisions/003-provider-activity-is-execution-state-authority.md](decisions/003-provider-activity-is-execution-state-authority.md):
  decision record for treating provider-native activity as execution-state
  authority while CCB job/Comms state remains workflow metadata.
- [decisions/004-sticky-failed-until-next-turn.md](decisions/004-sticky-failed-until-next-turn.md):
  decision record for keeping provider failures visible until a new turn or
  runtime ownership change.

## Related Sources

- [../../../ccb-agent-sidebar-integration-plan.md](../../../ccb-agent-sidebar-integration-plan.md)
- [../../../managed-provider-completion-reliability-plan.md](../../../managed-provider-completion-reliability-plan.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)
- [../../baseline/test-and-release-gates.md](../../baseline/test-and-release-gates.md)

## Scope

In scope:

- Provider-native activity artifact contract.
- Codex and Claude manual-pane status accuracy.
- Provider error/failure state propagation into `project_view`.
- Freshness, stale-state, wrong-pane, and wrong-generation protection.
- Dedicated automatic and manual validation matrix.

Out of scope:

- Rust sidebar direct provider-file reading.
- Global tmux or provider-home scanning.
- App-server transport migration as a prerequisite.
- New sidebar mutating controls.
- Changing provider completion authority for `opencode`.
