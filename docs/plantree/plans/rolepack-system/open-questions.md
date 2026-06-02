# Role Pack System Open Questions

Date: 2026-06-01

## Open

1. Should the first schema be authored only as `role.toml`, or should a JSON
   Schema artifact be published in parallel for non-Python hosts?
2. Should community roles be vendored into the CCB repository first, or should
   CCB support direct `github:owner/repo//path` installation in the first
   slice?
3. What minimum trust gate is required before running a third-party role
   installer: explicit prompt, allowlist, digest pin, signature, or all of
   these?
4. Should role memory changes require agent restart, provider-native clear, or
   an explicit projection refresh command?
5. How should a host resolve conflicts when two roles want to install the same
   provider skill name?
6. Should role tool dependencies be allowed to install into user-level
   language package managers, or must the first CCB implementation always use
   CCB-owned venv/cache roots?
7. Should role ids use `publisher.role` only, or should the spec also reserve a
   URI-like form such as `rolepack://publisher/role`?

## Resolved

- Fixed role identity and user-facing agent names are separate. See
  [decisions/001-role-id-separate-from-agent-name.md](decisions/001-role-id-separate-from-agent-name.md).
- Role assets are installed once and projected into agents. See
  [decisions/002-system-role-store-project-locks.md](decisions/002-system-role-store-project-locks.md).
- Role Packs should be host-neutral with adapters. See
  [decisions/003-rolepacks-are-host-neutral-with-adapters.md](decisions/003-rolepacks-are-host-neutral-with-adapters.md).
- CCB role-id shorthand resolves to a project-local agent name, and sidebar
  rows use that local name. See
  [decisions/004-role-id-shorthand-resolves-to-agent-name.md](decisions/004-role-id-shorthand-resolves-to-agent-name.md).
