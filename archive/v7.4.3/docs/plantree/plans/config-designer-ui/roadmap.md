# Config Designer UI Roadmap

Date: 2026-06-06

## Done

- Confirmed current config authority is complete replacement by source layer:
  built-in default, then user config, then project config.
- Confirmed current built-in default already includes a managed Neovim tool
  window.
- Confirmed current `ccb-config` skill already prefers `version = 2`
  `[windows]` topology.
- Updated inherited `ccb-config` skill sources so generated windows topology
  includes `[tool_windows.neovim]` by default.
- Cleaned the inherited `ccb-config` skill scope so it is config-only, shows a
  numbered option menu, and treats workflow memory as a separate follow-up.
- Reorganized the config option menu into Basic, Agent Advanced, Workspace
  Advanced, Provider Startup Advanced, Runtime Advanced, and Output groups.
- Documented that the built-in no-config default includes the managed Neovim
  tool window.
- Added language-following rules so `ccb-config` presents menus, questions, and
  explanations in the user's language while keeping CCB syntax literal.
- Accepted the single-authority config writing rule in
  [decisions/002-config-single-authority.md](decisions/002-config-single-authority.md):
  `[windows]` owns agent presence, provider, default `inplace`/`git-worktree`
  workspace mode, ordering, and window grouping; `[agents.<name>]` is overlay
  only.
- Updated generated role binding behavior so custom local Role Pack bindings no
  longer write redundant overlay `provider`.
- Added `ccb config validate` style warnings for redundant provider,
  redundant default workspace mode, overriding `inplace`/`git-worktree`
  workspace mode, and stale `[agents.<name>]` overlays.
- Updated `ccb_self`'s built-in `ccb-config` guidance to use Role Pack
  shorthand or role-only overlays and to treat style warnings as cleanup before
  reload.

## In Progress

- Keep the staged UI plan aligned with the single-authority config writing
  contract and the private `ccb_self` `ccb-config` skill.

## Next

1. Dogfood the cleaned `ccb-config` skill on a representative config migration.
2. Extract a supported config field registry so parser validation, docs,
   `ccb_self` skill guidance, UI metadata, and formatter behavior cannot drift.
3. Design and implement `ccb config format` or `ccb config normalize --write`
   for safe cleanup of redundant provider/default-workspace fields and stale
   overlays.
4. Design and implement `ccb config ui`:
   - local-only browser UI;
   - current config loading and draft editing;
   - TOML preview, diff, validation, and apply.
5. Add sidebar config icon only after `ccb config ui` is usable:
   - right-side icon in the tree header;
   - launch the same CLI command;
   - show fallback URL/status in the sidebar when browser open fails.

## Deferred

- Remote/shared configuration UI.
- Full drag-and-drop layout designer.
- Import/export of reusable team presets.
- Provider credential vault integration.
- Applying runtime reload directly from the first UI slice.
- Editing project workflow memory from the config UI.

## Phase Gates

Phase 1 is complete when:

- `ccb-config` skill can list supported config knobs clearly.
- The skill writes only `.ccb/ccb.config` or an explicitly requested
  `~/.ccb/ccb.config`; workflow memory remains a separate follow-up.
- Generated windows topology includes the managed Neovim tool window by
  default and validates with the current loader.

Phase 2 is complete when:

- `ccb config ui` opens a local browser editor on `127.0.0.1`.
- The editor can load, preview, validate, and apply `.ccb/ccb.config`.
- Apply shows a diff and validation result before writing.

Phase 3 is complete when:

- The sidebar shows a config icon without adding text buttons.
- Clicking the icon launches `ccb config ui` or displays a fallback URL.
- Existing sidebar restart and kill controls keep their current behavior.
