# Ask Parameter Policy Roadmap

Date: 2026-06-07

## Done

- Clarified that `--callback` and `--silence` express task relationship, not
  content transport.
- Clarified that result intent is now the first selector: `--silence` for
  publish/execute without successful result needs, `--compact` for distilled
  results, `--artifact-reply` for full text results, and plain `ask` only for
  short inline work.
- Clarified that artifact flags are orthogonal to route flags: artifacts
  preserve content, while callback and silence describe dependency shape.
- Clarified that automatic 4 KiB artifact spill is fallback behavior, not the
  primary smart-selection policy.
- Clarified that `A --silence -> B` does not auto-complete B; B still runs an
  active job, and B-to-C routing depends on whether B needs C's result.
- Clarified that callback chains require each waiting hop to use callback; CCB
  owns continuation propagation after callback edges exist.

## In Progress

- Keep docs and inherited ask skill wording aligned around result-intent-first
  selection and proactive use of `--silence`, `--compact`, and
  `--artifact-reply`.

## Next

- Keep Codex, Claude, and Droid inherited ask skill policy wording aligned.
- Add or maintain template checks for result-intent and artifact-policy text.
- Run focused unit tests for ask skill templates and ask route option mapping.
- Re-run external source-under-test validation from
  `/home/bfly/yunwei/test_ccb2` when the matrix or skill wording changes.

## Deferred

- Any automatic callback routing behavior in `ccbd`.
- Any CLI warning for suspicious flag combinations.
- Any README expansion beyond a short mention of artifact ask modes.

## Release Gate

This policy update is ready when:

- inherited ask skills explain result intent before request fidelity;
- no inherited ask skill contains Chinese text or the old `ccb ask` command form;
- static template tests pass;
- ask route option mapping tests still pass;
- external `ccb_test` starts from an isolated project and projects updated ask
  skill text into managed provider homes where those providers are configured.
