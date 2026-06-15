# Agent Roles Spec Is Catalog Authority

Date: 2026-06-03

Status: Partially superseded by
[006-agent-roles-spec-owns-roles-store.md](006-agent-roles-spec-owns-roles-store.md).
This decision remains authority that production role content and catalog
governance live in `agent-roles-spec`, not `ccb_source`. Decision 006 changes
the long-term owner of the local `.roles` package store and role payload
install/update semantics from CCB to `agent-roles-spec`.

## Context

The first CCB Role Pack slice placed a role package under the CCB source tree
as `roles/ccb.archi`. That was useful for proving manifest parsing, install,
project binding, role memory, skill projection, and tool hooks, but it makes
CCB itself the role library.

That is the wrong long-term boundary. `agent-roles-spec` is the RolePack
specification and role-library project. CCB should be one host that consumes
that library through a CCB adapter. CCB should not vendor role package content
inside its own source tree.

## Decision

`agent-roles-spec` is the catalog authority for CCB role packages.

CCB owns:

- role catalog discovery and refresh
- install/update commands
- the local installed-role store
- project role locks
- CCB config binding
- provider-home projection
- ask alias, sidebar, diagnostics, reload, and refresh behavior

`agent-roles-spec` owns:

- role package source content
- role ids, versions, README files, memory, skills, prompts, tools, plugins,
  and host adapter metadata
- role contribution governance
- spec, schema, templates, reference roles, and production-ready role catalog

The CCB source tree must not contain installable role package directories such
as `roles/<role-id>`. Existing CCB source-tree roles are temporary migration
artifacts and should be removed after CCB can install from
`agent-roles-spec`.

## Update Semantics

During `ccb update`, CCB should refresh the `agent-roles-spec` catalog before
role update decisions.

If an installed role has a newer version or digest in `agent-roles-spec`, CCB
should update that installed role as part of the CCB update flow, subject to
the same trust and tool-policy rules as `ccb roles update`.

If `agent-roles-spec` contains roles that are not installed locally, CCB should
show the newly available role ids, versions, and short descriptions, then ask
whether to install them into the local CCB role store.

Project locks must not silently float just because the installed store is
updated. Updating the user-level installed role store may make a project lock
stale; adopting the new role version in a project remains an explicit project
operation such as `ccb roles refresh --apply-lock` or a future project role
update command.

## Catalog Location

The first CCB implementation may resolve the catalog from:

1. user-level system role libraries at `~/.ccb/roles` and `~/.roles`
2. `CCB_AGENT_ROLES_SPEC_HOME` or `AGENT_ROLES_SPEC_HOME`
3. a default local clone such as `~/yunwei/agent-roles-spec`
4. a CCB-owned GitHub cache cloned from
   `https://github.com/SeemSeam/agent-roles-spec` under
   `$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec`

Additional configured local sources in the CCB role source registry are loaded
after the default sources and must not silently shadow earlier role ids.

The catalog resolver should report the chosen path and whether it is current,
missing, stale, or unreadable. It should not silently fall back to CCB
source-tree roles.

The CCB-owned GitHub cache is a read-only consumption cache from CCB's point of
view. Users who want to modify production role content should submit changes to
the upstream `agent-roles-spec` GitHub repository by pull request, then let CCB
refresh or reinstall from the accepted catalog content.

User-level system role libraries are local editable sources. When such a role
is added to a project, CCB snapshots it into the installed role store and locks
the project to that digest; project-level `.roles` directories are deferred for
the first implementation slice.

## Consequences

- CCB releases do not carry role package content.
- New roles can appear in `agent-roles-spec` without changing CCB source.
- CCB update can make installed roles current and can prompt users to install
  newly published roles.
- Role package governance moves to `agent-roles-spec`.
- CCB tests need fixtures for role packages, but those fixtures must be test
  fixtures or generated temporary directories, not the production role catalog.
- Built-in role fallback should be replaced by catalog lookup and installed
  role store lookup.
