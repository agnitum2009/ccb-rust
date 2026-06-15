# CCB Self First Slice Blueprint

Date: 2026-06-09

## Goal

Define the concrete first implementable package for `agentroles.ccb_self`.
This blueprint turns the role design into a Role source, while keeping CCB
runtime code changes separate.

Update: the first slice has been reworked for the updated Agent Roles protocol
and materialized at `/home/bfly/yunwei/agent-roles-spec/roles/ccb-self`.

## Package Shape

The production role content should live in the role catalog or a local editable
role source, not inside `ccb_source` production paths.

Updated Agent Roles target shape:

```text
roles/ccb-self/
  README.md
  role.toml
  memory.md
  skills/
    ccb-self-diagnose/SKILL.md
    ccb-self-recover/SKILL.md
    ccb-self-chain/SKILL.md
    ccb-comm-reply-recover/SKILL.md
    ccb-pane-view-diagnose/SKILL.md
    ccb-config/SKILL.md
  references/
    runtime-authority.md
    tmux-ccb-quickstart.md
    recovery-runbooks.md
  adapters/
    ccb/
      adapter.toml
      memory.md
      tools/doctor.py
  tests/
    validation.md
  tools/
    README.md
```

Provider-specific skill folders are no longer the source shape. Host adapters
project the generic Role skills into provider-native surfaces.

## `role.toml`

Required identity:

- `id = "agentroles.ccb_self"`
- display name: `CCB Self Maintainer`
- default agent name: `ccb_self`
- category: `ops`
- purpose: CCB runtime self-maintenance and auxiliary recovery.

Responsibilities:

- diagnose CCB runtime, tmux namespace, provider pane, config, storage, and
  message-chain health;
- inspect real CCB-owned pane text through read-only capture when
  self-supervision cannot classify progress from control-plane evidence alone;
- recover from provider/API failures by switching to already configured or
  user-supplied provider/model/profile/env-var references;
- perform bounded autonomous maintenance under a user maintenance objective;
- own CCB project config design/editing through built-in `ccb-config`;
- return original business work to the original target agent after repair.

Non-goals:

- do not own coding/product tasks;
- do not replace `ccbd`, keeper, lifecycle, mailbox, or provider authority;
- do not make other agents depend on `ccb_self`;
- do not run project-wide destructive operations autonomously.

Permissions:

- filesystem: project read/write for `.ccb/ccb.config` only through
  `ccb-config`; read-only diagnostics elsewhere unless a CCB control-plane
  command performs the mutation;
- secrets: none;
- network: none for v1, except role catalog update paths owned by CCB role
  commands. It may point users to official provider docs when asked, but it
  must not obtain, scrape, borrow, or use API keys from the internet;
- tmux: read-only pane/window evidence only.

## `memory.md`

Keep under 50 lines. It should include:

- identity and non-goals;
- failure isolation;
- authority/evidence/residue;
- config ownership;
- command boundaries;
- bounded autonomy;
- secret boundary;
- handoff rule.

Use the skeleton in [memory-and-mcp-tools.md](memory-and-mcp-tools.md) as the
starting point.

## Built-In Skills

### `ccb-self-diagnose`

Entry point for "what is broken" questions. It should:

- gather structured CCB diagnostics and tmux pane evidence;
- classify failure domains;
- separate authority, evidence, and residue;
- choose a next skill or action.

### `ccb-self-recover`

Runtime recovery. It should:

- handle provider context, pane, mount, clear, reload, and guarded restart
  flows;
- after config/API reload, re-read runtime status and pane/provider evidence
  before declaring recovery complete;
- restart only affected current-graph agents whose provider process or context
  still reflects stale startup inputs, and only when guarded restart is
  available and busy checks pass;
- run busy/pending checks before `clear` or `restart`;
- refuse force/restart-all/project shutdown without separate confirmation.

### `ccb-self-chain`

Message/job lineage repair. It should:

- trace job/message/reply/artifact/callback state;
- read artifact-backed replies before acting;
- choose retry, resubmit, or ack from lineage evidence;
- hand off to recover only when process/context repair is truly needed.

### `ccb-comm-reply-recover`

Communication reply recovery. It should:

- diagnose "reply not received" incidents from trace, queue, inbox, and pane
  evidence;
- identify head-of-line blockage and duplicate retries;
- prefer cancelling stale active jobs before retrying or restarting;
- use pane capture to decide whether a running job is genuinely progressing.

### `ccb-pane-view-diagnose`

Pane-view self-supervision. It should:

- start from current CCB authority to resolve the target pane;
- use `tmux capture-pane` style text capture, biased toward the bottom/current
  prompt and recent scrollback;
