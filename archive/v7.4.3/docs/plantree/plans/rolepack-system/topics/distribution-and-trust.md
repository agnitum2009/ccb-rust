# Distribution And Trust

Date: 2026-06-01

## Objective

Support community roles without turning role installation into an unsafe script
runner. Distribution must be explicit, inspectable, lockable, and reversible.

## Distribution Sources

Agent Roles catalog:

```text
source = "agent-roles-spec:/home/user/agent-roles-spec//roles/<role>"
source = "agent-roles-spec:/home/user/agent-roles-spec//reference_roles/<role>"
```

User-level system roles:

```text
source = "systemroles:/home/user/.ccb/roles/<role>"
source = "dotroles:/home/user/.roles/<role>"
```

Local path:

```text
source = "path:/home/user/roles/archi"
```

GitHub path:

```text
source = "github:SeemSeam/architec//roles/archi?ref=v0.1.0"
```

Future registry:

```text
source = "rolepack:seemseam.archi@0.1.0"
```

CCB source-tree role directories are not a production distribution source.
They may exist temporarily as migration fixtures, but role package content
belongs in `agent-roles-spec`.

## Agent Roles Catalog

`agent-roles-spec` is the first remote catalog authority. CCB should discover
user-level system role sources first, then environment/default local
`agent-roles-spec` paths, then a CCB-owned GitHub cache at
`$XDG_CACHE_HOME/ccb/role-catalogs/agent-roles-spec` cloned from
`https://github.com/SeemSeam/agent-roles-spec`. It then uses the discovered
roles to list available roles, install selected roles into the local CCB role
store, and update roles that are already installed.

User-level system role sources are editable local libraries, not project-local
runtime authority. A role added from `~/.ccb/roles` or `~/.roles` is first
snapshotted into the installed role store and locked by digest before it is
bound to a project. Project-level `.roles` directories are deferred for the
first slice.

The CCB-owned cache is not a role authoring workspace. CCB may create it with
`git clone` and refresh it with `git pull --ff-only`; role content changes must
be proposed to the upstream `agent-roles-spec` repository through a pull
request. Users who want local experimental roles should register an explicit
local source instead of editing the managed cache.

The long-term target moves catalog cache and `.roles` package-store ownership
into `agent-roles-spec` itself. In that model, CCB keeps the same user-facing
role commands but delegates role package sync/install/update/doctor operations
to the spec-owned package manager, then applies CCB project locks and provider
projection. See
[spec-owned-roles-store.md](spec-owned-roles-store.md).

Catalog status should distinguish:

- missing catalog
- unreadable catalog
- stale local clone/cache
- schema mismatch
- available roles that are not installed
- installed roles with newer catalog versions
- installed roles whose original catalog entry disappeared

During `ccb update`, CCB should refresh the catalog first. It should update
already installed roles that have newer catalog versions or digests, then show
new roles that are available in the catalog but not installed locally and ask
whether to install them.

## Trust Stages

1. Inspect: show manifest, permissions, tools, network needs, and files.
2. Resolve: download or locate role assets into a staging directory.
3. Verify: compute digest and validate manifest schema.
4. Trust: user approves installation of this digest/source.
5. Install: write into the system role store.
6. Lock: project records exact role version and digest when bound.

## Installer Rules

- Role install may fetch role assets.
- Tool install may fetch external dependencies only when the role declares the
  dependency and the user approves or passes an explicit non-interactive flag.
- Role install must not read or write provider sessions.
- Role install must not write secrets into project config.
- Role updates must not change project locks without an explicit project update
  or `ccb roles update --apply-lock`.

## Digest And Provenance

Every installed role should have a metadata record:

```json
{
  "schema": "rolepack-install/v1",
  "id": "agentroles.archi",
  "version": "0.1.0",
  "source": "agent-roles-spec",
  "source_path": "/home/user/agent-roles-spec/reference_roles/archi",
  "digest": "sha256:...",
  "installed_at": "2026-06-01T00:00:00Z"
}
```

The digest is the authority for immutable installed content. A `current`
symlink or pointer may move only after install succeeds.

## Community PR Governance

Community roles submitted to `agent-roles-spec` should include:

- manifest validation
- README and examples
- provider skill tests where possible
- declared permissions and external dependencies
- doctor command or explicit "no external tools" declaration
- no credentials, sessions, or binary blobs unless specifically justified
- versioned changelog for breaking role behavior changes

CCB PRs should not add production role package content. They should add or
change CCB adapter code, catalog consumption, install/update behavior,
projection, diagnostics, or tests.

## Deferred Security Work

- Signed role manifests.
- Public transparency log.
- Remote registry ownership verification.
- Sandboxed third-party install scripts.
- Automated malware scanning.
