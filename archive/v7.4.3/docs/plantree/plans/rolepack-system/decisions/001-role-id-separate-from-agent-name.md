# Role Id Separate From Agent Name

Date: 2026-06-01

## Context

Users need fixed, recognizable roles such as an architecture reviewer, but
project-local agent names must remain ergonomic and conflict-free. A visible
agent name such as `archi` is pleasant for `ccb ask archi ...`, while a stable
package id such as `agentroles.archi` is better for versioning, locks, and
catalog distribution.

## Decision

Role identity and agent instance names are separate.

Catalog roles use ids such as `agentroles.archi`. Other publishers use
`<publisher>.<role>`. Project-local agent names may use the short role name by
default, but users may rename the agent when a project already has a
conflicting name.

Example:

```toml
[agents.archi]
role = "agentroles.archi"
provider = "codex"
```

## Consequences

- Role packages can be versioned and locked independently from project agent
  names.
- Users can keep natural targets such as `archi`.
- Adapter-level shortcuts may resolve a role id to a project-local agent name,
  but they must not make the role id the runtime agent name. See
  [004-role-id-shorthand-resolves-to-agent-name.md](004-role-id-shorthand-resolves-to-agent-name.md).
- Multiple instances of one role can exist in a future project by using
  different agent names.
- CCB must validate role ids separately from agent names.
