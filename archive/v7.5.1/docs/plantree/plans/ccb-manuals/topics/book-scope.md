# Book Scope

Date: 2026-06-10

## Audiences

Developer manual:

- maintainers who need to understand CCB internals;
- agents or contributors modifying ccbd, communication, provider backends,
  config loading, runtime state, rolepacks, diagnostics, or tests;
- reviewers who need a source-backed architecture reference.

User manual:

- users configuring CCB projects;
- users operating multi-agent projects through commands and sidebar workflows;
- maintainers explaining command/config behavior without asking readers to read
  source code.

## Developer Manual Goals

- Explain the architecture from project entrypoint to daemon, tmux namespace,
  managed agents, provider homes, mailbox/dispatcher, and diagnostics.
- Use Archi/Hippo output as a code-map and hotspot guide, but verify claims
  against source.
- Give communication logic a full chapter rather than a short command
  reference.
- Preserve the distinction between authority, evidence, residue, and generated
  runtime state.
- Include enough file paths and contracts that a future maintainer can resume a
  bug fix from the manual.

## User Manual Goals

- Explain every command group and config surface from current source/help
  output.
- Provide safe examples for startup, config, ask, callback, artifacts,
  roles, provider profiles, diagnostics, reload, clear, kill, update, and
  troubleshooting.
- Avoid exposing project-local private runtime state as public configuration.

## Non-Goals

- The manuals are not marketing pages.
- The manuals do not replace the authoritative source contracts under `docs/`.
- The manuals do not certify runtime correctness; tests and source validation
  remain separate.
- The manuals should not teach users to edit runtime residue under
  `.ccb/ccbd/` or provider session files.

## Page Budget

Developer manual rough page allocation:

| Area | Pages |
| --- | ---: |
| Orientation and concepts | 8 |
| Architecture and module map | 14 |
| Startup, lifecycle, and supervision | 12 |
| Config and topology | 8 |
| Communication logic | 18 |
| Provider integration and session isolation | 12 |
| Memory, rolepacks, skills, and provider homes | 10 |
| Storage, diagnostics, testing, and release gates | 10 |
| Archi/Hippo analysis, hotspots, appendices | 8 |

The page count is a guide. Completeness and source-backed accuracy matter more
than hitting exactly 100 pages.

