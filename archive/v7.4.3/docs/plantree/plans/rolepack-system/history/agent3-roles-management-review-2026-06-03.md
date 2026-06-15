# Agent3 Roles Management Review

Date: 2026-06-03

## Source

CCB review job:

```text
job_0512c1947c5f
artifact: .ccb/ccbd/artifacts/text/completion-reply/job_0512c1947c5f-art_0ae7f364c75e4fe7.txt
sha256: 57e1bee1fe4b4e4f0f6782a6b835c5b105604c754e46832a6f3250e6d4f80999
```

## Verdict

No blocking implementation flaw was found in the current roles management
scheme. Agent3 endorsed the core boundary that role catalogs and editable role
sources are discovery surfaces, while project locks remain runtime authority.
The review specifically agreed that roles should not auto-update on CCB
restart; update, sync, and project adoption should remain explicit operations.

## Primary Decision Gap

Missing locked content is still undecided. Current behavior is warning-only:
CCB emits `role_lock_mismatch`, suppresses role memory and skills, and still
allows the agent to mount. Agent3 flagged that this may create an apparently
healthy agent with degraded role behavior. The plan needs a decision on whether
missing locked content should remain warning-only or become a hard startup or
reload error for mounted agents.

## Risks To Track

- Runtime import boundaries need smoke tests so config loading, provider home
  projection, and provider hooks cannot accidentally import role management or
  network-capable source discovery paths.
- Tool hook failure can leave an installed role with moved `current` and
  updated metadata but failed required dependencies. Either rollback or
  explicit degraded install state is needed.
- Concurrent install/update/sync operations for the same role need file-lock or
  transaction protection.
- `roles list` should surface duplicate role-id warnings clearly enough that
  users notice local roles shadowing catalog roles.
- `roles sync` is correctly explicit and path-scoped, but should keep warning
  clearly when the path contains no roles or only uninstalled roles.
- GitHub cache fetch/pull failures should become visible catalog diagnostics
  instead of silently making catalog roles disappear.
- Project config writes and role-lock writes are not currently one transaction;
  failure after config mutation can leave config and lock inconsistent.
- Long digest-heavy errors such as `role_lock_mismatch` need more readable
  user-facing wording.
- Installed digest versions need a future garbage collection policy that keeps
  project-locked content safe.

## Suggested Priority

Short term:

1. Decide missing locked content policy.
2. Add import-boundary smoke tests.
3. Add role operation failure-recovery and concurrency tests.
4. Improve stale lock diagnostics and duplicate warning visibility.

Medium term:

1. Add project role check/adopt or refresh command.
2. Harden role install/update/sync transaction behavior.
3. Add visible GitHub catalog fetch-failure diagnostics.
4. Add installed role garbage collection.
