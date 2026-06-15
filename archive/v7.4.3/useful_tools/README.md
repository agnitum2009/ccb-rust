# Useful Tools

This directory contains optional tools that are useful for CCB users but are not
installed by default.

The default installer intentionally keeps provider skills minimal. Copy tools
from this directory into the provider source home where you want them inherited.

## Skill Sets

Provider-specific skill sets live under:

- `useful_tools/codex_skills/`
- `useful_tools/claude_skills/`

To enable a skill for Codex-managed agents that inherit your global Codex
skills:

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}/skills"
cp -R useful_tools/codex_skills/plan-tree "${CODEX_HOME:-$HOME/.codex}/skills/"
```

Then restart or relaunch the managed CCB agent so its isolated Codex home can
inherit the skill. This affects same-provider agents whose
`provider_profile.inherit_skills` is true.

To enable a skill for Claude-managed agents that inherit your global Claude
skills:

```bash
mkdir -p "$HOME/.claude/skills"
cp -R useful_tools/claude_skills/plan-tree "$HOME/.claude/skills/"
```

Then restart or relaunch the managed CCB agent so its isolated Claude home can
inherit the skill. This affects same-provider agents whose
`provider_profile.inherit_skills` is true.

## Agent-Specific Requests

`.ccb/ccb.config` currently has `provider_profile.inherit_skills`, which turns
provider-home skill inheritance on or off as a whole. It does not have a
per-skill allowlist.

For requests such as "inject plan-tree into agent2", use the `ccb-config` skill
to inspect the target agent provider and choose a safe installation scope:

- durable provider-home install, inherited by same-provider agents;
- config change to enable/disable `provider_profile.inherit_skills`;
- temporary runtime copy for one already-mounted agent, only when explicitly
  requested and with the understanding that restart/projection refresh may
  replace it.

Temporary Codex runtime copy:

```bash
mkdir -p .ccb/agents/<agent>/provider-state/codex/home/skills
cp -R useful_tools/codex_skills/plan-tree .ccb/agents/<agent>/provider-state/codex/home/skills/
```

Temporary Claude runtime copy:

```bash
mkdir -p .ccb/agents/<agent>/provider-state/claude/home/.claude/skills
cp -R useful_tools/claude_skills/plan-tree .ccb/agents/<agent>/provider-state/claude/home/.claude/skills/
```

Replace `<agent>` with the configured CCB agent name. Before using temporary
runtime copy, verify the destination `skills` directory is not a symlink.

## Included Tools

- `plan-tree`: Maintain linked planning document trees containing roadmaps,
  topic notes, decision records, and open questions.
