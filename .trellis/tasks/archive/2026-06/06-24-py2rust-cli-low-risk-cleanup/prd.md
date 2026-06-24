# py2rust parity matrix audit and quick wins

## Goal

Systematically review the remaining `partial` clusters in `plans/rust-python-test-parity-matrix.md`, mark clusters that are already fully covered as `complete`, and close the small, low-risk gaps that are cheaper to finish now than to defer.

## Requirements

1. Inspect every cluster currently marked `partial` in the parity matrix.
2. Compare the mapped Python reference test(s) with the mapped Rust test(s)/implementation(s).
3. If the Rust side already exercises the same behavior as the Python test(s), update the matrix to `complete` with a concise note.
4. If a cluster has a small, surgical gap (e.g., missing assertion, edge-case test, or trivial helper), add the Rust test/code to close it.
5. Document any cluster that requires a larger, separate task (socket/watch contract changes, new subsystems, Windows/WSL-only utilities, live provider CLI tests) as out-of-scope for this audit.

## Acceptance Criteria

- [ ] `plans/rust-python-test-parity-matrix.md` is updated with accurate `complete`/`partial` statuses.
- [ ] At least one previously `partial` cluster is moved to `complete` by audit alone.
- [ ] Any small gaps added during the audit have corresponding passing Rust tests.
- [ ] `cargo test -p <affected-crate> -- --test-threads=1` passes for every crate touched.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy -p <affected-crate> --tests` produces no new warnings.
- [ ] A follow-up list of remaining larger tasks is recorded in the task notes.

## Out of Scope

- Changes to the ccbd control-plane socket protocol or mailbox kernel contracts (must be escalated).
- Full implementation of `ask_service` watch/reconnect/fallback parity.
- Full implementation of `cli_entrypoint` install/update management commands.
- Windows/WSL-specific bootstrap or path utilities.
- Live provider CLI integration tests.

## Notes

- This task intentionally stays inside existing crates and existing test patterns.
- Stop and escalate if an audit reveals that closing a gap would require modifying escalation-guarded surfaces.
