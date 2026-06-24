//! `ccbr-cleanup` — deprecation stub.
//!
//! 1:1 port of `bin/ccbr-cleanup` (Python). Standalone cleanup was removed; this
//! binary mirrors the Python deprecation message and non-zero exit so existing
//! callers get an actionable redirect.

fn main() -> std::process::ExitCode {
    eprintln!(
        "error: standalone ccbr-cleanup was removed; use `ccb kill --zombies` for \
         global cleanup or `ccb kill` inside a project"
    );
    std::process::ExitCode::from(1)
}
