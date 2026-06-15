# Multi-Agent Positioning And Comparison

Date: 2026-05-25

Role: Topic
Status: Active planning
Read when: Writing the opening README narrative
Related: [readme-information-architecture.md](readme-information-architecture.md), [multi-agent-research-notes.md](multi-agent-research-notes.md), [../decisions/002-readme-publication-defaults.md](../decisions/002-readme-publication-defaults.md)

## Purpose

Add a first README section that explains why multi-agent work exists before
showing CCB itself. The section should help new users understand:

- when one agent is enough;
- when multiple agents become useful;
- why different multi-agent systems feel different in practice;
- why CCB chooses visible, terminal-native coordination.

## Source Baseline To Verify

Use current primary sources before writing final README wording:

- Claude Code subagents support automatic and explicit delegation, custom tools,
  model selection, memory, background mode, and optional worktree isolation:
  <https://code.claude.com/docs/en/sub-agents>
- OpenAI Agents SDK documents LLM-driven orchestration, agents-as-tools,
  handoffs, code-driven chains, evaluator loops, and parallel execution:
  <https://openai.github.io/openai-agents-python/multi_agent/>
- Hive target is OpenHive at <https://github.com/aden-hive/hive>. Its README
  positions Hive as a multi-agent harness for production workloads with state
  management, failure recovery, observability, and human oversight. It describes
  a zero-setup, model-agnostic execution harness that dynamically generates
  graph-based multi-agent topologies for complex, long-running workflows.

Detailed research notes live in
[multi-agent-research-notes.md](multi-agent-research-notes.md). Use that file
for claims and caveats before drafting the README.

## Section 1: Single Agent Versus Multi Agents

Visible default content should be short and concrete:

| Work style | Best when | Weak point |
| :--- | :--- | :--- |
| Single agent | The task is local, sequential, and one context is enough. | One conversation must hold planning, implementation, review, testing, tools, skills, and waiting. |
| Multiple agents | Work benefits from separated roles, parallel investigation, independent review, or different models/tools. | Coordination, visibility, state isolation, and cleanup become product problems. |

Key message:

- Multi agents are not automatically better.
- They become useful when role separation improves quality or latency.
- The hard part is not spawning more models; it is keeping responsibility,
  state, visibility, and handoff semantics understandable.

Single-agent limitations to state plainly:

- Role mixing reduces context focus. Planning, coding, review, QA,
  investigation, and project memory compete inside one conversation.
- Task complexity ceiling is lower. A single agent must serialize decisions,
  execution, verification, and recovery instead of splitting work into
  independent executable tracks.
- Cost pressure is worse. If all tasks share one agent, users often default to
  the strongest model for everything, including routine review, search, or
  mechanical edits.
- Management gets harder. One agent owns all instructions, all tools, all
  skills, and all project rules, which makes the context heavier and harder to
  audit.
- Tool/skill concentration lowers efficiency. A broad toolset increases
  selection noise and makes specialized workflows harder to isolate.
- Execution is serial. While the agent is implementing, the user waits; review,
  research, alternative exploration, and test triage cannot naturally progress
  in parallel.

README wording should avoid attacking single-agent products. The point is that
single-agent mode is good for focused sequential tasks, but starts to strain
when one conversation becomes the planner, builder, reviewer, researcher, QA,
and operations console at the same time.

## Section 2: Why Multi Agents Become Necessary

Use three concrete examples:

- Builder plus reviewer: one agent edits, another critiques edge cases and
  tests.
- Research plus implementer: one agent investigates docs/issues while another
  changes code.
- Parallel worktrees: agents explore alternatives without fighting over the same
  working tree.

Avoid abstract claims such as "10x productivity" unless there is measured local
evidence. The README should frame this as an engineering workflow advantage:
specialization, isolation, visibility, and recoverability.

## Section 3: Compare Multi-Agent Approaches

Use a compact visible table first:

| Approach | Core idea | Strength | Tradeoff |
| :--- | :--- | :--- | :--- |
| Claude Code native multi-agent | Use Claude's built-in subagents, agent view, agent teams, worktrees, and batch flows inside the Claude Code ecosystem. | Deep native integration, automatic/explicit delegation, context isolation, model/tool controls, and Claude-native team coordination. | Claude-centered; agent teams are experimental, can cost more tokens, and do not make cross-provider CLI lifecycle the center. |
| Hive / OpenHive | Generate and run production-oriented multi-agent graphs from an objective. | Strong harness story for state, recovery, observability, human oversight, cost control, auditability, and long-running workflows. | Production harness framing; less focused on keeping existing CLI agents visibly side by side in one terminal workspace. |
| CCB | Run explicitly configured named CLI agents in a project-owned tmux workspace with visible panes, sidebar, ask routing, worktree options, and project lifecycle control. | Terminal-native, provider-mixed, visible, project-scoped, and capable of complex workflows when the team, windows, worktrees, memory, and handoff routes are configured. | Requires explicit configuration and a small tmux operating model instead of hiding orchestration. |

