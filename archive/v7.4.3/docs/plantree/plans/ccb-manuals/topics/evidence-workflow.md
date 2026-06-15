# Evidence Workflow

Date: 2026-06-10

## Evidence Classes

Use four evidence classes while writing:

1. Contract docs: existing architecture and behavior contracts under `docs/`.
2. Source code: current files under `lib/`, `config/`, `ccb`, `ccb_test`,
   tests, and install/update scripts.
3. Generated analysis: `.architec/*`, current `.hippos/*`, legacy
   `.hippocampus/*`, and any refreshed Archi/Hippo artifacts produced for this
   work.
4. Runtime/CLI evidence: command help output, focused `ccb_test` validation
   from the external test project, and sanitized runtime records.

Generated analysis is advisory. Manual text should not copy Archi conclusions
without checking source and contract docs.

## Initial Commands

Already run:

```bash
archi --help
archi --check .
archi --refresh-from-hippos --allow-static --out .architec/manual-architecture-analysis-20260610.json .
```

Observed:

- `archi --help` confirms the available command shape.
- `archi --check .` reported `Archi preflight OK`.
- The same command reported `Hippos bundle: refreshed`.
- Refresh writeback targets `.hippos/`, not legacy `.hippocampus/`.
- `.hippos/bundle-state.json` was generated on 2026-06-10 with 1556 indexed
  files, 1869 manifest files, and 1153 signature files.
- `.hippocampus/bundle-state.json` remains a legacy 2026-05-29 bundle with
  1518 indexed files, 1819 manifest files, and 1121 signature files.
- `.architec/manual-architecture-analysis-20260610.json` is an advisory
  selected-scope diff review. It should not be treated as the full manual
  architecture map.

Candidate next commands:

```bash
archi --full --refresh-from-hippos .
archi --full --refresh-from-hippos --out .architec/manual-architecture-analysis.json .
```

Before running a full analysis, decide whether an LLM-backed run is acceptable
for the manual draft and record the output path in the evidence ledger.

## Artifact Ledger

Current ledger:

- [../history/evidence-ledger-2026-06-10.md](../history/evidence-ledger-2026-06-10.md)

Each entry should record:

- command;
- working directory;
- environment assumptions;
- output paths;
- timestamps;
- whether artifacts are fresh, stale, or advisory;
- manual chapters that consume the evidence.

## Code Inventory Strategy

Use `rg --files`, `rg`, `find`, and focused `sed` reads. Do not rely on broad
directory listings that include `__pycache__`.

Minimum inventories:

- CLI parser and command dispatch.
- ccbd request handlers.
- mailbox and message-bureau models/stores.
- dispatcher and provider execution services.
- provider backends and reply polling.
- config loader and runtime topology materialization.
- storage path layout and diagnostics bundle sources.
- tests covering each surface.

## Verification Strategy

Use source validation discipline from project memory:

- use `/home/bfly/yunwei/ccb_source/ccb_test`;
- run source runtime validation from `/home/bfly/yunwei/test_ccb2`;
- set isolated `HOME` and `CCB_SOURCE_HOME` unless intentionally testing real
  inherited provider config;
- do not run source runtime commands from the `ccb_source` checkout itself.
