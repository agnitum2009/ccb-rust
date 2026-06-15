# Implementation Sequence

Date: 2026-06-07

## Current Gate

Phase 0 and Phase 1 were partially completed during the first implementation
pass. The architecture decision is settled, and provider behavior changes should
continue to follow the phase order below.

- Codex target policy excludes project `AGENTS.md` from generated
  `CODEX_HOME/AGENTS.md`; source-home `AGENTS.md` is classified as filtered
  provider user memory.
- OpenCode 1.16.2 natively discovers project `AGENTS.md` while also loading
  configured `instructions`; CCB therefore excludes project `AGENTS.md` from
  the generated runtime bundle.

Implementation must proceed in phases so future provider behavior is not changed
before source ownership assumptions are proven.

## Phase 0: Readiness Audits

Goal: close assumptions before code changes.

Tasks:

1. Audit OpenCode project `AGENTS.md` loading. Completed for OpenCode 1.16.2.
   - Inspect managed OpenCode startup with generated `OPENCODE_CONFIG`.
   - Determine whether project `AGENTS.md` is loaded by OpenCode itself,
     loaded only through `.ccb/runtime/memory/<agent>.md`, or both.
   - Update the OpenCode manifest row and `docs/opencode-completion-contract.md`
     before changing implementation.
2. Classify Codex source-home `AGENTS.md`. Classified as provider user memory.
   - Confirm whether `source_home / "AGENTS.md"` is true account memory, old
     CCB install output, or normally absent.
   - Decide whether provider-user-memory filters apply to that file.
3. Inventory released install markers and legacy sections. Initial inventory
   completed from `install.sh`, `config/*.md`, and Claude cleanup runtime.
   - Include marker-pair blocks from `install.sh`.
   - Include unmarked legacy sections such as old collaboration-rule headings.
   - Decide how to treat Claude route-mode
     `~/.claude/rules/ccb-config.md`: stop writing it, classify it as native
     rules residue, or provide a migration/removal path.
4. Decide old `.ccb/ccb_memory.md` seed migration. Completed with a
   metadata-based policy.
   - Auto-upgrade only files whose current hash still matches the hash recorded
     in `memory.seed.json` for an older generated template.
   - Leave user-edited files untouched.

Exit criteria:

- OpenCode `provider_native_project` row is no longer conditional. Done.
- Codex source-home `AGENTS.md` ownership is documented. Done.
- The filter allowlist has exact marker and legacy-section shapes. Done for
  current released installer paths.
- Old template migration policy is documented with seed metadata rules.

## Phase 1: Contract Alignment

Goal: make docs match the target policy before implementation.

Required updates:

- `docs/claude-session-isolation-contract.md`
  - State that managed Claude generated memory excludes project `CLAUDE.md`.
  - State that inherited provider user memory is filtered for CCB install
    residue.
- `docs/codex-session-isolation-contract.md`
  - State that managed `CODEX_HOME/AGENTS.md` excludes provider-native project
    `AGENTS.md` when Codex native project loading owns it.
  - Clarify source-home `AGENTS.md` ownership after Phase 0.
- `docs/opencode-completion-contract.md`
  - Align the `AGENTS.md` input-source statement with the OpenCode audit.
  - Preserve the generated `opencode.json` instructions merge contract.
- `docs/ccbd-startup-supervision-contract.md`
  - Update provider memory source language if it still implies all
    provider-native project files are bundled.
- `docs/ccb-project-shared-memory-plan.md`
  - Keep it as a historical shared-memory plan with a supersession note for
    provider-native project inclusion.

Exit criteria:

- No authoritative contract says a provider-native project memory file is
  bundled when the manifest says it is excluded.
- The OpenCode contract and manifest agree.

## Phase 2: Policy And Filters

Goal: add source ownership as code, not renderer heuristics.

Code touch list:

- Add `lib/project_memory/policy.py`.
  - Define source kinds and provider policies.
  - Provide a lookup for provider and source kind.
  - Keep defaults explicit; unknown providers should not silently inherit
    Claude or Codex behavior.
- Add or extend a filter module, for example `lib/project_memory/filters.py`.
  - Filter only `provider_user_memory`.
  - Strip complete recognized marker pairs.
  - Strip recognized legacy collaboration sections.
  - Preserve isolated markers and unrelated user text.
- Update `lib/project_memory/sources.py`.
  - Replace ad hoc `include_provider_native_project` usage with policy-backed
    inclusion decisions.
  - Preserve compatibility for existing direct tests until callers are migrated,
    or remove the flag in the same patch with updated tests.
- Update `lib/project_memory/types.py` if source filtering needs metadata.
  - Candidate metadata: original path, filtered flag, filter names, warning.
- Keep `lib/project_memory/renderer.py` focused on rendering selected sources
  and the single CCB runtime coordination section.

Exit criteria:

- Policy decides source inclusion by provider and source kind.
- Filters never run on `.ccb/ccb_memory.md`, agent-private memory, or
  provider-native project memory.
- Projection events or diagnostics can report that provider user memory was
  filtered without leaking large or secret content.

## Phase 3: Provider Wiring

Goal: route each managed provider through the same policy vocabulary.

Claude:

- `lib/provider_backends/claude/launcher_runtime/home.py`
  - Keep project `CLAUDE.md` excluded.
  - Replace the direct flag with the Claude provider policy.
  - Apply provider-user-memory filters to source-home `.claude/CLAUDE.md`.

