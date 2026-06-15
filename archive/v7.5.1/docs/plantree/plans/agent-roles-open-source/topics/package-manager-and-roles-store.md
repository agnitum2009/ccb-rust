# Package Manager And Roles Store

Date: 2026-06-04

## Objective

Plan the future `agent-roles` package-management layer that owns `.roles`
content sync, installation, update, diagnostics, digest metadata, and alias
migration independently from any one host runtime.

This is the `agent-roles-spec` side of the boundary decided in
[../../rolepack-system/decisions/006-agent-roles-spec-owns-roles-store.md](../../rolepack-system/decisions/006-agent-roles-spec-owns-roles-store.md).

## Responsibilities

The `agent-roles` package manager should own:

- syncing the upstream `agent-roles-spec` repository or configured catalogs
- listing available and installed roles
- installing and updating role package payloads into `.roles`
- computing and recording version, digest, source, provenance, and installed
  path metadata
- resolving aliases and migrations such as `ccb.archi -> agentroles.archi`
- validating package schemas and adapter metadata
- running package-level doctor checks that do not require a specific host
  runtime
- exposing stable machine-readable diagnostics

Host clients such as CCB should own:

- project configuration and locks
- provider/session/runtime projection
- host-specific tool execution policy
- user prompts and localized output
- runtime reload, sidebar, ask, mailbox, and daemon behavior

## CLI/API Shape

The first usable tool should prefer stable JSON output for host clients:

```bash
agent-roles sync . --json
agent-roles list --json
agent-roles install agentroles.archi --json
agent-roles update agentroles.archi --json
agent-roles doctor agentroles.archi --json
agent-roles resolve agentroles.archi --json
```

The human CLI can be friendlier, but host integrations should not scrape human
text. JSON records should include stable diagnostic codes and a schema version.

## Store Shape

The exact default path is still open, but the store should be spec-owned rather
than CCB-private:

```text
~/.roles/
  catalogs/
  installed/
  tools/
  indexes/
```

The store should support content-addressed installed role versions so host
project locks can pin `version + digest` without floating on update.

## CCB Compatibility Requirements

CCB needs the package manager to support:

- canonical id resolution for `agentroles.archi`
- legacy alias resolution for `ccb.archi`
- local editable roles from the current working directory for `sync .`
- same-id installed-role updates without installing every newly available role
- non-interactive failure diagnostics for CCB update
- an installed path that CCB can project from without network access
- stable metadata for `.ccb/role-lock.json`

## Phasing

1. Specify the `.roles` metadata and JSON protocol.
2. Implement a minimal `agent-roles` CLI or library in the spec project.
3. Add CCB compatibility tests that call the package manager from a temporary
   project and assert old CCB stores still resolve.
4. Update CCB wrappers to delegate payload operations to `agent-roles`.
5. Keep CCB-private role store writes as a compatibility path until existing
   v7.2.x installs can migrate safely.

## Open Design Points

- Default store path: `~/.roles`, XDG data, or both.
- Whether tool dependencies live under `.roles/tools` or stay host-owned.
- Whether host clients should use subprocess JSON, a library API, or both.
- How much package doctor can check before host-specific tool policy is
  applied.
- How to garbage collect unreferenced role digests without breaking host
  project locks.
