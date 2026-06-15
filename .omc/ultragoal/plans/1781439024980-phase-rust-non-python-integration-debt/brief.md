# Phase: Rust ↔ non-Python integration debt

Close the two integration gaps from the codegraph audit:
1. Translate the 3 own-system provider hook scripts still in Python
   (ccb-provider-finish-hook 313L, ccb-provider-activity-hook 176L,
   ccb-cleanup 17L) — these are invoked by the Rust ccb-provider-hooks
   crate as external commands.
2. De-Python install.sh (3390L, 192 python refs): it builds Rust binaries
   but still hard-requires Python 3.10+; remove the Python requirement so
   a Rust-only install works.

Each hook lands as a Rust binary in rust/tools or a ccb-cli subcommand;
install.sh keeps cargo build but drops the Python runtime requirement.
