# Asset Storage And Projection

Date: 2026-06-01

## Objective

Avoid copying role assets into every project while preserving provider session
and auth isolation. Role assets should be installed once, referenced by
projects, and projected into agent provider homes only as generated or
rebuildable assets.

Target ownership is amended by
[decisions/006-agent-roles-spec-owns-roles-store.md](../decisions/006-agent-roles-spec-owns-roles-store.md):
the package store should become `agent-roles-spec` owned, while CCB continues
to own project locks and provider projection. The paths below describe the
current CCB-first implementation shape and the runtime boundaries that must
survive the migration.

## Storage Layers

Catalog source:

```text
agent-roles-spec/
  roles/
  reference_roles/
```

System role store installed cache:

```text
$XDG_DATA_HOME/ccb/roles/
  agentroles.archi/
    current -> versions/0.1.0
    versions/
      0.1.0/
        role.toml
        memory.md
        skills/
        tools/
        prompts/
```

Project references:

```text
project/.ccb/
  ccb.config
  role-lock.json
  roles/
    agentroles.archi/
      memory.override.md
      config.toml
```

Agent runtime projection:

```text
project/.ccb/agents/archi/provider-state/codex/home/
  skills/archi-diff -> $XDG_DATA_HOME/ccb/roles/agentroles.archi/current/skills/codex/archi-diff
  skills/archi-full -> $XDG_DATA_HOME/ccb/roles/agentroles.archi/current/skills/codex/archi-full
  AGENTS.md
  sessions/
```

The provider home may contain symlinks or copied projected assets. The role
catalog authority is `agent-roles-spec`; the local installed cache and project
lock define what a user and project have adopted.

## Shareable Assets

These may live in the system role store or a content-addressed shared store:

- `role.toml` from `agent-roles-spec`
- role `README.md`
- role memory templates
- provider-specific skills
- prompts and templates
- tool lifecycle scripts
- CCB-owned tool wrappers and venvs
- documentation and test fixtures

## Project-Scoped Assets

These belong in `.ccb`:

- `.ccb/ccb.config` role references
- `.ccb/role-lock.json` exact role version and digest pins
- project role overrides
- agent private memory
- runtime evidence and diagnostics

## Agent-Private State

These must not be shared through role assets:

- provider sessions and conversation history
- auth secrets or keychain-derived state
- provider trust authority
- runtime pid, pane, socket, and lifecycle records
- mailbox, ask, reply, and completion authority
- agent-specific workspace bindings

## Projection Rules

- Prefer symlinks from managed provider homes to immutable role store assets.
- Fall back to copy when symlinks are unavailable or unsafe.
- Every copied projection must have a projection marker with source, digest,
  label, and update time.
- Projection refresh must be explicit and diagnosable.
- Removing a role from an agent must remove only projected assets owned by that
  role; it must not delete user-authored provider files.
- Catalog and installed-cache changes should be visible in diagnostics before
  project locks, runtime restart, or reload change project behavior.

## Version And Locking

The project should keep a role lock file:

```json
{
  "schema": "rolepack-lock/v1",
  "roles": {
    "agentroles.archi": {
      "version": "0.1.0",
      "digest": "sha256:...",
      "source": "agent-roles-spec"
    }
  }
}
```

The config can reference a role without duplicating the lock:

```toml
[agents.archi]
role = "agentroles.archi"
provider = "codex"
```

The lock records the resolved package adopted by the project. The config
records the desired binding. Updating the catalog or installed cache does not
silently move the project lock.