Codex:

- `lib/provider_core/memory_projection.py`
- `lib/provider_profiles/codex_home_config.py`
  - Use the Codex provider policy.
  - Exclude project `AGENTS.md` from generated `CODEX_HOME/AGENTS.md` after
    Phase 0 confirms native project loading ownership.
  - Apply provider-user-memory filters to source-home `AGENTS.md` if Phase 0
    classifies it as inherited user memory.

OpenCode:

- `lib/project_memory/materializer.py`
- `lib/provider_backends/opencode/launcher.py`
  - Use the audited OpenCode policy for runtime memory bundle sources.
  - Keep `opencode.json` instructions merge behavior stable.
  - Do not inline raw project `AGENTS.md` through config unless the audited
    policy explicitly requires it.

Gemini:

- Defer behavior changes until its `contextFileName` loading path is audited.
- Keep explicit manifest rows so future changes do not silently copy the
  Claude/Codex policy.

Exit criteria:

- Claude, Codex, and OpenCode all select sources through provider policy.
- Provider-native project memory inclusion matches contracts.
- `inherit_memory=false` behavior remains provider-specific but still removes
  generated projection consistently.

## Phase 4: Template And Migration

Goal: remove duplicated ask protocol from new shared memory without rewriting
user content.

Code touch list:

- `lib/project_memory/template.py`
  - Bump `TEMPLATE_VERSION`.
  - Remove the full Ask Communication section from the default template.
  - Keep concise collaboration guidance that points to renderer-owned runtime
    coordination rules.
- `lib/project_memory/seed.py`
  - Add seed-aware upgrade only when seed metadata proves the current file still
    matches an older generated template.
  - Leave user-edited files untouched.

Exit criteria:

- New `.ccb/ccb_memory.md` files do not duplicate ask protocol text.
- Existing edited `.ccb/ccb_memory.md` files are not overwritten.
- Existing unedited seeded `.ccb/ccb_memory.md` files upgrade to the current
  template.
- Rendered bundles still contain CCB ask protocol exactly once from the
  renderer.

## Phase 5: Tests And Validation

Unit tests:

- `test/test_project_memory.py`
  - Provider policy include/exclude behavior.
  - Filter complete marker pairs.
  - Preserve isolated markers and unrelated markers.
  - Strip legacy unmarked collaboration sections.
  - Template version and old-template migration behavior.
- `test/test_project_memory_filters.py`
  - Strip all recognized install marker pairs.
  - Strip English and Chinese legacy collaboration-rule sections.
  - Preserve isolated markers, unrelated user text, and user-authored paragraph
    spacing.
- `test/test_project_memory_real_context.py`
  - Build a realistic temporary CCB project shape and run the real Claude,
    Codex, OpenCode, and Gemini memory materializers.
  - Assert each provider's rendered bundle has one runtime coordination section
    and the expected source ownership composition.
- `test/test_provider_memory_external_context.py`
  - Opt-in final inspection for the dedicated real external source-test
    project `/home/bfly/yunwei/test_ccb2`.
  - Default skip unless `CCB_REAL_PROJECT_MEMORY_CHECK=1` is set.
- `test/test_provider_core_memory_projection.py`
  - Provider-user-memory filtering metadata in projection results/events.
- `test/test_provider_profiles.py`
  - Claude excludes project `CLAUDE.md`.
  - Codex excludes project `AGENTS.md`.
  - Gemini unchanged unless audited.
- `test/test_provider_hook_settings.py`
  - OpenCode instructions merge still includes the bridge path and preserves
    user project config entries.
- `test/test_v2_runtime_launch.py`
  - Runtime launch projections remain stable and emit one memory projection
    event.

External runtime validation:

- Use `/home/bfly/yunwei/ccb_source/ccb_test` only from the dedicated external
  source-test project `/home/bfly/yunwei/test_ccb2`, unless another external
  project is explicitly allowed with `CCB_TEST_ROOTS` or
  `CCB_SOURCE_ALLOWED_ROOTS`.
- Validate managed Claude, Codex, and OpenCode startup bundles after the unit
  tests pass.
- After generated files exist, run the opt-in external context check:

```bash
CCB_REAL_PROJECT_MEMORY_CHECK=1 \
CCB_REAL_TEST_PROJECT=/home/bfly/yunwei/test_ccb2 \
pytest -q test/test_provider_memory_external_context.py
```

- Do not run source runtime validation from `ccb_source` itself.

Exit criteria:

- Unit tests covering source policy, filtering, and provider projection pass.
- The realistic temporary project fixture passes.
- The external project context check passes after `ccb_test` has regenerated the
  managed provider memory files from the source-under-test runtime.
- External `ccb_test` runtime validation confirms generated memory size and
  source composition for Claude, Codex, and OpenCode.
- Plan-tree status is updated with landed evidence after implementation.

## Rollout And Rollback

Rollout:

- Land contract updates before behavior changes.
- Land policy/filter foundation before provider-specific caller rewrites.
- Keep provider behavior changes scoped by provider so Claude fixes do not
  depend on OpenCode uncertainty.

Rollback:

- Reverting provider wiring should restore previous source inclusion behavior
  without touching user-authored memory files.
- Filter changes must be reversible because they only affect generated bundles;
  they must not edit provider user memory in place.
- Template migration must never overwrite user-edited `.ccb/ccb_memory.md`, so
  rollback should not need to recover user content.
