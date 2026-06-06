# Archi Role First Slice

Date: 2026-06-01

## Objective

Use `agentroles.archi` from `agent-roles-spec` as the first concrete CCB
consumed Role Pack. It validates the model because it needs fixed role
identity, role memory, provider skills, external tool installation,
diagnostics, and project-level binding.

## Role Identity

```toml
schema = "agent-role/preview-0.1"
id = "agentroles.archi"
name = "Architecture Reviewer"
version = "0.2.0"
description = "Architecture review role powered by Architec."

[identity]
default_name = "archi"
category = "review"
purpose = "Review architecture, boundaries, coupling, and structural risk."
non_goals = ["business implementation", "release publishing"]
```

## Assets

- `memory.md`: rules for architecture review, default no-code-edit posture,
  and expected output shape.
- Codex skills: `archi-diff`, `archi-full`, `archi-advice`, `archi-goal`.
- Claude skills: same semantic set in Claude skill format.
- Tools:
  - install or update Archi through the global npm package
    `@seemseam/architec`
  - do not split Hippo or llmgateway into CCB-managed pip, venv, git, or
    editable installs
  - doctor checks the main `archi` CLI and bundled `hippo`/`llmgateway`
    command availability without trying to install them separately

## CCB Binding

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

Shorthand binding:

```toml
[windows]
main = "agent1:codex, agentroles.archi:codex"
```

The visible target is `archi`. The stable role id is `agentroles.archi`. When
the shorthand form is used, CCB resolves `agentroles.archi` through the
installed system role store and derives `archi` from the role identity.

Sidebar must display `archi`, not `agentroles.archi`. The role id may appear in
role details or diagnostics, but not as the main agent row label.

## Doctor Expectations

`ccb roles doctor agentroles.archi` should report:

- installed role version and digest
- `archi` CLI readiness
- bundled `hippo` and `llmgateway` command availability
- remediation pointing to `npm install -g @seemseam/architec` when missing
- skill projection status for each bound provider
- memory projection hash
- actionable remediation for missing dependencies

## First Slice Boundaries

Implemented in the first code slice, with migration needed:

- legacy `ccb.archi` compatibility input alias, which resolves to
  `agentroles.archi`
- system role store install
- CCB config binding through `[agents.<name>] role = "agentroles.archi"`
- project role lock writing
- role memory inclusion in generated provider memory
- Codex and Claude role skill projection into managed provider homes
- `ccb roles list/show/install/update/sync/add/doctor`
- Archi tool lifecycle execution through global npm package
  `@seemseam/architec`
- `doctor` checks the `archi`, `hippo`, and `llmgateway` commands without
  printing secrets or attempting split installs
- shorthand config validation with sidebar display as `archi`
- real `test_ccb2` validation for install/update/sync/add, projection,
  startup, reload, and runtime doctor

Still in scope after the first production role PR:

- an explicit role projection refresh/adopt command for already-running agents
- a policy decision on whether missing locked content is warning-only or a hard
  mount error
- final release packaging/publishing by agent4

Out of scope:

- public role registry
- signed role packages
- automatic role discovery UI
- hot updating a running Archi provider session without explicit refresh or
  restart
