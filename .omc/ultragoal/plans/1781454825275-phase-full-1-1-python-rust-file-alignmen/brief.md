# Phase: Full 1:1 Python→Rust file alignment (1139 gaps)

Systematic, progressive alignment of all 1139 Python .py files that have no
same-named .rs counterpart, categorized into A/B/C:

- A (structural): functionality exists in Rust but under consolidated/renamed
  files → split/rename to match Python file boundaries (like the reload split).
- B (functional): Python has logic with no Rust equivalent → translate.
- C (skip): internal helpers/runtime subpackages replaced by Rust architecture
  → mark as not-applicable with a rationale comment.

Exit: every Python .py has a corresponding .rs (real code or documented stub),
cargo build --workspace passes, tests pass, clippy/fmt clean.
