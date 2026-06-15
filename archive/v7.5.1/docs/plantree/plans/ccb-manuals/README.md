# CCB Manuals Plan

Date: 2026-06-10

## Purpose

Produce two deep CCB manuals from code and runtime evidence:

1. A LaTeX developer manual, roughly 100 pages, explaining CCB architecture,
   operating principles, source module boundaries, Archi/Hippo code-map
   evidence, and a dedicated chapter on communication logic.
2. A detailed user manual, produced after the developer manual, covering all
   configuration and command-system behavior with code-backed analysis.

This plan is analysis-first. It does not treat existing docs as enough by
themselves; every book chapter should point to source files, contract docs,
runtime artifacts, or command output.

## Current Mode

`manuals-built`.

Allowed write surface for the first phase:

- plan-tree files under this plan root;
- generated architecture evidence under existing Archi/Hippo artifact paths
  only after the refresh behavior is understood;
- later book drafts under a dedicated docs/manuals path after the outline and
  evidence ledger are accepted.

Do not edit CCB runtime implementation files for this documentation task unless
a later implementation bug is discovered and explicitly split into its own
work item.

## Current Outputs

- Developer manual source:
  [../../../manuals/developer-guide/](../../../manuals/developer-guide/)
- Developer manual PDF:
  [../../../manuals/developer-guide/build/main.pdf](../../../manuals/developer-guide/build/main.pdf)
  (82 pages, generated locally)
- User manual source:
  [../../../manuals/user-guide/](../../../manuals/user-guide/)
- User manual PDF:
  [../../../manuals/user-guide/build/main.pdf](../../../manuals/user-guide/build/main.pdf)
  (38 pages, generated locally)
- `ccb_self` expert Markdown guide:
  [../../../manuals/ccb-self-expert-guide.md](../../../manuals/ccb-self-expert-guide.md)
  (role-facing architecture, command, config, communication, diagnosis, and
  recovery guidance)

## File Map

- [roadmap.md](roadmap.md): phase sequence, done/in-progress/next/deferred
  state, and execution gates.
- [open-questions.md](open-questions.md): unresolved scope, language, output,
  and evidence questions.
- [topics/book-scope.md](topics/book-scope.md): manuals, audiences, non-goals,
  and acceptance criteria.
- [topics/evidence-workflow.md](topics/evidence-workflow.md): code, docs,
  Archi/Hippo, runtime, and CLI evidence collection workflow.
- [topics/developer-manual-outline.md](topics/developer-manual-outline.md):
  proposed 100-page LaTeX developer manual structure.
- [topics/user-manual-outline.md](topics/user-manual-outline.md): proposed
  follow-up user manual structure.
- [topics/communication-logic-analysis-plan.md](topics/communication-logic-analysis-plan.md):
  source-backed plan for the communication chapter.
- [topics/communication-source-inventory.md](topics/communication-source-inventory.md):
  initial source inventory for the end-to-end ask, dispatcher, mailbox,
  finalization, and callback paths.
- [topics/command-config-inventory.md](topics/command-config-inventory.md):
  first-pass inventory of CLI command groups, role/tool management commands,
  and configuration grammar sources for the user manual.
- [topics/latex-production-plan.md](topics/latex-production-plan.md): book
  source layout, build path, figures, citations, and review gates.
- [history/evidence-ledger-2026-06-10.md](history/evidence-ledger-2026-06-10.md):
  accepted evidence log for the initial Archi/Hippo refresh and command/config
  source inventory.
- [decisions/README.md](decisions/README.md): index of active manual
  production decisions.

## Related Sources

Project-wide planning baseline:

- [../../baseline/README.md](../../baseline/README.md)
- [../../baseline/module-map.md](../../baseline/module-map.md)
- [../../baseline/runtime-flows.md](../../baseline/runtime-flows.md)
- [../../baseline/storage-and-state.md](../../baseline/storage-and-state.md)

Authoritative runtime and config contracts:

