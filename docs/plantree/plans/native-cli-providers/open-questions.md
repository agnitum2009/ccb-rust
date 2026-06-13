# Native CLI Providers Open Questions

Date: 2026-06-13

## Open

- Should CCB later support `provider = "deepcode"` as an alias for
  `provider = "deepseek"` if user configs naturally follow the binary name?
- Should Kimi get a second execution mode based on `kimi --prompt` after the
  pane-backed mode lands and is stable?
- Should CCB add provider-specific config validation for missing Kimi login or
  missing Deep Code API key, or keep that inside `ccb doctor` only?

## Resolved

- First provider key decision: use `kimi` and `deepseek`; map `deepseek` to
  command `deepcode`.
- First execution decision: keep pane-backed managed runtime for Kimi and
  DeepSeek.
- Completion decision update: replace `CCB_DONE` marker detection for Kimi,
  DeepSeek/DeepCode, and AGY with provider-native session/event log detection.
