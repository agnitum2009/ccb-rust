# Config Designer UI Plan

Date: 2026-06-06

## Purpose

Plan a focused CCB configuration experience that starts with a cleaner
`ccb-config` skill, then adds an optional local browser editor for
`.ccb/ccb.config`, and finally exposes that editor from the native sidebar.

The plan keeps configuration authority in `.ccb/ccb.config`. It does not turn
the skill or UI into workflow-memory authoring, runtime control, or a second
source of truth.

## File Map

- [roadmap.md](roadmap.md): staged sequence and gates.
- [open-questions.md](open-questions.md): unresolved product and implementation
  questions only.
- [topics/ccb-config-skill-scope.md](topics/ccb-config-skill-scope.md):
  required skill cleanup, menu-style configuration guidance, and boundaries.
- [topics/config-ui-design.md](topics/config-ui-design.md): local browser UI
  shape, API, safety model, and validation flow.
- [topics/sidebar-config-entry.md](topics/sidebar-config-entry.md): sidebar
  icon entry point after the standalone config UI exists.
- [decisions/001-config-ui-is-local-config-editor.md](decisions/001-config-ui-is-local-config-editor.md):
  decision record for keeping the UI local, optional, and config-only.
- [decisions/002-config-single-authority.md](decisions/002-config-single-authority.md):
  decision record for canonical `.ccb/ccb.config` writing rules that keep
  topology authority in `[windows]` and use `[agents.<name>]` only as overlays.

## Related Sources

- [../../../ccb-config-layout-contract.md](../../../ccb-config-layout-contract.md)
- [../managed-tool-windows/README.md](../managed-tool-windows/README.md)
- [../sidebar-tips-layout/README.md](../sidebar-tips-layout/README.md)
- [../workspace-sharing/README.md](../workspace-sharing/README.md)

## Scope

In scope:

- Clean `ccb-config` skill guidance so it edits config only.
- Present configurable fields as a clear menu/list grouped by user level.
- Generate and validate `version = 2` windows topology by default.
- Include the managed Neovim tool window by default in generated windows
  topology, with opt-out by removing `[tool_windows.neovim]`.
- Add a local browser config editor launched by a CLI command.
- Add a sidebar icon that launches the same config editor.

Out of scope:

- Editing `.ccb/ccb_memory.md` or per-agent memory.
- Designing workflow/role memory inside the config skill.
- Writing provider-state homes, installed roles, or runtime records.
- Replacing `ccb reload` or project lifecycle commands.
- Hosting a remote web service or adding a persistent web daemon.
