# CCB Self Role Roadmap

Date: 2026-06-10

## Done

- Chose `ccb_self` as the default project-local agent name.
- Chose `agentroles.ccb_self` as the stable role id.
- Established that `ccb_self` is an auxiliary maintenance role, not a business
  task owner.
- Established that `ccb_self` failure must not affect other configured agents,
  daemon lifecycle, provider sessions, or tmux panes.
- Reviewed the current configured-agent authority boundary: restartable agent
  targets must come from the mounted daemon service graph, not disk-only config,
  tmux pane lists, or `.ccb/agents/*` residue.
- Reviewed the preferred runtime restart command shape:
  `ccb restart <agent>`, with `repair` kept for message/job lineage recovery.
- Reviewed tmux user/reference guidance: include practical CCB-managed tmux
  usage, but do not expose raw destructive tmux controls as agent tools.
- Chose four broad capability groups instead of many narrow skills:
  `ccb-self-diagnose`, `ccb-self-recover`, `ccb-self-chain`, and built-in
  `ccb-config`.
- Recorded the auxiliary-agent boundary in
  [decisions/001-auxiliary-self-agent.md](decisions/001-auxiliary-self-agent.md).
- Accepted the user's product direction that CCB config design/editing should
  be a built-in `ccb_self` role skill, not universally available to all agents.
  See
  [decisions/002-built-in-ccb-config-skill.md](decisions/002-built-in-ccb-config-skill.md).
- Added the memory/tooling direction: role memory should carry identity,
  authority, command boundaries, and delegation rules; MCP tools should start
  read-only and include bounded screen evidence only for CCB-owned targets.
- Incorporated reviewer2's skill review: bootstrap config repair, chain/recover
  handoff, role asset update commands, v1 pane activity sampling, reload
  dry-run prerequisites, screenshot artifact storage, and config-skill test
  impact.
- Accepted stronger bounded autonomy for `ccb_self`: maintenance intent allows
  autonomous read-only diagnostics, validation, dry-runs, safe reloads, supported
  chain repairs, role asset repair, and guarded single-agent recovery.
- Recorded that provider/API or startup-affecting config recovery is
  reload-then-recheck, with guarded per-agent restart when a running provider
  process or context still needs refresh.
- Defined the v1/v2 Role blueprint in
  [topics/first-slice-blueprint.md](topics/first-slice-blueprint.md).
- Drafted reviewable first-slice `agentroles.ccb_self` Role content under
  [drafts/agentroles.ccb_self/](drafts/agentroles.ccb_self/), including role
  memory, four Codex built-in skill drafts, references, and a draft read-only
  doctor helper contract.
- Captured initial isolated validation evidence in
  [topics/skill-drafts-review-test-evidence.md](topics/skill-drafts-review-test-evidence.md):
  static role/skill checks pass, config validate and reload dry-run gates work,
  `repair retry|resubmit|ack` command semantics are visible, the initial
  `ccb restart <agent>` contract blocker was captured, and catalog validation
  is blocked until `agentroles.ccb_self` is materialized.
- Completed coworker and reviewer3 review loops for the four built-in skills,
  processed findings in
  [topics/skill-drafts-review-test-evidence.md](topics/skill-drafts-review-test-evidence.md),
  and fixed the remaining chain cancel-failure blocker.
- Reworked the accepted draft for the updated Agent Roles protocol and
  materialized it in `/home/bfly/yunwei/agent-roles-spec/roles/ccb-self`.
  The new source uses host-neutral `skills/`, CCB-specific adapter metadata in
  `adapters/ccb/`, catalog aliases, and tests that pass the Agent Roles CLI
  and manifest loader.
- Moved the full `ccb-config` skill under `agentroles.ccb_self` Role source and
  removed it from the public inherited Codex/Claude skill folders. Install
  paths now clean old public `ccb-config` residue.
- Implemented guarded `ccb restart <agent>` as a current-graph, single-agent
  ccbd control-plane operation with busy/pending/callback blockers and
  old/new runtime evidence.
- Mounted the prepared Role into the local `agent-roles-spec` catalog with
  aliases, roles index entry, mount-oriented wording, and local
  install/resolve/doctor/full-suite validation ready for the user to push.
- Collected legacy public `ccb-config` residue into
  `/home/bfly/.ccb/deprecated/ccb-config-public-20260610T081814+0800` and
  verified the current project now exposes `ccb-config` only through the
  Role-owned `ccb_self` private skill symlink.
- Added the CCB-side 7.4.0 provisioning direction: `install.sh install` now
  attempts to install or refresh `agentroles.ccb_self` as a recommended default
  Role Pack, post-update Role Pack provisioning installs missing recommended
  roles, and docs strongly recommend explicit project binding with
  `ccb roles add agentroles.ccb_self:codex`.
- Recorded the default install boundary in
  [decisions/004-default-recommended-install.md](decisions/004-default-recommended-install.md):
  role assets are prepared by default, but project topology changes remain
  explicit.

## In Progress

- Finish 7.4.0 release validation and push after review.

## Next

1. Decide whether to add a separate non-self delegation stub; the full inherited
   `ccb-config` source has been removed.
2. Add the v1 structured MCP/control-plane diagnostic helper contracts.

## Deferred

- Automatic background self-healing without user request.
- `restart all` or window-level restart commands.
- Role-driven raw tmux mutation.
- Force, project-wide, or destructive repair without confirmation.
- Business-task continuation by `ccb_self` after another agent fails.
- Reintroducing the full config editing skill as a global inherited skill for
  non-`ccb_self` agents by default.
- Multiple maintenance roles with shared lock arbitration.