Then fold deeper detail under `<details>`.

## Plain-Language Comparison Draft

Use this style in the README instead of framework-heavy wording:

### Visible Short Table

Keep this table visible by default. It should answer the first decision in
under one screen.

| If you care most about... | Use Claude Code native multi-agent when... | Use Hive / OpenHive when... | Use CCB when... |
| :--- | :--- | :--- | :--- |
| Model choice | you are happy staying mostly in Claude Code. | you want a model-agnostic production harness. | you want Claude, Codex, Gemini, OpenCode, and other CLI agents side by side. |
| Control | you want Claude-native prompts, tools, permissions, skills, memory, and worktrees. | you want generated workflow policies, budgets, human approval, recovery, and audit. | you want to explicitly design each agent's provider, model, API route, memory, tools, workspace, pane, window, and ask route. |
| Context | you want subagents to keep side work out of the main Claude conversation. | you want persistent workflow state and role memory across long-running processes. | you want project memory plus per-agent memory, with each agent's provider state isolated and visible. |
| Visibility | you are comfortable watching work inside Claude Code's own views. | you want browser/harness observability for a generated workflow. | you want every CLI agent visible in terminal panes plus the v7 sidebar. |
| Recovery | you rely on Claude's native session/team behavior. | you need production recovery, cost limits, and audit trails. | you want explicit project commands: start, attach, rebuild, kill, force cleanup, and pane recovery. |

### Expanded Detail Table

Fold this under `<details>` so the README stays readable.

| Question users actually care about | Claude Code native multi-agent | Hive / OpenHive | CCB |
| :--- | :--- | :--- | :--- |
| Can I use different model vendors together? | Mostly Claude-first. Good if your team already standardizes on Claude Code. | Yes, the harness is model-agnostic and designed around many LLM providers. | Yes. Each named agent can use a different CLI/provider, model, API key, and base URL. |
| Can I decide what each agent is allowed to do? | Yes inside Claude Code: subagents can have focused prompts, tools, permissions, skills, memory, and worktrees. | Yes at workflow/runtime level: graph nodes, tools, policies, budgets, human intervention, and observability. | Yes at project level: each named agent has its own provider, workspace mode, memory, model/API shortcuts, pane, window placement, and ask route. |
| Will context stay clean? | Better than one chat: subagents get separate context and return summaries. Agent teams add shared task/message coordination. | Built for persistent workflow state and role memory across long-running processes. | Built around explicit project memory plus per-agent memory and isolated provider state, so roles can stay separate and visible. |
| Can I see what every agent is doing? | Claude has native views and team displays; visibility stays inside Claude Code. | Browser/harness observability for workflow execution. | Terminal panes plus v7 sidebar show named agents, windows, activity, and Comms in the same project workspace. |
| Can different agents use different tools/skills? | Yes, within Claude Code's subagent/team configuration model. | Yes, through workflow nodes, integrations, tools, and MCP-style connectivity. | Yes, by giving each named provider runtime its own projected memory, inherited skills/tools where supported, and workspace policy. |
| Can agents work in parallel safely? | Yes with subagents, background sessions, teams, and worktrees, especially inside Claude Code. | Yes through graph-based parallel execution and session isolation. | Yes through multiple visible agent panes and optional git-worktree isolation per agent. |
| Is it easy to hand work between agents? | Native handoff is strongest inside Claude Code's team/subagent surfaces. | Handoff is part of the generated workflow graph and runtime state. | Handoff is explicit through `/ask`, `$ask`, callback continuations, and Comms state. |
| Who owns the lifecycle? | Claude Code owns most of the agent/session experience. | Hive owns the generated workflow runtime/harness. | CCB owns the project backend, tmux namespace, configured agents, startup, attach, recovery, and shutdown. |
| What happens when work gets stuck? | Good within Claude's native session/team model, but lifecycle is Claude-centered. | Strong production recovery/audit/cost-control story. | Project lifecycle is explicit: start, attach, rebuild, kill, force cleanup, and pane recovery are CCB concepts. |
| How much do I need to learn? | Lowest if you already use Claude Code. | Higher if you need to understand the harness/workflow model. | Moderate: complex workflows are configured explicitly through CCB config, windows, worktrees, memory, and ask routes, plus a small tmux operating model. |
| When is it the wrong fit? | If you need mixed CLI providers as equal visible agents. | If you mainly want to operate existing coding CLIs in a terminal rather than run a production workflow harness. | If you want everything hidden behind one provider-native assistant or a production graph harness. |

Readable summary:

- Claude Code native multi-agent is the best "stay inside Claude" path. It
  reduces context pollution and gives strong role/tool controls, but it is still
  centered on Claude Code.
- Hive is the best "generate and run agent workflows as a production harness"
  path. It is about graph execution, state, recovery, observability, cost, and
  human oversight.
- CCB is the best "I want to explicitly configure and operate a visible team of
  CLI agents in this project" path. It supports complex workflows too, but the
  workflow is designed through named agents, windows, worktrees, memory, model
  choices, and ask routes rather than hidden behind an automatic harness.

