# Role Pack Core Spec

Date: 2026-06-01

## Objective

Define a portable Role Pack format that can be consumed by CCB first, but can
also be implemented by other hosts. The core spec must not assume tmux, CCB
runtime state, Codex, Claude, or any single provider. Host-specific behavior
belongs in adapters.

## Package Shape

```text
roles/
  <role-id-or-short-name>/
    role.toml
    README.md
    memory.md
    prompts/
    skills/
      codex/
      claude/
      generic/
    tools/
      install.py
      doctor.py
      update.py
    mcp/
      servers.toml
    adapters/
      ccb.toml
      codex.toml
      claude.toml
    tests/
```

Only `role.toml`, `README.md`, and at least one behavior asset are required.
For a purely advisory role, the behavior asset may be only `memory.md`. For a
tool-backed role, it should include skills and tool lifecycle scripts.

## Manifest Example

```toml
schema = "rolepack/v1"
id = "agentroles.archi"
name = "Architecture Reviewer"
version = "0.1.0"
description = "Architecture review role powered by Architec."
license = "MIT"

[identity]
default_agent_name = "archi"
category = "review"
purpose = "Analyze architecture, coupling, boundaries, and structural risk."
responsibilities = [
  "Review current diffs for architecture risk",
  "Run full-project architecture review when requested",
  "Explain structural tradeoffs and sequencing"
]
non_goals = [
  "Direct business feature implementation",
  "Release publishing"
]

[compatibility]
providers = ["codex", "claude"]
hosts = ["ccb"]
min_host_versions = { ccb = "7.1.0" }

[memory]
files = ["memory.md"]
merge_strategy = "append_after_project_memory"

[skills]
codex = ["skills/codex/archi-diff", "skills/codex/archi-full"]
claude = ["skills/claude/archi-diff", "skills/claude/archi-full"]

[tools.architec]
install = "python tools/install.py"
doctor = "python tools/doctor.py"
update = "python tools/update.py"
required = true

[permissions]
default = "manual"
network = "install_update_only"
filesystem = "project_read"
secrets = "external_config_only"

[activation]
ask_target = true
recommended_workspace_mode = "inplace"
```

## Required Manifest Fields

- `schema`: schema version, starting with `rolepack/v1`.
- `id`: stable role id. Catalog roles should use `<publisher>.<role>`.
- `name`: human-readable name.
- `version`: semver-compatible version.
- `description`: short user-facing explanation.
- `identity.purpose`: why this role exists.
- `identity.non_goals`: explicit exclusions to prevent role creep.
- `compatibility.providers`: providers that can receive the role projection.
- `compatibility.hosts`: hosts that have an adapter or declared support.

## Optional Manifest Fields

- `memory`: role memory sources and merge strategy.
- `skills`: provider-specific skill directories.
- `tools`: external tool lifecycle hooks.
- `mcp`: MCP servers or server templates.
- `permissions`: declared permission needs.
- `activation`: recommended agent name, workspace mode, ask routing, and
  window placement.
- `tests`: smoke tests and compatibility tests.
- `distribution`: source, digest, signature, and update policy.

## Validation Rules

- `id` must be stable and must not depend on the local agent name.
- Role ids must be normalized and collision-resistant within one host store.
- Provider-specific skill paths must not be projected into unsupported
  providers.
- Tool lifecycle commands must be declared, not hidden in memory text.
- Memory files must be text and must be safe to render into provider-native
  memory bundles.
- Role Packs must not contain credentials, sessions, runtime authority, or
  project-specific private state.
- Host-specific behavior must live under adapter fields or adapter files.

## Relationship To Skills And MCP

Skills are task capabilities. A role may include many skills.

MCP servers are tool protocols. A role may require or recommend MCP servers.

Role Packs bind identity, responsibilities, memory, skills, tools, and
permissions into one reusable agent archetype.
