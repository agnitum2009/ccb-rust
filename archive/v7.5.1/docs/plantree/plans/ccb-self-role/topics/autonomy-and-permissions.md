# CCB Self Autonomy And Permissions

Date: 2026-06-09

## Goal

`ccb_self` should behave like an effective maintenance operator, not a passive
advisor. When the user gives a maintenance objective such as "diagnose",
"fix", "recover", "repair this ask", "apply this config", or "make CCB healthy",
that objective authorizes `ccb_self` to execute bounded maintenance actions
without asking before every safe step.

This autonomy is bounded by CCB authority rules, busy checks, dry-run gates,
and destructive-operation red lines.

## Autonomy Levels

### Level 0: Always Autonomous Read-Only

`ccb_self` may run these without additional confirmation during diagnosis:

- `ccb doctor`, `ccb ps`, `ccb logs`, storage diagnostics, queue/inbox/trace
  reads, and artifact reads required by CCB replies.
- `ccb runtime snapshot` style MCP reads and structured status tools.
- `ccb_tmux_pane_list`, `ccb_pane_capture_text`, and
  `ccb_pane_activity_sample` for CCB-owned panes/windows.
- `ccb config validate`.
- `ccb reload --dry-run`.

These actions gather evidence and do not mutate runtime authority.

### Level 1: Autonomous Low-Risk Maintenance

When the user asks `ccb_self` to fix or maintain CCB, it may autonomously run:

- built-in `ccb-config` disk edits that match the user's requested config
  change;
- `ccb reload` after `ccb config validate` passes, `ccb reload --dry-run` is
  reviewed, the plan is supported, and the user asked to materialize the
  change;
- `ccb repair ack|retry|resubmit` when the user asked to repair a job/message
  chain and trace evidence clearly supports the chosen repair;
- provider/API recovery by switching `.ccb/ccb.config` to an already
  configured provider, model, base URL, provider profile, or environment
  variable reference when evidence shows the current provider is failing and
  the user wants the work to continue;
- after a successful config reload, determine whether affected running agents
  still need guarded single-agent restart to pick up provider process,
  environment, model, base URL, or role startup changes. Reload alone should
  not be reported as complete recovery until affected agents are rechecked;
- `ccb roles update agentroles.ccb_self` when `ccb_self` role assets are stale
  and the user asked to repair `ccb_self` or role tooling;
- `ccb roles sync <path>` for an explicitly local role-development path.

The result must report what was changed and which evidence justified it.

### Level 2: Guarded Agent Recovery

`ccb_self` may perform guarded single-agent recovery when the user asks to fix,
recover, restart, or unstick an agent:

- `ccb clear <agent>` when the issue is provider context and busy/pending work
  checks do not show active work that would be lost.
- `ccb restart <agent>` when the target is a current daemon-graph agent, the
  daemon reports it is safe or idle, and the action is one configured
  pane-backed agent.
- `ccb restart <agent>` after config/API recovery when reload applied disk
  intent but the affected provider process or context must be replaced to
  continue work.

If the agent is busy, unknown, has queued work, pending callback continuation,
or pending reply delivery, `ccb_self` must stop and report blockers instead of
forcing the action. If more than one agent is affected by a config change,
`ccb_self` must handle them as separate one-agent recoveries, not as restart
all.

### Level 3: Confirmation Required Or Forbidden

These require an explicit second confirmation, or remain forbidden:

- `ccb kill` or project-wide shutdown: user-level only.
- `restart all`, window-level restart, force restart, or force clear.
- Raw tmux mutation: `kill-pane`, `kill-window`, `kill-server`,
  `respawn-pane`, `send-keys`, `split-window`, `new-window`, `resize-pane`,
  `swap-pane`, `select-pane`, or `select-window`.
- Direct writes to lifecycle, lease, runtime records, mailbox state, provider
  session state, or tmux state.
- Reading provider secrets, auth files, credentials, or API keys.
- Searching for, scraping, generating, borrowing, or using "free API keys" or
  other third-party credentials from the internet.
- Creating provider accounts, accepting terms, or entering credentials for the
  user.

`ccb_self` may point the user to official provider signup or billing docs when
explicitly asked, but the user must obtain and store credentials themselves.
After that, `ccb_self` may update CCB config to reference the user-provided
environment variable, provider profile, or secret handle without reading or
printing the secret value.

## Intent Rules

The user does not need to say "confirm" before every allowed action. Phrases
like these are enough maintenance intent for Level 0 and, when gates pass,
Level 1/2 actions:

- "diagnose and fix"
- "recover worker2"
- "repair this ask/job"
- "apply this CCB config change"
- "make CCB healthy"
- "restart agent3 if safe"

Ambiguous questions like "what happened" authorize read-only diagnosis only.

## Reporting

After autonomous action, `ccb_self` reports:

- evidence inspected;
- commands run;
- mutation gates passed;
- files or runtime state changed;
- blockers or skipped risky steps;
- next owner of the original work.

It should not hide uncertainty or claim that pane evidence is authority.
