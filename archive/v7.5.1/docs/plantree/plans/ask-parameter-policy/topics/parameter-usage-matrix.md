# Ask Parameter Usage Matrix

Date: 2026-06-08

## Quick Selector

Use this list before submitting an ask:

1. No delegation needed: answer directly.
2. Publish or execute a task, success result not needed: `--silence`.
3. Result wanted, but only status/findings/risks/next steps: `--compact`.
4. Consultation, analysis, review report, generated document, or full text
   needed: `--artifact-reply`.
5. Short question or short handoff where inline text is enough: plain `ask`.
6. Active parent cannot finish until the child result arrives: add
   `--callback`, then stop for continuation.
7. Request contains exact transient text: add `--artifact-request`; if reply
   also needs full preservation, use `--artifact-io`.

## Parameter List

| Parameter | Use When | Avoid When |
| --- | --- | --- |
| plain `ask` | Short question, short handoff, inline answer is enough | Execution, consultation, analysis, long reports, exact input, or full result needed |
| `--silence` | Task publication, execution, notification, smoke check, cleanup, sync; success result not needed | Caller needs a successful result, or current active task depends on the child |
| `--compact` | Caller wants a concise result: pass/fail, status, findings, risks, blockers, next actions | Full evidence or complete output must be preserved |
| `--callback` | Active CCB parent task cannot finish until child result arrives | Normal top-level dispatch without an active parent dependency |
| `--artifact-request` | Pasted logs, command output, external diff, copied contents, long config, JSON/YAML/table, structured transient text | Target can read the same repo file path directly |
| `--artifact-reply` | Consultation, analysis, complete report, generated doc, structured findings, long evidence, later agent/continuation must read full text | Caller does not need a successful result |
| `--artifact-io` | Exact request input and full reply both need artifact backing | Only one side needs preservation |

## Combination List

| Scenario | Recommended Flags | Notes |
| --- | --- | --- |
| Publish an execution task | `--silence` | Success should not interrupt; failures still surface |
| Publish execution with exact pasted input | `--silence --artifact-request` | Good for fire-and-forget with logs/config/diff input |
| Run a task and get short outcome | `--compact` | Use for status, pass/fail, short findings |
| Active parent needs short child result | `--callback --compact` | Child result resumes parent as a compact continuation |
| Active parent needs full child result | `--callback --artifact-reply` | Most useful callback combination for reports or analysis |
| Ask for full consultation or analysis | `--artifact-reply` | Weakens no-flag ask for analysis-heavy work |
| Ask with exact JSON/YAML/log input | `--artifact-request` | Add `--compact` or `--artifact-reply` based on result intent |
| Exact input plus full output | `--artifact-io` | Add `--callback` if an active parent depends on it |
| Broadcast short notification | plain `ask` or `--silence` | Prefer `--silence` when successful acknowledgements are not useful |
| Short question or short handoff | plain `ask` | Keep this narrow |

## Discouraged Combinations

- `--callback --silence`: callback needs a result; silence says success should
  not interrupt.
- `--silence --artifact-reply`: silence says no successful result is needed;
  artifact-reply preserves a successful result.
- Plain nested ask from an active CCB task: use `--callback` for dependencies
  or `--silence` for independent work.

## Reading The Result

- For `--artifact-request`, verify request artifacts under
  `.ccb/ccbd/artifacts/text/ask-request/`; message `payload_ref` can remain
  null even when request delivery was artifact-backed.
- For `--artifact-reply`, verify `reply_artifact` in `replies.jsonl` or read
  the completion-reply artifact path returned by CCB.
- `--compact` is not a storage guarantee; use `--artifact-reply` when exact
  output must be preserved.
