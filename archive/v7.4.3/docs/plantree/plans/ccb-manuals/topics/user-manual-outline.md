# User Manual Outline

Date: 2026-06-10

## Proposed Title

`CCB User Manual: Configuration, Commands, and Multi-Agent Workflows`

## Sequencing

Start this book only after the developer manual has a buildable complete draft.
The user manual should reuse verified concepts and diagrams, but present them
as workflows and references rather than internals.

## Proposed Structure

1. Getting Started
   - install/update assumptions, project anchor, first `ccb` start, tmux
     expectations, safe reset.
2. Project Configuration
   - config file locations, `version = 2`, windows, agents, provider profiles,
     roles, sidebar, tool windows, compact examples.
3. Daily Operation
   - start, attach, focus, sidebar, project view, status, reload, restart,
     clear, kill.
4. Asking Agents
   - direct asks, routes, callback, silence, compact, artifact request/reply,
     watch, pend, queue, inbox, ack, trace, retry, resubmit, cancel.
5. Roles And Skills
   - install/list/show/add/update/doctor roles, role ids, role locks, inherited
     skills, projected skills and commands.
6. Provider Setup
   - Codex, Claude, Gemini, OpenCode and provider-home/profile behavior at the
     user-facing level.
7. Memory Model
   - shared memory, agent private memory, provider user memory, provider-native
     project memory, role memory, update behavior.
8. Diagnostics And Troubleshooting
   - doctor, ps, logs, diagnostics bundles, common startup failures, storage
     and provider-state issues.
9. Advanced Workflows
   - multi-window layouts, worktrees, callback chains, artifact handoffs,
     manual recovery, source-runtime testing for contributors.
10. Command Reference
    - generated from current parser/help output and grouped by task.
11. Configuration Reference
    - generated from contract docs and config loader source.

## Evidence Requirements

- Every command group should be checked against current CLI parser/help output.
- Every config field should be checked against config loader source and
  `ccb-config-layout-contract.md`.
- Examples should use sanitized project names and avoid this checkout's live
  `.ccb` runtime state.

