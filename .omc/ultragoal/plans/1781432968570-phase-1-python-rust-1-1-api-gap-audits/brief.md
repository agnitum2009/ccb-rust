# Phase 1: Python→Rust 1:1 API Gap Audits

Verify "strict 1:1" completion for the 5 large crates by diffing Python public
symbols (functions/classes/constants exposed via `__all__` or module-level
`def`/`class`/`UPPER =`) against Rust `pub fn`/`pub struct`/`pub const`.

Each goal produces a gap report under `docs/gap-reports/<crate>.md` listing
"Python has / Rust missing" symbols with a suggested landing crate.

Exit criteria per goal: gap report written, count + severity recorded.
