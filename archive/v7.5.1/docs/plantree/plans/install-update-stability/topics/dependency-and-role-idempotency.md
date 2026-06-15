# Dependency And Role Idempotency

Date: 2026-06-04

## Rule

Provisioning should be check-first, install-only-if-needed.

Each dependency class needs an authority file or probe:

- Python package: import check from the selected Python.
- Managed venv: venv Python exists, is executable, and can import required
  modules.
- Neovim: CCB tool manifest plus wrapper health.
- LazyVim: managed profile marker and health check.
- Role Pack: installed metadata version and digest versus catalog version and
  digest.
- Role-owned tool: tool manifest and doctor result.
- Droid MCP: registration marker or provider-native config status.

## Python Packages

`tomli`:

- skip on Python versions with `tomllib`
- skip when `tomli` or `toml` imports
- install only for the selected Python
- failure is warning unless rich TOML is required by the active command

`watchdog`:

- skip when import succeeds
- install only for the selected Python
- failure is warning; CCB can use polling/readback paths

Both checks must use the same Python that installed entrypoint wrappers will
use.

## Managed Venv

Managed venv can be rebuilt during release install, but post-install
provisioning must not assume user-global packages.

Stable behavior:

- managed venv creation failure is blocking when managed venv is required
- pip upgrade failure is warning if the venv remains usable
- packages are installed inside the venv and then import-checked

## Neovim Tool

Authority:

```text
$XDG_DATA_HOME/ccb/tools/neovim/manifest.json
$XDG_DATA_HOME/ccb/tools/neovim/bin/ccb-nvim
```

No-repeat behavior:

- if wrapper exists and health is ok, `tools install/update neovim` should
  report `ok` without redownloading managed Neovim
- if LazyVim profile health fails, repair only the managed LazyVim profile
- if system `nvim` exists, do not download managed Neovim unless policy
  requires managed binary

## Role Packs

Authority:

```text
$XDG_DATA_HOME/ccb/roles/<role-id>/install.json
$XDG_DATA_HOME/ccb/roles/<role-id>/versions/<version>/<digest>/
$XDG_DATA_HOME/ccb/roles/<role-id>/current
project/.ccb/role-lock.json
```

No-repeat behavior:

- if catalog `version + digest` equals installed metadata, status is `current`
  and update hooks are skipped
- if catalog differs, copy source to staging, compute digest, and update
  `current`
- if an existing target digest directory is polluted, repair it from clean
  staging
- project locks remain unchanged unless the user explicitly adopts the new
  installed digest

Legacy migration:

- all read paths accept `ccb.archi`
- all new writes use `agentroles.archi`
- installed `ccb.archi/install.json` should be copied or migrated to canonical
  `agentroles.archi/install.json` when safe
- stale `source_path` under old CCB source-tree role directories should be
  ignored when a canonical catalog source is available

## Role-Owned Tools

Role tool hooks should be idempotent independently of Role Pack asset updates.

Expected tool hook behavior:

- `install`: create missing wrapper/venv/tool only when missing or version
  mismatch
- `update`: refresh only when source package or requested version changed
- `doctor`: inspect and report without mutating whenever possible
- all Python hooks use `python -B` or equivalent bytecode suppression
- tool state lives under `$XDG_DATA_HOME/ccb/tools/<tool-id>/`, not inside the
  installed role snapshot

## Update Summary Output

Post-update output should classify each item:

- `current`: checked, no install work performed
- `updated`: changed because version/digest/tool version differed
- `skipped`: policy or non-interactive mode skipped it
- `warning`: optional provisioning failed with retry command
- `failed`: required provisioning failed

This classification prevents users from interpreting an optional Role Pack
warning as a failed CCB update.
