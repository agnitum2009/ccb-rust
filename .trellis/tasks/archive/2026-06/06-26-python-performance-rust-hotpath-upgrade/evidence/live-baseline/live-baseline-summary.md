# Live Python latest baseline summary

| Scenario | Samples | Avg CPU | ccbd CPU | Codex bucket CPU | Codex procs max | Total procs max |
|---|---:|---:|---:|---:|---:|---:|
| 2codex | 10 | 73.740% | 26.370% | 29.950% | 11 | 23 |
| 4codex | 12 | 107.700% | 18.783% | 71.075% | 21 | 39 |

## Notes

- Profiles were run from `/home/agnitum/test_ccb2/*` via `/home/agnitum/ccb-git/ccb_test`, satisfying the source-runtime guard.
- Codex hooks were not disabled or masked.
- Each run ended with `ccb_test kill -f` and a process-residue check.
- These are startup/idle live profiles, not ask-storm profiles; ask/callback functional gates remain pending before Slice A/B enablement.
