# Keep Reviewed Install And Watch Fallback Surfaces

Date: 2026-05-18

## Context

Architec cleanup heuristics flagged `install.ps1` as `obsolete_script` and
`lib/cli/services/watch_fallback.py` as `fallback_branch`. The semantic judge
downgraded both to manual review because neither had enough evidence for safe
archive or deletion.

## Decision

Keep `install.ps1` as an active Windows install surface. It is referenced by
the README files, `install.sh`, `install.cmd`, Windows bootstrap automation,
management install code, release packaging docs, and tests.

Keep `lib/cli/services/watch_fallback.py` as an active runtime fallback. It is
imported by both `cli.services.watch_runtime` and
`cli.services.ask_runtime.watch`, and reconnect tests cover persisted terminal
job fallback behavior.

## Consequences

Cleanup ledgers must not treat these paths as archive or delete candidates
unless a future decision removes their documented callers and tests first.
Fallback naming is not sufficient cleanup evidence in this repository because
fallback paths are part of runtime resilience.