- compare short-interval captures to classify active work versus stuckness;
- use screenshot fallback only when text is unavailable or insufficient;
- keep pane text and screenshots as evidence, not authority.

### `ccb-config`

CCB config ownership. It should:

- edit `.ccb/ccb.config`;
- run `ccb config validate` after every edit;
- run `ccb reload --dry-run` before materialization;
- execute `ccb reload` autonomously when gates pass and the user wants the
  change applied;
- identify affected agents that may need guarded restart after reload;
- hand affected-agent refresh decisions to `ccb-self-recover`; `ccb-config`
  does not perform runtime replacement itself;
- never execute `ccb restart`, `ccb kill`, or raw runtime writes from
  `ccb-config` itself.

## References

V1 should include:

- `runtime-authority.md`: daemon graph, lifecycle, lease, runtime records,
  tmux evidence, residue, and command boundaries.
- `tmux-ccb-quickstart.md`: user-facing tmux basics and safe/unsafe CCB tmux
  actions.
- `recovery-runbooks.md`: short operational flows, not full architecture
  contracts.
- `config-contracts.md`: config validation, reload gates, role binding,
  window/tool-window/sidebar/workspace rules.

## `tools/doctor.py`

V1 helper should be read-only and JSON-only:

```json
{
  "status": "ok|warn|error",
  "summary": "...",
  "findings": [],
  "evidence": [],
  "recommended_actions": []
}
```

Allowed reads:

- installed `ccb` diagnostics;
- non-secret CCB logs and artifact metadata;
- daemon/runtime/config status through CCB CLI or stable runtime APIs;
- CCB-owned tmux pane/window evidence.

Forbidden:

- provider credentials or auth files;
- internet-sourced API keys or unknown third-party credentials;
- raw lifecycle/lease/runtime writes;
- raw tmux mutation;
- arbitrary screenshots.

## MCP V1

V1 MCP should prioritize read-only evidence:

- `ccb_runtime_snapshot`
- `ccb_agent_status`
- `ccb_trace_lineage`
- `ccb_queue_status`
- `ccb_reload_plan`
- `ccb_storage_summary`
- `ccb_namespace_snapshot`
- `ccb_tmux_pane_list`
- `ccb_pane_capture_text`
- `ccb_pane_activity_sample`
- `ccb-pane-view-diagnose` should be able to consume these text artifacts as
  the default self-supervision path.

V1 mutation can remain CLI-driven through the role's normal shell commands if
MCP mutation wrappers are not ready.

## MCP V2

Add screenshot fallback and controlled mutations:

- `ccb_pane_screenshot`
- `ccb_visual_inspect`
- `ccb_reload_project`
- `ccb_clear_agent`
- `ccb_repair_retry`
- `ccb_repair_resubmit`
- `ccb_repair_ack`
- `ccb_restart_agent` after `ccb restart <agent>` exists.

Screenshot artifacts must stay in CCB-owned project/runtime artifact storage
and must only target CCB-owned panes/windows/tool windows. They are fallback
evidence when text capture cannot classify the state.

## Migration Work

1. Move full config editing instructions out of inherited/global
   `ccb-config` into the `agentroles.ccb_self` Role.
2. Replace non-self inherited `ccb-config` with a tiny delegation stub or remove
   it from non-self agents.
3. Update provider memory so non-self agents know CCB config changes belong to
   `ccb_self`.
4. Update repo hygiene tests that currently expect inherited `ccb-config`
   content.

## Validation

Use source-runtime isolation rules:

- Run source validation with `/home/bfly/yunwei/ccb_source/ccb_test` from
  `/home/bfly/yunwei/test_ccb2`.
- Use isolated `HOME` and `CCB_SOURCE_HOME` under the external test project.
- Do not run source runtime from `ccb_source`.
- Do not delete active `.ccb/agents/*` or `.ccb/ccbd/*` in this work
  environment.

V1 acceptance:

- `ccb roles install/add agentroles.ccb_self` binds `ccb_self`.
- `ccb_self` receives role memory and all built-in skills.
- Non-self agents do not receive full config editing instructions.
- `ccb ask ccb_self "diagnose CCB"` can gather read-only diagnostics.
- Built-in `ccb-config` follows edit -> validate -> dry-run -> safe reload.
- Provider/API config recovery verifies post-reload runtime state and either
  proves the affected agents picked up the change or reports/executes guarded
  per-agent restart when the target is restartable and busy checks pass.
- Pane evidence tools read only CCB-owned panes and do not mutate tmux state.
- Pane-view self-supervision can classify a stuck-provider incident from
  trace + bottom pane capture + activity sample, and uses screenshot only as
  fallback when text evidence is insufficient.
