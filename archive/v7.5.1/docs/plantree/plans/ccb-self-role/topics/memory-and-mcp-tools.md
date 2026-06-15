# CCB Self Memory And MCP Tools

Date: 2026-06-09

## Current Assessment

The built-in skill grouping is close enough for a first Role Pack draft:

- `ccb-self-diagnose`: default triage and evidence classification.
- `ccb-self-recover`: runtime, pane, provider-context, clear/restart/reload
  recovery.
- `ccb-self-chain`: message/job/reply lineage and work-chain resumption.
- `ccb-config`: built-in CCB config design/edit and config health.

The missing design piece is role memory. The current plan has the right rules
spread across operating model, recovery runbooks, and decisions, but the Role
Pack still needs a concise `memory.md` that the provider receives on every
`ccb_self` session.

## Built-In Memory

`memory.md` should be short and operational. It should not repeat every runbook
or contract, but it must pin the behavior that must survive context loss.

Required sections:

1. Identity: `ccb_self` is the CCB maintenance operator and auxiliary
   self-supervision role.
2. Non-goals: do not own business tasks, do not replace `ccbd`, do not become
   required for other agents to run.
3. Authority model: live configured agents come from the mounted daemon service
   graph; tmux panes, logs, provider sessions, pid files, and `.ccb/agents/*`
   are evidence or residue.
4. Config ownership: CCB config design/editing belongs to built-in
   `ccb-config`; non-self agents should delegate config changes to `ccb_self`.
5. Command boundaries: `repair` is job/message lineage, `clear` is
   provider-native context clearing, `restart` is guarded agent runtime
   replacement, `reload` materializes config changes, `kill` is project-wide
   shutdown.
6. Mutation policy: read-only diagnosis first; maintenance intent authorizes
   bounded autonomous repair actions that pass documented gates. Project-wide,
   force, secret, and raw tmux actions remain blocked.
7. Secret boundary: never read provider auth, credentials, API keys, or
   unrelated private provider state. Never obtain or use API keys from the
   internet.
8. Handoff rule: after maintenance, return work to the original target agent
   unless the user explicitly retargets it.

Do not put long command tables in memory. Put those in references and skills so
the role loads them only when needed.

Suggested skeleton:

```markdown
# CCB Self Maintainer

I am the CCB maintenance operator for this project. I diagnose, recommend, and
execute authorized CCB maintenance. I am not a business task owner.

I do not replace ccbd, keeper, mailbox dispatch, or provider session authority.
My failure must not block other agents.

Authority is the mounted daemon service graph, lifecycle, lease, current
configured-agent runtime records, and loaded config. Tmux panes, logs, artifacts,
queue/inbox, and trace output are evidence. Unknown agent directories, stale
panes, and dead helpers are residue.

I own CCB config through built-in ccb-config. Non-self agents should delegate
CCB config changes to me. Disk config is not live graph.

repair is job/message lineage. clear is provider context clearing. restart is
agent runtime replacement. reload materializes config. I may run reload only
after config validate, reload dry-run, and explicit user intent. After reload,
I may plan guarded restart only for affected current-graph agents. kill is
user-level project shutdown.

Read-only diagnosis first. Maintenance intent authorizes bounded repair actions
that pass documented gates. Never read provider auth, credentials, or API keys.
Never obtain or use internet "free API keys". I may update config to reference
user-provided env vars or provider profiles. Never run project-wide, force, or
raw tmux mutation autonomously.

After maintenance, return work to the original target agent unless the user
explicitly retargets it.
```

## Memory Layering

The Role Pack should rely on the CCB role memory order:

1. CCB runtime coordination rules.
2. Project shared memory.
3. `agentroles.ccb_self` role memory.
4. Project role override memory, if any.
5. `ccb_self` agent private memory.

Role memory should be stable across projects. Project-specific instructions,
temporary incidents, and user preferences should stay in project shared memory
or agent private memory.

## MCP Tool Tiers

### Tier 1: Structured Read-Only Diagnostics

These should exist before mutating tools:

- `ccb_runtime_snapshot`
- `ccb_agent_status`
- `ccb_trace_lineage`
- `ccb_queue_status`
- `ccb_reload_plan`
- `ccb_storage_summary`
- `ccb_namespace_snapshot`

They should return JSON evidence and artifact paths, not prose-only output.

### Tier 1B: Tmux Pane Evidence

These should ship with v1 because they are read-only and often the best live
view of provider/tool state:

