# CCB Self Role Open Questions

Date: 2026-06-09

## Product

Resolved: `ccb_self` is included in the built-in blank-project default and
existing project/user configs still add it explicitly when needed.

1. Should the default provider be Codex only for the first slice, or should the
   role declare Claude/OpenCode-compatible variants at launch?
2. Should `ccb ask agentroles.ccb_self ...` resolve as a role alias when the
   role is bound once, or should user docs always target `ccb_self` directly?
3. Should the public display name remain "CCB Self Maintainer", or change to a
   broader expert name such as "CCB Runtime And Architecture Expert"?
4. Which CCB expert surfaces should be mandatory in the default Role Pack:
   architecture navigation, command usage, release/update awareness,
   pane-view self-supervision, or all four?

## Permission Model

1. Should the role ever use `ccb kill`, or should project-wide shutdown remain
   user-only?

## Implementation

1. Should the first diagnostic helper be a `ccb_self` built-in script, an MCP
   server, or both, given that the role package can include scripts before MCP
   is stable?
2. What exact JSON schema should `ccb_runtime_snapshot` return so it remains
   useful across future daemon graph versions?
3. Where should read-only tmux namespace evidence be sourced from when WSL
   mounted-drive compatibility redirects sockets and runtime files?
4. What exact `tmux capture-pane` contract should `ccb_self` use for
   self-supervision: bottom/current screen only, configurable scrollback depth,
   or both?
5. Should screenshot capture be opt-in fallback only, or enabled automatically
   when pane text capture is unavailable, blank, or insufficient?
6. Should fallback visual analysis rely on provider-native image understanding,
   or should the MCP tool also provide OCR/text extraction for providers
   without image input?
7. What input/output contract should a `ccb_self` running-supervision skill
   expose to the CCB-owned
   [maintenance heartbeat](../ccb-maintenance-heartbeat/README.md), while
   keeping scheduler state, cadence changes, and wakeup authority outside the
   Role Pack?
8. Should expert knowledge refresh be a manual skill, a helper command, a
   post-push/release handoff from `push`, or a task triggered only when
   maintenance heartbeat has been manually enabled?
9. Where should the canonical expert references live once materialized:
   inside the `agentroles.ccb_self` Role source only, in CCB source docs, or in
   both with one treated as generated/projection?
