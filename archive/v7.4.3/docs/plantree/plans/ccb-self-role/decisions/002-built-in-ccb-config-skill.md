# Built-In CCB Config Skill

Date: 2026-06-09

## Context

The user wants `ccb_self` to be the CCB self-maintenance operator. That
includes project configuration work: designing and editing `.ccb/ccb.config`,
checking role bindings, understanding tmux/window layout implications,
detecting config drift, and deciding when reload or restart is needed.

CCB previously treated `ccb-config` as a universal inherited skill available
to all agents. That is convenient, but it spreads topology-changing authority
across the whole team. For a self-maintenance role, a cleaner model is that
other agents do their business work and route CCB configuration changes to
`ccb_self`.

Naming update: [006-future-modification-guardrails.md](006-future-modification-guardrails.md)
supersedes the early `agentroles.ccb_self` spelling. The canonical Role Pack id
is now `agentroles.ccb_self`.

## Decision

Make `ccb-config` a built-in skill of `agentroles.ccb_self`.

The canonical skill name should remain `ccb-config` because it is the CCB
configuration skill. The skill is built directly into the
`agentroles.ccb_self` Role Pack as a role-owned asset. It is not a global
inherited skill and is not a separate shared skill later assigned to the role.
When CCB installs or materializes the role for an agent, the built-in skill
appears only in `ccb_self`'s managed provider home.

The built-in `ccb-config` skill may edit `.ccb/ccb.config` and validate config
health. It must keep disk config, last-applied config signature, current daemon
graph, and tmux evidence separate. It may recommend reload/restart classes and
may execute `ccb reload` for `ccb_self` after validation gates pass and user
intent is explicit. It must not silently execute `ccb reload`, and it must not
execute `ccb restart` or `ccb kill`.

Every config edit has a required validation gate:

1. Write the disk config change.
2. Run or require `ccb config validate`.
3. If the user wants the change materialized and validation passed, run or
   require `ccb reload --dry-run`.
4. Only after the dry-run plan is understood may `ccb_self` execute
   `ccb reload`.
5. After reload, `ccb_self` must re-check affected agents. Provider command,
   provider profile, model, base URL, environment, role asset, or startup
   context changes may require a separate guarded single-agent restart.

## Consequences

- `ccb_self` becomes the single normal route for CCB project configuration
  design, edits, drift diagnosis, and reload readiness.
- Other agents should delegate config changes to `ccb_self` instead of editing
  `.ccb/ccb.config` directly.
- The skill can cover both design-time editing and runtime config health,
  reducing fragmentation.
- Runtime mutation remains separate from config editing: config writes affect
  disk intent; live graph changes require explicit CCB control-plane actions.
- Migration must remove the full inherited/global `ccb-config` from non-self
  agents, or replace it with a lightweight delegation stub.

## Naming

Primary name: `ccb-config`.

Rationale: users already understand the phrase, and the skill still owns CCB
configuration. The fact that it is private to `ccb_self` should be represented
by being a built-in Role Pack skill, not by forcing the name to carry
ownership.

Acceptable alias in docs: `ccb-self-config`, when the discussion needs to
emphasize the role owner.

Rejected names:

- `ccb-runtime-config`: ambiguous with daemon/runtime internals.
- `ccb-config-health`: too narrow once the private skill can edit disk config.
- `ccb-config-maintenance`: too broad and suggests runtime mutation.
- `ccb-ops-config`: less specific than `ccb_self` ownership.

## Migration Notes

The transition should avoid breaking existing projects abruptly:

1. Move the full config editing skill into the `agentroles.ccb_self` Role Pack
   as a built-in skill.
2. Remove the full skill from inherited/global skill sets for non-self agents.
3. Optionally leave a tiny non-self stub that says: route CCB config edits to
   `ccb_self`; do not edit `.ccb/ccb.config` directly.
4. Update provider memory so non-self agents know CCB topology/config work is
   owned by `ccb_self`.
5. Keep tests proving the built-in skill is materialized for `ccb_self`.

Test impact:

- Existing repo hygiene tests that require inherited
  `inherit_skills/*/ccb-config/SKILL.md` content must be updated when the full
  skill moves into `agentroles.ccb_self`.
- If a non-self delegation stub remains, tests should assert that it delegates
  to `ccb_self` and does not preserve full config-editing instructions.
