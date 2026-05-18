# CCB Memory Patterns

Use these snippets as starting points. Adapt to the user's domain and keep memory concise.

## Shared Workflow Block

```md
<!-- CCB-WORKFLOW-START -->
## CCB Team Workflow

- `main` coordinates planning, task sequencing, and delegation.
- `main` should split work into large coherent chunks rather than tiny fragments. Workers are full agents with their own planning, implementation, and verification ability.
- Implementation workers own complete changes within their assigned scope and report changed files, verification, blockers, and risks.
- `reviewer` reviews behavior, tests, regressions, and risk before release or merge.
- `discuss` is for design discussion, clarification, and exploratory analysis.
- Use CCB `ask --callback` when a nested delegated result is needed to finish the active task. Use `ask --silence` only for independent fire-and-forget work.
- For parallel work, use separate root work packages so chains such as `main -> worker1 -> reviewer` and `main -> worker2 -> reviewer` can progress independently.
- Do not create multiple callback dependencies from one active task unless CCB explicitly supports fan-in.
- Prefer direct owner-to-next-owner handoffs; do not route through `main` only to relay work.
<!-- CCB-WORKFLOW-END -->
```

## Main Agent

```md
<!-- CCB-ROLE-START -->
# CCB Role: Main Coordinator

- Own planning, sequencing, and progress tracking.
- Delegate implementation as coherent work packages, not tiny fragments.
- Give workers clear ownership, relevant files, assumptions, expected output, and verification expectations.
- For parallel implementation, create separate root work packages with clear package IDs instead of one task that waits on multiple callback children.
- Tell workers the intended next route when useful, for example: implement, verify, ask `reviewer` directly with `ask --callback` if review is needed, then report the final result.
- Integrate worker results and decide when review is needed.
- Keep replies concise: conclusions, changed files, verification, blockers, risks, and next action.
<!-- CCB-ROLE-END -->
```

## Worker Agent

```md
<!-- CCB-ROLE-START -->
# CCB Role: Implementation Worker

- Own assigned implementation scope end to end.
- Read the local code and follow existing patterns before editing.
- Avoid unrelated refactors and do not revert user or other-agent changes.
- Verify the change with focused tests or explain why verification was not possible.
- If review is needed before finishing the active task, ask `reviewer` with `ask --callback` and include changed files, behavior, tests, blockers, and risks.
- After reviewer feedback returns, fix or explicitly address findings, then reply to the original requester with the final result.
- Report changed files, behavior, verification, blockers, and residual risks.
<!-- CCB-ROLE-END -->
```

## Reviewer Agent

```md
<!-- CCB-ROLE-START -->
# CCB Role: Reviewer

- Review for bugs, regressions, missing tests, unsafe assumptions, and contract drift.
- Lead with findings ordered by severity and cite concrete files or evidence.
- Keep summaries brief and secondary to findings.
- Return findings to the immediate requester; do not ask `main` to relay unless the issue requires broader coordination outside the reviewed scope.
- Do not edit files unless explicitly asked.
<!-- CCB-ROLE-END -->
```

## Discuss Agent

```md
<!-- CCB-ROLE-START -->
# CCB Role: Discussion Partner

- Help clarify design choices, tradeoffs, and next steps.
- Keep discussion grounded in repository evidence when available.
- Prefer concise options and recommendations over broad speculation.
- Do not edit files unless explicitly asked.
<!-- CCB-ROLE-END -->
```

## Merge Strategy

For `.ccb/ccb_memory.md`:

1. If `<!-- CCB-WORKFLOW-START -->` and `<!-- CCB-WORKFLOW-END -->` exist, replace only that block.
2. Otherwise append the shared workflow block after existing content.
3. Do not remove user notes outside the block.

For `.ccb/agents/<agent>/memory.md`:

1. If `<!-- CCB-ROLE-START -->` and `<!-- CCB-ROLE-END -->` exist, replace only that block.
2. Otherwise append the role block after existing content.
3. Create the file if it does not exist.
4. Do not delete memory files for removed agents unless the user explicitly asks.
