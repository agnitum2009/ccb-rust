# Lifecycle And Tooling

Date: 2026-06-01

## Objective

Define how roles are installed, updated, diagnosed, repaired, and removed
without conflating reusable role assets with per-agent runtime state.

## Commands

List available roles:

```bash
ccb roles list
```

Show a role manifest:

```bash
ccb roles show agentroles.archi
```

Install role assets into the system role store and prepare declared external
tool dependencies:

```bash
ccb roles install agentroles.archi
```

Bind a role to a project agent:

```bash
ccb roles add agentroles.archi:codex
```

Diagnose a role and its bound agents:

```bash
ccb roles doctor agentroles.archi
ccb roles doctor --agent archi
```

Update installed role assets and declared external tool dependencies:

```bash
ccb roles update agentroles.archi
```

Sync edited local role source into the installed store:

```bash
ccb roles sync
ccb roles sync /path/to/role-or-role-library
```

Refresh projections for a bound agent remains planned:

```bash
ccb roles refresh archi
```

The first implementation includes `list`, `show`, `install`, `update`, `sync`,
`add`, and `doctor`. Role install/update handles declared dependencies by
default after the user approves the role/tool policy. `sync` defaults to the
current working directory, only considers roles discovered under that path, and
updates already installed same-id roles without changing project locks. Tool
update hooks run only when requested for sync. `repair` and `refresh` remain
planned commands.

The target boundary is that role payload lifecycle operations are implemented
by `agent-roles-spec` package-management tools, with CCB retaining these
commands as CCB-facing wrappers. `ccb roles add`, project locks, provider
projection, and CCB post-update failure policy remain CCB-owned. See
[spec-owned-roles-store.md](spec-owned-roles-store.md).

CCB update should also run a catalog-aware role update pass:

1. refresh or locate the `agent-roles-spec` catalog
2. compare catalog roles with the local installed-role store
3. update roles that are already installed locally when the catalog has a
   newer version or digest
4. show newly available catalog roles that are not installed locally
5. ask the user whether to install those new roles into the local CCB role
   store
6. report bound project locks that became stale, without silently changing
   project locks

## Lifecycle States

- `available`: discoverable but not installed.
- `installed`: present in the system role store.
- `new_available`: present in the refreshed `agent-roles-spec` catalog but not
  installed locally.
- `update_available`: installed locally, but the refreshed catalog has a newer
  version or digest.
- `locked`: referenced by a project lock.
- `bound`: assigned to one or more project agents.
- `projected`: assets rendered into provider homes.
- `degraded`: installed but doctor found missing optional or required pieces.
- `stale`: installed version differs from project lock or projected digest.
- `removed`: unbound from project; system assets may still remain installed.

## External Tools

Role tools should be installed under CCB-owned roots where possible:

```text
$XDG_DATA_HOME/ccb/tools/<tool-id>/
$XDG_CACHE_HOME/ccb/tools/<tool-id>/
```

For example, `agentroles.archi` installs Archi through the global npm package
`@seemseam/architec`. CCB must not split Hippo or llmgateway into separate
pip, venv, git, or editable installs because those capabilities are provided
by the Architec package.

Tool lifecycle hooks:

- `install`: prepare required binaries or wrappers without vendoring secrets.
- `doctor`: check readiness without mutating when possible.
- `update`: refresh tool dependencies.
- `repair`: optional, safe remediation for known broken states.

CCB runs Python role hook commands with bytecode generation disabled. CCB
adapter metadata should also spell Python hooks as `python -B ...` so the hook
remains cache-free when inspected or run by a host that does not inject
`PYTHONDONTWRITEBYTECODE`. Hook state belongs under CCB-owned tool roots, not
inside the installed role snapshot.

## Secrets

Role tools may require external configuration, but the Role Pack must not store
secrets. For Architec, doctor may check whether bundled llmgateway capability is
available, but remediation should point to `npm install -g @seemseam/architec`
or updating that package rather than separate llmgateway installation steps.

## Removal

Unbinding a role from a project should:

- remove project role references from config when requested
- remove role projections from bound provider homes
- keep provider sessions and auth untouched
- keep system role assets installed unless `ccb roles uninstall` is requested

System uninstall should refuse to remove an installed role while any project
lock still references it, unless forced with clear diagnostics.

## Catalog-Owned Source

CCB source code should not contain production role packages. The local system
role store is an installed cache; `agent-roles-spec` is the catalog source.

`ccb roles list` should show catalog roles and installed roles together, with
status labels such as `available`, `installed`, `update_available`, and
`installed_source_missing`.

`ccb roles install <role-id>` should resolve the role from
`agent-roles-spec` by default, unless the user passes an explicit path/source.

`ccb roles update <role-id>` should update from the recorded source path or
the refreshed `agent-roles-spec` catalog. It should not fall back to a CCB
source-tree `roles/` directory.

`ccb roles sync [path]` is for local editable role sources such as
`~/.ccb/roles/<role-id>` or `~/.roles/<role-id>`. The omitted path is `.`.
The command must not scan every known role source by default and must not
install missing roles implicitly; missing same-id installed roles are reported
as skipped.
