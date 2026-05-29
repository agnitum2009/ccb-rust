# CCB Plan Tree

Date: 2026-05-25

## Purpose

This is the planning entrypoint for durable project plans that should be easy
to resume across agents and sessions.

## Authority Order

1. Product/runtime contracts under `docs/` remain authoritative for shipped
   behavior.
2. Registered plan roots under `docs/plantree/plans/` define active planning
   state.
3. `implementation-status.md` files are operational handoff notes and must not
   override roadmap or decision records.
4. Legacy plan files outside this tree are preserved in place until a specific
   migration decision exists.

## Baseline

- [baseline/README.md](baseline/README.md) indexes the lightweight project
  baseline used by plan roots.

## Active Plans

| Plan | Status | Scope |
| :--- | :--- | :--- |
| [readme-v7-redesign](plans/readme-v7-redesign/README.md) | In progress | Redesign public README content, screenshots, demo videos, and tmux onboarding for the v7 release line. |
| [sidebar-tips-layout](plans/sidebar-tips-layout/README.md) | In progress | Add a three-panel sidebar layout with compact Comms and configurable tmux Tips. |
| [sidebar-provider-activity](plans/sidebar-provider-activity/README.md) | Planning | Add provider-native activity evidence for accurate sidebar status, including Codex/Claude manual-pane state and API fault validation. |
| [ccbd-agent-hot-reload](plans/ccbd-agent-hot-reload/README.md) | Planning | Dynamically load, unload, and later replace agents in a running daemon without breaking unrelated panes. |

## Legacy Planning Sources

These roots predate `docs/plantree/` and are intentionally left in place:

- [architecture-optimization](../../plans/architecture-optimization/README.md)
- [ccb-communication-test-plan.md](../../plans/ccb-communication-test-plan.md)
- [droid-delegation-skills-plan.md](../../plans/droid-delegation-skills-plan.md)
- [factory-ai-integration-plan.md](../../plans/factory-ai-integration-plan.md)
- [project-scoped-daemon-isolation-plan.md](../../plans/project-scoped-daemon-isolation-plan.md)

## How To Read

Start with the baseline, then the specific plan root. Use the plan
`implementation-status.md` only when resuming active work.
