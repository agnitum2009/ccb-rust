# ccbr

Native Rust implementation of **Claude Codex Bridge** — a multi-agent
orchestration runtime (daemon `ccbrd` + CLI `ccbr`) coordinating provider
agents (Codex, Claude, Gemini, Droid, AGY, OpenCode, …) across tmux panes
with message-bureau dispatch, completion tracking, namespace materialization,
config reload, and supervision.

`ccbr` is the Rust rebrand of the project, kept distinct from the legacy
Python `ccb` to avoid runtime/debug collisions when both are installed.

## Build / Run

```bash
cargo build --release        # workspace at repo root
cargo run -p ccbr-cli        # the ccbr CLI
```

Config: project-local `.ccbr/ccbr.config` (directory `.ccbr/`).

## Test

```bash
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Layout

- `crates/` — `ccbr-cli`, `ccbr-daemon`, `ccbr-providers`, `ccbr-mailbox`,
  `ccbr-completion`, `ccbr-jobs`, `ccbr-agents`, `ccbr-terminal`, `ccbr-storage`,
  `ccbr-memory`, …
- `tools/` — release builder, MCP server, provider finish-hook.

Distribution binaries via `tools/ccbr-release-builder`.