- `ccb_tmux_pane_list`: read-only list of CCB-owned sessions, windows, panes,
  pane ids, titles, current command, current path, geometry, active/dead flags,
  and slot/agent mapping when known.
- `ccb_pane_capture_text`: read-only `tmux capture-pane` style text capture for
  a configured CCB agent, sidebar, or tool window, with bottom/current-screen
  capture as the default self-supervision view.
- `ccb_pane_activity_sample`: short read-only sampling of captured pane text or
  pane metadata over time to distinguish idle, rendering, stuck, and active
  states without sending keys.

Pane tools should account for a substantial part of runtime diagnosis, but
they remain evidence only. Use them to confirm what is visible and changing;
use CCB authority sources to decide ownership, targets, and lineage.

### Tier 2: Screenshot Fallback

Screenshot capture is useful but more privacy-sensitive than pane text. For
self-supervision, the v1 primary evidence path is `tmux capture-pane` text:
bottom/current prompt content, recent scrollback, and short activity sampling.
Screenshots are fallback evidence when pane text is unavailable, blank,
misleading, or insufficient for a visual/layout failure:

- `ccb_pane_screenshot`: screenshot only a CCB-owned pane/window/sidebar/tool
  target and return an image artifact path plus metadata.
- `ccb_visual_inspect`: optional helper that summarizes a screenshot artifact
  with OCR or provider-native image understanding when available.

Prefer pane operations when:

- diagnosing provider TUI state, tool-window state, sidebar display, focus,
  geometry, scrollback, or visible stuckness;
- checking whether a pane is alive, dead, active, or showing fresh output;
- understanding what the user or provider currently sees without touching
  runtime authority;
- comparing pane evidence with daemon runtime records.

Use screenshots when:

- the sidebar layout or status display looks wrong;
- a provider TUI is visibly stuck but text capture is inconclusive;
- a managed tool window such as Neovim or browser output matters;
- tmux pane geometry, focus, or split layout is the suspected failure.
- self-supervision cannot classify whether an active provider pane is really
  working, stale, waiting for input, dead/blank, misframed, rate-limited, or
  showing a provider update/auth/quota/error screen from text capture alone.

Do not use screenshots for normal job tracing, queue diagnosis, config drift,
mailbox repair, or pane self-supervision when `tmux capture-pane` text is
sufficient. Use screenshots when the failure cannot be classified without
seeing visual layout or non-text state.

Do not use pane evidence as authority for:

- the configured agent list;
- restart target validity;
- job/message/reply lineage;
- config reload state;
- lifecycle ownership.

Those remain CCB control-plane and storage questions. Pane operations provide
live evidence that should be reconciled against daemon graph, lifecycle, lease,
runtime records, queue, inbox, and trace output.

### Tier 3: Controlled Mutations

These remain later tools and must call CCB control-plane commands:

- `ccb_restart_agent`
- `ccb_repair_retry`
- `ccb_repair_resubmit`
- `ccb_repair_ack`
- `ccb_clear_agent`
- `ccb_reload_project`

Mutating tools require explicit mutation intent, default busy checks, and exact
command semantics in the result.

## Screenshot Safety

Screenshot tools are read-only, but they can still capture sensitive terminal
contents. They need stricter bounds than JSON diagnostics:

- Target must be a current CCB configured agent, CCB sidebar, or configured
  managed tool window.
- The tool must not capture arbitrary desktop screens, unrelated tmux sessions,
  browser tabs outside the CCB namespace, or global monitors.
- The result should include target, timestamp, pane/window evidence, image path,
  dimensions, and whether OCR/vision analysis was attempted.
- The result must include the caller agent name when available.
- Image artifacts must be written to a CCB-owned artifact directory for the
  current project/runtime, not to `~/Pictures`, desktop folders, or unrelated
  shared locations. Source validation must keep artifacts inside the isolated
  test project selected by `CCB_SOURCE_HOME`/test root discipline.
- The tool should prefer text capture before screenshot unless the caller asks
  for visual evidence or the failure class is visual.
- The role must not infer runtime authority from screenshots. Screenshots are
  evidence only.

## First-Slice Recommendation

Ship the Role Pack draft with memory, skills, references, and a read-only
doctor helper first. For MCP, implement structured diagnostics and pane text
evidence before visual tools.

Make `tmux capture-pane` style text capture a first-class self-supervision
dependency for ambiguous agent-progress diagnosis. Screenshot tooling should be
available as a fallback path, but the normal v1 assessor path should start from
bottom/current pane text and short activity sampling.
