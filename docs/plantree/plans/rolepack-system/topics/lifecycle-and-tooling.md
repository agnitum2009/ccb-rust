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
ccb roles show ccb.archi
```

Install role assets into the system role store and prepare declared external
tool dependencies:

```bash
ccb roles install ccb.archi
```

Bind a role to a project agent:

```bash
ccb roles add ccb.archi:codex
```

Diagnose a role and its bound agents:

```bash
ccb roles doctor ccb.archi
ccb roles doctor --agent archi
```

Update installed role assets and declared external tool dependencies:

```bash
ccb roles update ccb.archi
```

Refresh projections for a bound agent remains planned:

```bash
ccb roles refresh archi
```

The first implementation includes `list`, `show`, `install`, `update`, `add`,
and `doctor`. Role install/update handles declared dependencies by default,
after the user confirms bundled Role Pack provisioning during CCB install or
update. `repair` and `refresh` remain planned commands.

## Lifecycle States

- `available`: discoverable but not installed.
- `installed`: present in the system role store.
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

For example, `ccb.archi` should prefer a CCB-owned venv and a wrapper such as
`ccb-archi` instead of requiring a global `pip install --user`.

Tool lifecycle hooks:

- `install`: prepare required binaries, venvs, or wrappers.
- `doctor`: check readiness without mutating when possible.
- `update`: refresh tool dependencies.
- `repair`: optional, safe remediation for known broken states.

## Secrets

Role tools may require external configuration, but the Role Pack must not store
secrets. For Architec, `llmgateway` configuration should remain in the
appropriate external config location and doctor should report missing config
without printing secrets.

## Removal

Unbinding a role from a project should:

- remove project role references from config when requested
- remove role projections from bound provider homes
- keep provider sessions and auth untouched
- keep system role assets installed unless `ccb roles uninstall` is requested

System uninstall should refuse to remove an installed role while any project
lock still references it, unless forced with clear diagnostics.
