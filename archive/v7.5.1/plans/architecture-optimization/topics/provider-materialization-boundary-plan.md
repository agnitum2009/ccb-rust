# Provider Materialization Boundary Plan

Date: 2026-05-18

## Purpose

Reduce the highest-risk runtime component by extracting shared provider-home
materialization mechanisms while preserving provider-specific startup,
auth, config, and session contracts.

## Current Inventory

Primary files:

- `lib/provider_profiles/codex_home_config.py`
- `lib/provider_backends/claude/launcher_runtime/home.py`
- `lib/provider_backends/gemini/launcher_runtime/home.py`
- `lib/provider_backends/opencode/launcher.py` for inventory and later review;
  it has similar memory projection mechanics, but its result/event payload also
  carries OpenCode-specific config merge fields and should not be included in
  the first shared-helper extraction.
- `lib/provider_core/projected_assets.py`
- `lib/provider_profiles/materializer.py`
- `lib/cli/services/provider_hooks.py`

Protective tests:

- `test/test_provider_profiles.py`
- `test/test_provider_hook_settings.py`
- `test/test_gemini_launcher_env.py`
- relevant sections of `test/test_v2_runtime_launch.py`

Relevant authority docs:

- [../../../docs/ccb-provider-state-storage-boundary-plan.md](../../../docs/ccb-provider-state-storage-boundary-plan.md)
- [../../../docs/codex-session-isolation-contract.md](../../../docs/codex-session-isolation-contract.md)
- [../../../docs/claude-session-isolation-contract.md](../../../docs/claude-session-isolation-contract.md)
- [../../../docs/gemini-session-isolation-contract.md](../../../docs/gemini-session-isolation-contract.md)
- [../../../docs/codex-plugin-projection-plan.md](../../../docs/codex-plugin-projection-plan.md)

## Boundary Diagnosis

The shared responsibilities are:

- memory bundle rendering result shape;
- memory projection event writing and duplicate suppression;
- projection marker/signature comparison;
- safe text hashing;
- projected asset routing through marker-protected symlink/copy helpers.

The provider-specific responsibilities are:

- Codex `config.toml`, `auth.json`, plugin bundle authority, and route
  authority fingerprint behavior;
- Claude `.claude/settings.json`, `.claude.json`, macOS keychain behavior,
  auth preservation, and hook asset projection;
- Gemini `.gemini/settings.json`, `.env`, auth selected type, trusted folders,
  and `contextFileName` behavior;
- OpenCode `opencode.json` memory config merge behavior.

## Target Shape

Introduce a shared provider projection support layer, likely under
`lib/provider_core/`, that owns small, behavior-preserving helpers:

- `ProjectionResult` or equivalent record shape;
- `memory_projection_result(...)`;
- `record_memory_projection_event(...)`;
- `same_projection_signature(...)`;
- `text_file_sha256(...)`.

The event writer must be provider-parameterized:

```text
record_memory_projection_event(result, *, provider, event_path, marker_path, agent_name)
```

The caller passes the provider name. The shared helper must not own a provider
enum or infer provider identity from paths.

Keep provider modules as orchestration owners:

- they still decide what to project;
- they still decide when inheritance flags remove or preserve files;
- they still own provider-specific merge rules;
- they call shared support helpers for result/event mechanics only.

## Sequence

1. Extract memory projection result and event helpers from one provider into a
   shared module.
2. Switch Codex, Claude, and Gemini to the shared helper without changing
   outputs.
3. Run provider profile, provider hook, Gemini launcher, and runtime-launch test
   slices.
4. Review whether OpenCode can reuse part of the shared helper through explicit
   extension fields, without losing its config merge event behavior.
5. Only after event mechanics are shared, review whether config merge helpers
   can be table-driven.
6. Defer broad provider abstraction until the repeated mechanics are stable.

## Safety Rules

- Do not change inheritance semantics in the same patch as helper extraction.
- Do not move Codex plugin projection logic out of Codex materialization before
  the plugin contract has a narrower shared bundle API.
- Do not move Claude keychain behavior into a generic provider helper.
- Do not change generated event JSON field names.
- Preserve marker-based duplicate suppression.

## Verification

Focused commands:

```bash
pytest test/test_provider_profiles.py test/test_provider_hook_settings.py test/test_gemini_launcher_env.py test/test_v2_runtime_launch.py -x
```

Broader follow-up:

```bash
archi .
```