- [../../../ccbd-startup-supervision-contract.md](../../../ccbd-startup-supervision-contract.md)
- [../../../ccbd-lifecycle-stability-plan.md](../../../ccbd-lifecycle-stability-plan.md)
- [../../../ccbd-diagnostics-contract.md](../../../ccbd-diagnostics-contract.md)
- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../../../managed-provider-completion-reliability-plan.md](../../../managed-provider-completion-reliability-plan.md)
- [../../../ccb-provider-state-storage-boundary-plan.md](../../../ccb-provider-state-storage-boundary-plan.md)

Communication-specific sources:

- [../../../agent-mailbox-kernel-design.md](../../../agent-mailbox-kernel-design.md)
- [../../../ask-native-async-job-architecture.md](../../../ask-native-async-job-architecture.md)
- [../../../agent-message-timeout-retry-contract.md](../../../agent-message-timeout-retry-contract.md)
- [../../../ccbd-p3-p4-mailbox-cli-plan.md](../../../ccbd-p3-p4-mailbox-cli-plan.md)
- [../../../ccbd-ask-submit-fastpath-plan.md](../../../ccbd-ask-submit-fastpath-plan.md)
- [../ask-parameter-policy/README.md](../ask-parameter-policy/README.md)

Provider/session sources:

- [../../../codex-session-isolation-contract.md](../../../codex-session-isolation-contract.md)
- [../../../claude-session-isolation-contract.md](../../../claude-session-isolation-contract.md)
- [../../../gemini-session-isolation-contract.md](../../../gemini-session-isolation-contract.md)
- [../../../opencode-completion-contract.md](../../../opencode-completion-contract.md)

## First Evidence Snapshot

- Existing Archi summary: `.architec/architec-summary.md`, generated
  2026-05-29, mode `diff`.
- Current Hippos bundle state: `.hippos/bundle-state.json`, generated
  2026-06-10T13:56:37Z, with 1556 indexed files, 1869 manifest files, and
  1153 signature files.
- Legacy Hippo index: `.hippocampus/hippocampus-index.json`, generated
  2026-05-29, with 1518 indexed files and 4477 function dependencies. Treat
  this as historical unless explicitly comparing old and new code maps.
- `archi --help` is available from `/home/bfly/.nvm/versions/node/v22.20.0/bin/archi`.
- `archi --check .` on 2026-06-10 reported `Archi preflight OK` and
  `Hippos bundle: refreshed`. Follow-up inspection showed that the current
  refresh target is `.hippos/`, while `.hippocampus/` is a legacy snapshot.
- Initial manual analysis output:
  `.architec/manual-architecture-analysis-20260610.json`, advisory
  selected-scope diff review with current Hippos snapshot context.

## Scope

In scope:

- Developer manual architecture explanation based on current source and
  contract docs.
- Code map and architecture analysis using Archi/Hippo artifacts.
- Dedicated communication logic chapter covering ask, mailbox, queue, callback,
  dispatcher, provider execution, reply detection, artifacts, watch/pend/trace,
  and failure handling.
- User manual after the developer manual, covering config syntax, roles,
  provider profiles, command system, lifecycle commands, communication commands,
  diagnostics, update/install, and operational workflows.
- LaTeX source with figures, tables, cross references, and a reproducible build
  command.

Out of scope for the first planning phase:

- Publishing or packaging the manuals.
- Rewriting public README content.
- Changing runtime behavior to match documentation.
- Treating generated architecture output as authoritative without direct code
  reading.

## Acceptance Criteria

The developer manual is ready when:

- the LaTeX source builds cleanly;
- the table of contents covers architecture, lifecycle, provider integration,
  storage/state, rolepacks, memory, diagnostics, tests, and communication
  logic;
- every major claim links to source files, contract docs, or generated
  evidence;
- the communication chapter includes at least one end-to-end ask flow and one
  callback/artifact flow;
- stale or uncertain Archi/Hippo evidence is explicitly labeled.

The user manual is ready when:

- all public command groups are inventoried from current CLI parser/help code;
- all config grammar and project/runtime files are tied back to contracts or
  source;
- examples are sanitized and runnable in an external project;
- verification commands are recorded for generated examples.
