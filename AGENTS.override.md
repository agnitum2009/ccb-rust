# AGENTS.override.md

## Scope
This override applies to `/home/agnitum/ccb` as the CCB project repository. It is the primary implementation repository for the CCB multi-agent CLI workspace tool. Development, testing, and release work happens here directly.

## Project Authority
CCB v7.5.1 — multi-agent CLI workspace using tmux. Source of truth:

```text
/home/agnitum/ccb/README.md          - project overview and quickstart
/home/agnitum/ccb/CHANGELOG.md       - release history and change rationale
/home/agnitum/ccb/docs/              - architecture plans and contracts
/home/agnitum/ccb/config/            - template configs and role definitions
/home/agnitum/ccb/.clinerules        - role assignment (designer/inspiration/reviewer/executor)
```

## Development Location
All development happens in `/home/agnitum/ccb`. No separate internal/external worktree split — this is a single-repo project. The canonical implementation is the Rust workspace under `rust/`.

## Testing Discipline
- Default: targeted `cargo test` per crate (`cargo test -p <crate> -- --test-threads=1`).
- Pre-commit: run related test files, not the full suite.
- Full workspace only for release validation or major refactors: `cargo test --workspace -- --test-threads=1`.
- Integration tests live in `rust/crates/<crate>/tests/`.

## Provider Convention
When adding or modifying provider support:
1. Follow the provider abstraction layer in `rust/crates/ccb-provider-core/`.
2. Each provider needs: launcher, communicator, session fields, polling, and hook integration.
3. Test with `rust/crates/ccb-providers/tests/provider_<name>_tests.rs` files.
4. Document any new config keys in `config/` templates.

## CodeGraph / Structural Reading
This is a Rust project. Use grep/glob or `cargo` for code search. For understanding crate relationships, check `rust/Cargo.toml` workspace members and crate imports.

## Architecture Drift Control
New features that affect the following require discussion before implementation:
- ccbd daemon protocol or socket interface
- Inter-agent messaging (ccb-mailbox)
- Provider abstraction layer contracts
- tmux namespace or pane identity
- `.ccb/ccb.config` schema changes
- Role pack format or loading behavior

For these, write a plan in `docs/` or `plans/` first and get review.

## Reporting
For completed work, report:
```text
Changed files: <list>
Tests run: <command> → <result>
Tests skipped: <reason if any>
Remaining risks: <none or description>
```

## Build and Release
- Linux: `python scripts/build_linux_release.py`
- macOS: `python scripts/build_macos_release.py`
- General: `python scripts/build_release.py`

These Python scripts are thin wrappers around the Rust `ccb-release-builder` tool (`rust/tools/ccb-release-builder`). The builder compiles the Rust workspace and packages the native binaries (`ccb`, `ccbd`, `ask`, `autonew`, `ctx-transfer`) into the release tarball. `install.sh` installs these native binaries directly.

## UI Skill Rule
For sidebar or terminal UI work, review existing `tools/ccb-agent-sidebar` patterns first.

## Review Framework
CCB uses a scored review system (defined in `config/agents-md-ccb.md`):
- Plan review: 5 dimensions (clarity, completeness, feasibility, risk, alignment).
- Code review: 6 dimensions (correctness, security, maintainability, performance, test coverage, plan adherence).
- Pass threshold: overall >= 7.0, no dimension <= 3.
