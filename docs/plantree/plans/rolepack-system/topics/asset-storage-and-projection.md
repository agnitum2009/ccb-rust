# Asset Storage And Projection

Date: 2026-06-01

## Objective

Avoid copying role assets into every project while preserving provider session
and auth isolation. Role assets should be installed once, referenced by
projects, and projected into agent provider homes only as generated or
rebuildable assets.

## Storage Layers

System role store:

```text
$XDG_DATA_HOME/ccb/roles/
  ccb.archi/
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
    ccb.archi/
      memory.override.md
      config.toml
```

Agent runtime projection:

```text
project/.ccb/agents/archi/provider-state/codex/home/
  skills/archi-diff -> $XDG_DATA_HOME/ccb/roles/ccb.archi/current/skills/codex/archi-diff
  skills/archi-full -> $XDG_DATA_HOME/ccb/roles/ccb.archi/current/skills/codex/archi-full
  AGENTS.md
  sessions/
```

The provider home may contain symlinks or copied projected assets, but the
authority remains the system role store plus project lock.

## Shareable Assets

These may live in the system role store or a content-addressed shared store:

- `role.toml`
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
- Project lock changes should be visible in diagnostics before runtime restart
  or reload.

## Version And Locking

The project should keep a role lock file:

```json
{
  "schema": "rolepack-lock/v1",
  "roles": {
    "ccb.archi": {
      "version": "0.1.0",
      "digest": "sha256:...",
      "source": "builtin"
    }
  }
}
```

The config can reference a role without duplicating the lock:

```toml
[agents.archi]
role = "ccb.archi"
provider = "codex"
```

The lock records the resolved package. The config records the desired binding.

