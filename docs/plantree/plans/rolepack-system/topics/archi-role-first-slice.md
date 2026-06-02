# Archi Role First Slice

Date: 2026-06-01

## Objective

Use `ccb.archi` as the first concrete Role Pack. It validates the model because
it needs fixed role identity, role memory, provider skills, external tool
installation, diagnostics, and project-level binding.

## Role Identity

```toml
schema = "rolepack/v1"
id = "ccb.archi"
name = "Architecture Reviewer"
version = "0.1.0"
description = "Architecture review role powered by Architec."

[identity]
default_agent_name = "archi"
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
  - install Architec in a CCB-owned venv where possible
  - expose a stable wrapper such as `ccb-archi`
  - doctor checks `archi`, `hippocampus`, `llmgateway`, and project readiness
  - update refreshes the installed Architec package

## CCB Binding

Explicit binding:

```toml
[windows]
main = "agent1:codex, archi:codex"

[agents.archi]
role = "ccb.archi"
provider = "codex"
workspace_mode = "inplace"
permission = "manual"
```

Shorthand binding:

```toml
[windows]
main = "agent1:codex, ccb.archi:codex"
```

The visible target is `archi`. The stable role id is `ccb.archi`. When the
shorthand form is used, CCB resolves `ccb.archi` through the installed system
role store and derives `archi` from `identity.default_agent_name`.

Sidebar must display `archi`, not `ccb.archi`. `ccb.archi` may appear in role
details or diagnostics, but not as the main agent row label.

## Doctor Expectations

`ccb roles doctor ccb.archi` should report:

- installed role version and digest
- Architec wrapper path
- Architec import or CLI readiness
- `llmgateway` config presence without secret output
- skill projection status for each bound provider
- memory projection hash
- actionable remediation for missing dependencies

## First Slice Boundaries

Implemented in the first code slice:

- built-in `roles/ccb.archi` manifest and source-tree assets
- system role store install
- CCB config binding through `[agents.<name>] role = "ccb.archi"`
- project role lock writing
- role memory inclusion in generated provider memory
- Codex and Claude role skill projection into managed provider homes
- `ccb roles list/show/install/add/doctor`

Still in scope for the complete first role:

- Architec tool lifecycle execution in a CCB-owned venv
- richer `doctor` checks for `archi`, `hippocampus`, and `llmgateway`
- shorthand config validation with sidebar display as `archi`
- real `test_ccb2` validation with `ccb reload`, `ccb ask archi`, and the
  `ccb ask ccb.archi` alias

Out of scope:

- public role registry
- signed role packages
- automatic role discovery UI
- hot updating a running Archi provider session without explicit refresh or
  restart
