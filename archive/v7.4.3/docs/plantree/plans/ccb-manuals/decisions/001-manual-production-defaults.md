# Manual Production Defaults

Date: 2026-06-10

## Context

The user requested two deep CCB manuals and explicitly asked to analyze before
execution. The first execution pass needs stable defaults for language,
source layout, evidence policy, build artifacts, and runtime validation so
manual drafting can begin without waiting for more input.

Related topics:

- [../topics/book-scope.md](../topics/book-scope.md)
- [../topics/evidence-workflow.md](../topics/evidence-workflow.md)
- [../topics/developer-manual-outline.md](../topics/developer-manual-outline.md)
- [../topics/user-manual-outline.md](../topics/user-manual-outline.md)
- [../history/evidence-ledger-2026-06-10.md](../history/evidence-ledger-2026-06-10.md)

## Decision

- Primary manual prose will be Chinese. Source identifiers, commands, config
  keys, file paths, API names, and code terms remain in English.
- Developer manual source will live under `docs/manuals/developer-guide/`.
- User manual source will later live under `docs/manuals/user-guide/`.
- Generated PDFs are build artifacts and are not committed by default. Commit
  LaTeX source, figures, bibliography/source-map files, and build scripts.
- The developer manual targets about 100 pages as a density and coverage goal,
  not an exact page count. Diagrams and appendices may move the final count.
- The user manual will be a second LaTeX book for the first pass. A web or
  Markdown manual can be generated later if the project needs it.
- The first developer-manual draft will use direct source reading, contract
  docs, current `.hippos/` evidence, and the advisory
  `.architec/manual-architecture-analysis-20260610.json` output. A full
  LLM-backed `archi --full` run is optional follow-up evidence, not a drafting
  blocker.
- Runtime validation examples use `/home/bfly/yunwei/test_ccb2` with isolated
  `HOME=/home/bfly/yunwei/test_ccb2/source_home` and
  `CCB_SOURCE_HOME=/home/bfly/yunwei/test_ccb2/source_home` unless a test is
  explicitly about inherited provider configuration.
- Main manual citations should use current `.hippos/` evidence. Legacy
  `.hippocampus/` evidence may appear only as historical comparison or stale
  evidence notes.

## Consequences

- The first LaTeX source tree can be created without asking more questions.
- Help-output snapshots and runtime examples remain required before finalizing
  the user manual, but they do not block the developer manual draft.
- If the user later chooses a different primary language, PDF policy, or manual
  format, this decision should be superseded rather than silently rewritten.