### Suggested README Copy

Use concise language like:

```text
If you are Claude-first, Claude Code's native multi-agent features are the
shortest path: subagents and agent teams keep side work out of the main chat and
let Claude manage the team.

If you want an agent harness to generate and run long-running workflow graphs,
Hive is closer to that runtime: it focuses on graph execution, state, recovery,
observability, cost, and human oversight.

If you want several existing CLI agents from different providers visible in one
project, CCB is the explicit terminal workspace: every agent has a name, pane,
model, memory, workspace policy, window placement, and ask route. CCB can run
complex workflows, but you configure the team and handoff structure yourself.
```

README wording should stay concrete:

- Prefer "Can I mix Claude, Codex, Gemini, and OpenCode in one project?" over
  "provider heterogeneity".
- Prefer "Can I see what each agent is doing?" over "observability surface".
- Prefer "Does every agent get its own memory and tools?" over "context
  inheritance model".
- Prefer "How do I stop or recover the workspace?" over "lifecycle authority".

## Final Comparison Design

Recommended public README structure:

- visible: 2-3 sentence introduction plus the short decision table;
- folded: expanded detail table and source links;
- visible after fold: one sentence explaining why CCB picks visible
  coordination rather than hidden orchestration.

This keeps the opening intuitive while preserving enough detail for technical
readers.

## Claude Code Native Multi-Agent Notes

Frame this as provider-native orchestration, with Claude Code as the concrete
researched example:

- Claude Code's official "Agents and parallel work" surface compares subagents,
  agent view, agent teams, worktrees, and `/batch`.
- Subagents are best for delegated side tasks that should preserve the main
  conversation context and return a summary.
- Agent view dispatches and monitors independent background sessions.
- Agent teams coordinate multiple Claude Code sessions with a lead, teammates,
  shared task list, and inter-agent messaging, but are experimental and disabled
  by default.
- Worktrees isolate parallel sessions at the git checkout level.
- Claude subagents can use focused system prompts, tool restrictions, model
  choices, permission modes, skills, memory, hooks, and optional worktree
  isolation.

Positioning:

- Best for users already standardized on Claude Code who want native delegation
  and team coordination inside that ecosystem.
- Less ideal when the README's target user wants mixed providers, every agent as
  an explicit named CLI session, and project lifecycle controlled outside any
  one provider runtime.

## Hive / OpenHive Notes

Target: OpenHive at `github.com/aden-hive/hive`.

Observed public positioning:

- multi-agent harness for production workloads;
- state management, failure recovery, observability, and human oversight;
- zero-setup, model-agnostic execution harness;
- dynamic multi-agent topology generation;
- strict graph-based execution DAG;
- persistent role-based memory;
- asynchronous execution across LLM providers;
- human-in-the-loop control, cost limits, auditability, and production
  workload framing.

Positioning:

- Best for users whose bottleneck is production harness reliability around
  long-running agent workflows: state, recovery, observability, auditability,
  and cost controls.
- Different from CCB's terminal-native design, where the user's existing CLI
  agents remain visible in tmux panes, provider sessions are directly
  observable, and project lifecycle is controlled by `ccb`.

## CCB Notes

CCB's opening claim should emphasize:

- named agents, not anonymous hidden workers;
- provider mixing across Codex, Claude, Gemini, Kimi, OpenCode, and other
  supported CLIs;
- visible terminal panes plus v7 sidebar;
- `/ask`, `$ask`, callback, and broadcast-style coordination;
- project-scoped config, memory, worktree isolation, startup, attach, recovery,
  and kill/rebuild behavior;
- complex workflows configured explicitly through named agents, windows,
  worktree isolation, memory, model/API choices, and ask/callback routes;
- minimal tmux learning required, documented in the README.

Avoid implying CCB cannot handle complex workflows. The distinction is that CCB
asks the user to explicitly design the workflow through configuration and
visible handoffs, while OpenHive emphasizes generated graph/harness execution.
Also avoid claiming CCB replaces provider-native subagents or production
harnesses such as Hive. Position it as the best fit when the user wants
terminal-native, visible, project-scoped collaboration across multiple CLI
agents.

## Draft Opening Flow

1. Start with the single-agent pain:
   "A single agent is fine until the same conversation has to plan, code,
   review, test, remember project rules, and coordinate follow-up work."
2. Introduce multi-agent value:
   "Multi-agent coding is useful when roles should be separated and work should
   continue in parallel or in isolated worktrees."
3. State the coordination problem:
   "The hard part is seeing who is doing what, where state lives, and how to
   stop or recover the project."
4. Compare the three approach families.
5. Introduce CCB as the terminal-native visible team workspace.

## Open Verification Before README Text

- Confirm final Hive comparison wording against the current `aden-hive/hive`
  README before publishing.
- Verify current CCB-supported provider list before naming providers in the
  opening table.
- Decide whether to name Claude/OpenAI examples in the visible table or keep
  them inside a folded "official orchestration examples" section.
