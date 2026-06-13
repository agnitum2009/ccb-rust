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
| [managed-tool-windows](plans/managed-tool-windows/README.md) | Planning | Add first-class non-agent tool windows such as Neovim that appear in sidebar without provider/agent rows. |
| [rolepack-system](plans/rolepack-system/README.md) | Planning | Define a host-neutral Role Pack system for reusable agent roles, with CCB installation, projection, and governance as the first adapter. |
| [provider-memory-ownership](plans/provider-memory-ownership/README.md) | In progress | Replace ad hoc provider memory bundling with a source ownership manifest across Claude, Codex, and OpenCode. |
| [agent-roles-open-source](plans/agent-roles-open-source/README.md) | Planning | Plan the public `agent-roles` GitHub project as a spec-first RolePack standard with templates, reference roles, and future host adapters. |
| [install-update-stability](plans/install-update-stability/README.md) | Planning | Make fresh install, managed update, dependency provisioning, Role Pack refresh, and bilingual user output stable across supported environments. |
| [source-runtime-isolation](plans/source-runtime-isolation/README.md) | Planning | Keep source editing, source-under-test validation, and installed-release work environments isolated from each other. |
| [ccb-self-role](plans/ccb-self-role/README.md) | Planning | Design the `agentroles.ccb_self` maintenance role, its broad skills, CCB/tmux diagnostics tools, and self-contained recovery boundaries. |
| [ccb-maintenance-heartbeat](plans/ccb-maintenance-heartbeat/README.md) | Planning | Add a generic CCB-owned maintenance heartbeat that independently diagnoses agent health and escalates risk, unknown, or unhealthy states to a semantic assessor, defaulting to `ccb_self`. |
| [managed-provider-completion-reliability](plans/managed-provider-completion-reliability/README.md) | Planning | Track managed pane-backed provider completion terminalization, empty-reply guards, and timeout reliability fixes. |
| [native-cli-providers](plans/native-cli-providers/README.md) | In progress | Add first-class pane-backed native CLI providers for Kimi Code and DeepSeek-oriented Deep Code. |
| [workspace-sharing](plans/workspace-sharing/README.md) | In progress | Add explicit external workspace paths and internal shared worktree groups without changing default per-agent worktree behavior. |
| [ask-parameter-policy](plans/ask-parameter-policy/README.md) | Planning | Clarify how ask skills choose silence, compact, callback, and artifact flags from result intent, dependency, and content-preservation needs. |
| [ccb-manuals](plans/ccb-manuals/README.md) | Complete | Produced source-backed CCB developer and user manuals, including Archi/Hippo architecture analysis and a deep communication-logic chapter. |

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
