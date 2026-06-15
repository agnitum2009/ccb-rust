# Multi-Agent Research Notes

Date: 2026-05-25

Role: Topic
Status: Active research notes
Read when: Writing the README multi-agent comparison
Related: [multi-agent-positioning-and-comparison.md](multi-agent-positioning-and-comparison.md)

## Research Scope

The README comparison should be written only after checking current primary
sources for:

- Claude Code native multi-agent features;
- OpenHive at `aden-hive/hive`;
- CCB's own visible, terminal-native multi-agent model.

This file records source-backed findings and the comparison implications.

## Claude Code Native Multi-Agent

Primary sources:

- <https://code.claude.com/docs/en/agents>
- <https://code.claude.com/docs/en/sub-agents>
- <https://code.claude.com/docs/en/agent-teams>
- <https://code.claude.com/docs/en/agent-view>
- <https://code.claude.com/docs/en/worktrees>

### Observed Capabilities

Claude Code's current multi-agent/parallel-work surface is broader than
"implicit subagents":

- `subagents`: delegated workers inside one session, each with its own context,
  custom prompt, tool access, independent permissions, and summary return.
- `agent view`: a dashboard-like view for dispatching and monitoring background
  Claude Code sessions.
- `agent teams`: multiple coordinated Claude Code sessions with a lead,
  teammates, shared task list, inter-agent messaging, and centralized
  management.
- `worktrees`: git checkout isolation for parallel sessions touching files.
- `/batch`: planned split of a large change into multiple worktree-isolated
  subagents that open pull requests.

Subagent configuration supports focused role prompts, tool allow/deny lists,
model selection, permission mode, MCP server scoping, skills, hooks, persistent
memory, effort, background execution, and optional worktree isolation.

Agent teams add direct teammate interaction, shared tasks, task dependencies,
teammate messaging, split-pane or in-process display modes, and hooks for
quality gates.

### Strengths For README Comparison

- Native to Claude Code, so setup and mental model are close to existing Claude
  users.
- Strong context isolation story for research, logs, codebase exploration, and
  focused worker roles.
- Supports cost routing by letting subagents use cheaper/faster models where
  appropriate.
- Provides a native path from lightweight subagents to more coordinated agent
  teams.
- Worktree isolation is first-class for parallel file edits.

### Caveats For README Comparison

- Claude-centered: workers are Claude Code sessions. Other tools/providers are
  integrated indirectly, such as through MCP, not as first-class visible CLI
  panes beside Claude.
- Agent teams are experimental and disabled by default.
- Agent teams increase token usage because each teammate has its own context.
- Agent teams have documented limitations around session resumption, task
  coordination, shutdown, one team at a time, no nested teams, fixed lead, and
  permission behavior.
- Split-pane teammate mode depends on tmux or iTerm2 and has terminal
  limitations.
- Agent teams do not make project-scoped cross-provider lifecycle, `kill`,
  rebuild, or CCB-style named provider runtime ownership the central concept.

### Implication

The README should not present Claude Code native multi-agent as weak. It is a
strong option for Claude-first users. CCB's distinction is different: visible,
provider-mixed, project-scoped CLI agent collaboration with explicit lifecycle
control.

## OpenHive

Primary sources:

- <https://github.com/aden-hive/hive>
- <https://raw.githubusercontent.com/aden-hive/hive/main/README.md>
- <https://docs.adenhq.com/>

### Observed Capabilities

OpenHive positions itself as an agent harness for production workloads. Its
README emphasizes:

- zero-setup, model-agnostic execution harness;
- dynamically generated multi-agent topologies from a plain-language objective;
- strict graph-based execution DAG;
- concurrent parallel task execution;
- persistent, role-based memory;
- deterministic fault tolerance and crash recovery;
- state observability and asynchronous execution across LLM providers;
- human-in-the-loop control;
- cost limits, audit trails, and production reliability;
- business-system connectivity through tools/MCP;
- browser interface opened by the quickstart.

The README explicitly frames Hive as useful when the bottleneck is the harness
around the model: state persistence, recovery, observability, cost enforcement,
auditability, and production workflow execution.

### Strengths For README Comparison

- Strong production harness framing: state, recovery, observability, cost, and
  human oversight.
- Model-agnostic and intended to connect to many business systems.
- Graph/DAG model is a better fit for repeatable long-running workflows than
  ad-hoc terminal coordination.
