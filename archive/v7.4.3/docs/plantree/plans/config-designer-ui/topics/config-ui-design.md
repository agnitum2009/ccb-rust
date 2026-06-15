# Config UI Design

Date: 2026-06-06

## Goal

Add an optional browser-based editor for CCB project configuration without
changing config authority. The UI is a convenience layer over `.ccb/ccb.config`;
it is not a persistent service and not a runtime control plane.

## Command Shape

Proposed first command:

```bash
ccb config ui
```

Possible later flags:

```bash
ccb config ui --user
ccb config ui --no-open
ccb config ui --port 0
```

First slice should default to project config editing only.

## Runtime Shape

- CLI starts a short-lived local HTTP server.
- Bind only to `127.0.0.1`.
- Generate a random token and include it in the URL.
- Open the browser automatically when possible.
- Print the URL as fallback.
- Server exits after an idle timeout or explicit close.

## UI Sections

Left navigation:

1. Project
2. Windows
3. Agents
4. Tools
5. Sidebar
6. Workspace
7. Model And API
8. Provider Advanced
9. Runtime
10. Preview And Apply

Default visible sections should prioritize Project, Windows, Agents, Tools, and
Sidebar. Workspace and later sections are advanced.

## API Sketch

```text
GET  /api/config
POST /api/preview
POST /api/validate
POST /api/apply
```

The server should keep the draft in memory. The source of truth remains the file
written by apply.

## Validation And Apply

Before writing:

- render a complete TOML preview;
- show a diff against the current target file;
- run the same config loader validation used by `ccb config validate`;
- warn when likely secret fields are present.

Apply writes only after validation passes and the user confirms.

After writing:

- run validation again;
- show the active source kind and target path;
- tell the user to run `ccb reload --dry-run` / `ccb reload` or restart.

## Boundaries

The UI must not:

- edit memory files;
- install roles or tools;
- write provider-state;
- start, stop, reload, or kill project runtime in the first slice;
- open a remote listener;
- become a long-running daemon.
