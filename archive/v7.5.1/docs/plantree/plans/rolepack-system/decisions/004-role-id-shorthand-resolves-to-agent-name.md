# Role Id Shorthand Resolves To Agent Name

Date: 2026-06-01

## Context

Users should be able to configure catalog roles with less boilerplate.
For example, writing `agentroles.archi:codex` in a CCB window layout is
clearer than manually adding both an `archi:codex` leaf and an
`[agents.archi] role = "agentroles.archi"` overlay.

At the same time, CCB runtime surfaces such as sidebar rows, mailbox owners,
job targets, and provider runtime records need ergonomic project-local agent
names. A role id is package identity, not the mounted agent instance name.

## Decision

CCB may accept a role-id shorthand in config leaves. A leaf whose name is a
publisher-qualified role id such as `agentroles.archi` is resolved during
config load:

1. The role id must exist in the installed system role store.
2. Missing installed roles are config errors with guidance to run
   `ccb roles install <role-id>`.
3. The configured agent name is derived from the role manifest's
   `identity.default_agent_name`, for example `archi`.
4. The resolved agent receives `role = "<role-id>"` and the leaf provider.
5. If the derived agent name conflicts with another configured agent, CCB
   fails closed and asks for an explicit `[agents.<name>] role = "<role-id>"`
   binding.

The shorthand expansion happens after TOML parsing and window-leaf parsing, but
before agent defaults are merged, overlay tables are applied, and topology
validation builds `ProjectConfig`. That makes shorthand behave like an authored
`archi:provider` leaf plus an authored role overlay, and avoids a second
validation pass over partially expanded config.

Explicit `[agents.<name>] role = "<role-id>"` and shorthand-derived role
bindings share the same role lookup, provider compatibility, role-store error,
and conflict checks. The shorthand path must not accept a role id that the
explicit path would reject.

Example shorthand:

```toml
[windows]
main = "agent1:codex, agentroles.archi:codex"
```

Resolved runtime meaning:

```toml
[windows]
main = "agent1:codex, archi:codex"

[agents.archi]
role = "agentroles.archi"
provider = "codex"
```

The project-local agent name remains `archi`. Sidebar, mailbox, job, and pane
labels use `archi`, not `agentroles.archi`. The role id may appear only as
secondary metadata in diagnostics or details.

`ccb ask <role-id> ...` is a convenience alias, not a runtime target name. It
resolves to the single configured agent bound to that role id. If no configured
agent or more than one configured agent uses the role id, the command fails and
asks the user to target the project-local agent name directly.

Alias errors use one format:

- no match: `role agentroles.archi is not bound to any configured agent; target
  the project-local agent name or add the role to config`
- multiple matches: `role agentroles.archi is bound to multiple agents: archi,
  archi-review; target one agent name explicitly`

## Consequences

- Config can be short for common fixed roles while preserving clean runtime
  agent names.
- Sidebar rows stay ergonomic and match actual ask targets such as `archi`.
- Role ids remain stable package identities and are not admitted as general
  agent names.
- Multiple instances of one role remain possible through explicit agent names.
- Config loading and ask routing need role-id alias resolution with clear
  ambiguity errors.
- Role-store lookup failures need distinct user-facing categories: not
  installed, unreadable store, invalid manifest, and provider-incompatible role.
