# Role Pack System Open Questions

Date: 2026-06-01

## Open

1. Should the first schema be authored only as `role.toml`, or should a JSON
   Schema artifact be published in parallel for non-Python hosts?
2. What minimum trust gate is required before running a third-party role
   installer: explicit prompt, allowlist, digest pin, signature, or all of
   these?
3. Should role memory changes require agent restart, provider-native clear, or
   an explicit projection refresh command?
4. How should a host resolve conflicts when two roles want to install the same
   provider skill name?
5. Should role tool dependencies be allowed to install into user-level
   language package managers, or must the first CCB implementation always use
   CCB-owned venv/cache roots?
6. Should role ids use `publisher.role` only, or should the spec also reserve a
   URI-like form such as `rolepack://publisher/role`?
7. After an installed role is updated from `agent-roles-spec`, should CCB only
   mark bound project locks as stale, or should it offer an interactive
   project-lock update for the current project?
8. If project-locked role content is missing from the installed store, should
   CCB keep warning-only behavior and mount the agent with role memory/skills
   suppressed, or fail startup/reload for that role-bound agent?
9. When a required role tool hook fails during install, update, or sync, should
   CCB roll back `current` and metadata, or record an installed-but-degraded
   state that doctor and reload can surface?
10. What transaction model should protect role install/update/sync and
   project config plus role-lock writes from concurrent or partial failures?
11. Should GitHub catalog clone/pull failures be represented as explicit
   catalog diagnostics instead of silently returning no default catalog?
12. What exact store path should the spec-owned package manager use by default:
    `~/.roles`, an XDG data path, or a configurable path with `~/.roles` as a
    user-facing alias?
13. Should CCB call the spec-owned package manager through a subprocess JSON
    protocol, a Python library, or both?
14. How should existing `$XDG_DATA_HOME/ccb/roles/` installs migrate to the
    spec-owned store without invalidating existing project locks?

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
- Production role package content should live in `agent-roles-spec`, not in
  the CCB source tree. The first CCB slice consumes the catalog and owns local
  installation, projection, update prompts, and diagnostics. See
  [decisions/005-agent-roles-spec-is-catalog-authority.md](decisions/005-agent-roles-spec-is-catalog-authority.md).
- Long term, `agent-roles-spec` should own the `.roles` package store and role
  payload install/update semantics, while CCB owns project/runtime integration.
  See
  [decisions/006-agent-roles-spec-owns-roles-store.md](decisions/006-agent-roles-spec-owns-roles-store.md).
- CCB may manage a consumption-only GitHub cache of `agent-roles-spec` under
  `$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec` when no local catalog is
  available. User-level system role sources and local env/path catalogs still
  take precedence, and role content changes go to upstream `agent-roles-spec`
  by pull request. See
  [decisions/005-agent-roles-spec-is-catalog-authority.md](decisions/005-agent-roles-spec-is-catalog-authority.md)
  and [topics/distribution-and-trust.md](topics/distribution-and-trust.md).
- User-level system role sources at `~/.ccb/roles` and `~/.roles` are the
  first local editable role library. They are visible in `ccb roles list` and
  can be snapshotted into the installed store by `ccb roles add`; project-level
  `.roles` directories are deferred.
- During `ccb update`, CCB reports newly available catalog roles, prompts
  interactively before installing them into the local role store, and prints
  follow-up install commands in non-interactive runs. See
  [topics/catalog-update-flow.md](topics/catalog-update-flow.md).
