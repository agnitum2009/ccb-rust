# Distribution And Trust

Date: 2026-06-01

## Objective

Support community roles without turning role installation into an unsafe script
runner. Distribution must be explicit, inspectable, lockable, and reversible.

## Distribution Sources

Built-in:

```text
source = "builtin"
```

Local path:

```text
source = "path:/home/user/roles/archi"
```

GitHub path:

```text
source = "github:SeemSeam/architec//roles/archi?ref=v0.1.0"
```

Future registry:

```text
source = "rolepack:seemseam.archi@0.1.0"
```

## Trust Stages

1. Inspect: show manifest, permissions, tools, network needs, and files.
2. Resolve: download or locate role assets into a staging directory.
3. Verify: compute digest and validate manifest schema.
4. Trust: user approves installation of this digest/source.
5. Install: write into the system role store.
6. Lock: project records exact role version and digest when bound.

## Installer Rules

- Role install may fetch role assets.
- Tool install may fetch external dependencies only when the role declares the
  dependency and the user approves or passes an explicit non-interactive flag.
- Role install must not read or write provider sessions.
- Role install must not write secrets into project config.
- Role updates must not change project locks without an explicit project update
  or `ccb roles update --apply-lock`.

## Digest And Provenance

Every installed role should have a metadata record:

```json
{
  "schema": "rolepack-install/v1",
  "id": "ccb.archi",
  "version": "0.1.0",
  "source": "builtin",
  "digest": "sha256:...",
  "installed_at": "2026-06-01T00:00:00Z"
}
```

The digest is the authority for immutable installed content. A `current`
symlink or pointer may move only after install succeeds.

## Community PR Governance

Community roles submitted to the CCB repository should include:

- manifest validation
- README and examples
- provider skill tests where possible
- declared permissions and external dependencies
- doctor command or explicit "no external tools" declaration
- no credentials, sessions, or binary blobs unless specifically justified
- versioned changelog for breaking role behavior changes

## Deferred Security Work

- Signed role manifests.
- Public transparency log.
- Remote registry ownership verification.
- Sandboxed third-party install scripts.
- Automated malware scanning.

