# Open Questions

Date: 2026-05-18

## Questions

- Should `lib/provider_model_shortcuts.py` and `lib/release_artifacts.py` stay
  as root-level implementation modules, or should a later package-root cleanup
  introduce `lib/provider/` and `lib/release/` namespaces?
- Should `lib/cli/management_runtime/startup_update.py`, hotspot rank 8, be
  handled in Phase 2 with release/dev-tooling complexity work or deferred to a
  separate CLI management pass?
- Can OpenCode share only the generic memory projection signature/event helper
  while preserving its extended config merge fields and
  `opencode_config_merge_failed` event, or should it remain separate?