- Human-in-the-loop and auditability language is much stronger than a simple
  coding-agent pane manager.

### Caveats For README Comparison

- It is not primarily a terminal workspace for keeping existing CLI coding
  agents visible side by side.
- It introduces a harness/runtime and browser interface rather than preserving
  the user's current terminal-first CLI agent workflow as the main surface.
- Its production/business-process framing is broader than README's first target:
  new CCB users coordinating coding agents in a project.

### Implication

The README should compare Hive respectfully as a production multi-agent harness.
CCB should not claim to be "more production" than Hive. CCB's position is:
terminal-native, visible, provider-mixed CLI collaboration for project work,
with `ccb` owning start/attach/recover/kill and v7 sidebar visibility.

## CCB Comparison Frame

CCB should be presented as:

- explicit named agents rather than hidden workers;
- mixed providers in one project workspace;
- visible tmux panes plus v7 sidebar;
- project-scoped config, shared memory, worktree options, and runtime
  supervision;
- explicit `/ask`, `$ask`, callback, and broadcast-style coordination;
- complex workflows configured explicitly through named agents, windows,
  worktree isolation, memory, model/API choices, and handoff routes;
- lifecycle commands users can understand: `ccb`, `ccb -s`, `ccb -n`,
  `ccb kill`, and `ccb kill -f`;
- a fit for users who want to see and control the team from the terminal.

CCB should not be positioned as:

- a replacement for Claude Code's native subagents when the user is Claude-only;
- a magic auto-orchestrator that hides coordination.

CCB should also not be described as unable to run complex workflows. It can
support complex workflows when the user explicitly configures the team layout,
windows, worktree isolation, memory, model choices, and ask/callback handoff
routes. The distinction from OpenHive is generated harness orchestration versus
explicit terminal-native configuration and visibility.

## Recommended README Comparison Table

| Approach | Best fit | Main advantage | Main tradeoff |
| :--- | :--- | :--- | :--- |
| Claude Code native multi-agent | Claude-first users who want native subagents, background sessions, teams, and worktrees. | Deep provider integration and rich role/context/tool controls. | Claude-centered; agent teams are experimental; cross-provider CLI lifecycle is not the center. |
| Hive / OpenHive | Users who want a harness to generate and run workflow graphs with state, recovery, observability, cost control, and human oversight. | Strong production harness around long-running multi-agent workflows. | Broader harness/browser workflow, not primarily a visible terminal workspace for existing CLI agents. |
| CCB | Developers who want explicitly configured named CLI agents from multiple providers visible and controllable in one project tmux workspace. | Terminal-native visibility, provider mixing, sidebar, ask routing, worktree options, project lifecycle control, and configurable complex workflows. | Requires explicit team/workflow configuration and a small CCB/tmux operating model. |

## Recommended Comparison Presentation

Use two layers:

- A short visible decision table with rows for model choice, control, context,
  visibility, and recovery.
- A folded detail table with additional rows for tools/skills, safe parallel
  work, handoff, lifecycle ownership, learning cost, and wrong-fit cases.

This layout keeps the opening intuitive while still answering deeper technical
questions.

## Plain-Language Writing Rules

The public README should use questions users recognize:

- "Can I use different model providers together?"
- "Can I control what each agent can see and do?"
- "Does each agent keep a clean role and context?"
- "Can I see what is happening without guessing?"
- "What happens when an agent gets stuck?"
- "When should I not use this approach?"

Avoid leading with terms such as "graph DAG", "provider heterogeneity",
"lifecycle authority", or "observability plane". Those are useful internally,
but README readers need practical consequences first.

Recommended short summary:

- Claude Code native multi-agent: good when you want Claude to manage
  delegation inside Claude Code.
- Hive / OpenHive: good when you want a generated workflow harness with state,
  recovery, audit, and human oversight.
- CCB: good when you want several different CLI agents, with separate roles and
  models, visible and controllable in one project terminal workspace. CCB can
  also support complex workflows, but you configure them explicitly.

## Remaining Research Gaps

- Verify current Claude Code version/platform limits before final README text,
  especially agent teams and split-pane mode.
- Verify current CCB supported provider list and which provider names should be
  shown in the opening table.
- Decide how much source attribution should appear in the public README versus a
  folded "comparison notes" section.
