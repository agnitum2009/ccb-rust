# Reviewer1 Alignment: Provider Memory Ownership

Date: 2026-06-07

## Summary

Reviewer1 agreed that the source ownership manifest is the right architecture
and that implementation should proceed after readiness audits and contract
alignment. The review required policy/filter unit tests to land with the
policy/filter code rather than being deferred to a final validation-only phase.

Artifact:

- `.ccb/ccbd/artifacts/text/completion-reply/job_823610dbfaaa-art_5c2147efebae4197.txt`

## Decisions Applied

- OpenCode project `AGENTS.md` needed a native-loading audit before changing
  provider wiring. The first implementation pass audited OpenCode 1.16.2 and
  found native `AGENTS.md` discovery plus configured `instructions` loading.
- Codex source-home `AGENTS.md` should be treated as provider user memory and
  receive conservative CCB install-block filtering.
- Claude route-mode `~/.claude/rules/ccb-config.md` should no longer be written
  by new installs; uninstall cleanup should continue removing known CCB-owned
  copies.
- Policy/filter tests must land together with the policy/filter implementation.

