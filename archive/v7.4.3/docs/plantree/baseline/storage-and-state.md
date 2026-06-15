# Storage And State

Date: 2026-05-25

## User-Authored Project Files

- `.ccb/ccb.config`: highest-priority project config authority when present.
- `.ccb/ccb_memory.md`: shared project memory.
- `.ccb/agents/<agent>/memory.md`: optional per-agent memory.

These are user-facing concepts and should be explained in README only at the
level needed for first setup and team customization.

## Runtime Evidence

- `.ccb/ccbd/`: lifecycle, lease, namespace, diagnostics, and backend records.
- `.ccb/agents/<agent>/runtime.json`: configured-agent runtime records.
- Provider session files and tmux facts are evidence, not public configuration
  authority.

The README should avoid asking users to edit runtime records manually.

## Public Assets

- Current media is under `assets/`.
- A README v7 refresh should use a dedicated subfolder such as
  `assets/readme_v7/` if new screenshots and animations are created.
- Large generated raw recordings should not be committed unless explicitly
  needed; only optimized public artifacts should be referenced from README.

