# CCB Host Adapter

Date: 2026-06-01

## Objective

Define how CCB consumes host-neutral Role Packs without turning them into CCB
only assets. CCB should provide config binding, role installation, provider
home projection, reload integration, and diagnostics.

This topic describes the CCB host adapter and the current CCB-first
implementation. The long-term package-store boundary is amended by
[spec-owned-roles-store.md](spec-owned-roles-store.md): CCB keeps project and
runtime integration, while role payload package management moves to the
`agent-roles-spec` tool/API.

## Config Shape

Explicit binding:

```toml
[windows]
main = "agent1:codex, archi:codex"

[agents.archi]
role = "agentroles.archi"
provider = "codex"
workspace_mode = "inplace"
permission = "manual"
```

`role` is metadata for projection and behavior. The project-local agent name is
still `archi`; the stable role id is the catalog role id, for example
`agentroles.archi`.

Shorthand binding:

```toml
[windows]
main = "agent1:codex, agentroles.archi:codex"
```

When a window leaf uses a publisher-qualified role id such as
`agentroles.archi`, CCB checks the installed system role store during config
load. If the role is installed, the leaf resolves to the role manifest's
default agent name, for example `archi`, and CCB adds the equivalent role
binding. If the role is not installed, config loading fails with guidance to
run `ccb roles install agentroles.archi`.

Expansion runs after TOML and window leaf parsing, before agent defaults,
overlay merge, and final topology validation. The expanded form must flow
through the same validation path as an explicit `archi:codex` leaf with
`[agents.archi] role = "agentroles.archi"`.

Runtime surfaces use the project-local agent name. The sidebar row, mailbox
owner, job target, pane label, and primary ask target are `archi`, not
`agentroles.archi`. The role id may be shown only as secondary diagnostic
metadata.

## CLI Surface

```bash
ccb roles list
ccb roles show agentroles.archi
ccb roles install agentroles.archi
ccb roles add agentroles.archi:codex
ccb roles doctor agentroles.archi
```

In the current CCB-first slice, `install` resolves role content from
`agent-roles-spec` by default, mutates the local system role store, and
prepares declared dependencies. In the target split, CCB delegates role payload
install/update/doctor work to the spec-owned package manager, then applies
CCB-specific policy and diagnostics. `add` mutates project config and lock.
`doctor` reports catalog, installed, lock, projection, and tool state.
`refresh` is a planned follow-up.

`ccb update` should refresh the `agent-roles-spec` catalog, update already
installed roles when newer catalog content exists, and prompt before installing
newly available roles that are not yet in the local CCB role store.

`ccb ask agentroles.archi ...` is a convenience alias. It resolves to the
single configured agent bound to `agentroles.archi`; if there is no match or
more than one match, CCB fails and asks the user to target the project-local
agent name.

Alias errors must distinguish:

- no configured binding: tell the user the role is not bound and to add the
  role or target an existing agent name
- multiple configured bindings: list the matching agent names and require an
  explicit agent target

## Memory Layers

CCB should render provider memory in this order:

1. provider-native user memory, when inherited by provider profile
2. CCB runtime coordination rules
3. project shared memory
4. role memory
5. project role override memory
6. agent private memory

The generated provider memory file remains under the agent provider home. The
role memory source remains in the system role store.

## Skill Projection

For Codex:

```text
provider-state/codex/home/skills/<skill>
```

For Claude:

```text
provider-state/claude/home/.claude/skills/<skill>
```

Only the role-bound agent receives role skills. Role skills must not be added
to global `inherit_skills` for every agent.

## Reload Semantics

Role changes should be classified separately from topology changes:

- `role_only_change`: role id or role version changed for an existing agent.
- `role_projection_change`: memory, skill, or prompt projection changed.
- `role_tool_change`: tool dependency version or doctor state changed.

First implementation may require explicit `ccb roles refresh <agent>` or agent
restart after role projection changes. Hot projection into an actively running
provider must be treated as a provider-specific feature, not assumed.

`ccb reload` handles topology and mounted-agent reconciliation. `ccb roles
refresh` handles rebuildable role projections for an already configured agent.
A role memory, skill, prompt, or tool-state change must not be treated as a
topology replacement unless the configured agent set or provider leaf changed.

## Diagnostics

`ccb roles doctor <role>` should report:

- role installed/missing
- role version and digest
- project lock state
- provider compatibility
- projected skill status for each bound agent
- memory projection hash
- external tool doctor result
- secrets/config presence without printing secret values
- stale or orphaned projected assets

`ccb doctor` should include a compact role summary for configured agents.

## Failure Boundaries

- Missing optional role assets should degrade diagnostics, not break unrelated
  agents.
- Missing required tools should block role activation but not daemon startup
  for unrelated agents.
- Projection failure should be reported per agent and should not delete
  non-role provider files.
- A stale role lock should not silently float to a new version.
- Missing installed roles, unreadable role store state, invalid role manifests,
  and provider-incompatible roles are separate errors. Only the missing-role
  case should suggest `ccb roles install <role-id>` as the primary fix.
- Explicit role bindings and shorthand-derived bindings must use the same
  role-store lookup and compatibility checks.
