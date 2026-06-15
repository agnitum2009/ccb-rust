# Artifact Transport Policy

Date: 2026-06-07

## Principle

Artifact flags are chosen from content-preservation needs, not from task
dependency shape. They are compatible with plain ask, callback, silence, and
compact modes.

Automatic spill for request or reply text over 4 KiB is a safety fallback. It
should not be the only reason agents use artifact transport. Agents should use
artifact flags proactively when exact transient text matters.

## Use `--artifact-request`

Use request artifacts when the ask body contains exact material that the target
cannot reliably read from the workspace:

- pasted logs or command output
- external diffs or patch text
- copied file content from outside the repo
- long configuration snippets
- JSON, YAML, tables, or other structured text that should not be summarized
- any transient text where losing details would change the task

Prefer passing repo paths when the target agent can read the relevant files
directly. Do not copy large repo files into the ask body just to force artifact
transport.

## Use `--artifact-reply`

Use reply artifacts when the expected result should be preserved as full text:

- complete review reports
- long evidence summaries
- generated documents
- structured findings
- output another agent or a later callback continuation must read without
  summary loss

## Use `--artifact-io`

Use artifact IO when both request context and expected result need artifact
backing.

## Keep Inline Text

Keep the request inline when it is a short natural-language task with no exact
transient material. For the reply, plain inline text is only preferred when the
answer is short enough that no compact or artifact-reply intent is needed.

For consultation, analysis, review reports, generated documents, or structured
findings, prefer `--artifact-reply` even if the request itself is short.
