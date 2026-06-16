use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    delegate_to_ccb("autonew")
}

fn delegate_to_ccb(subcommand: &str) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ccb_path = find_ccb_binary().unwrap_or_else(|| PathBuf::from("ccb"));

    // Introspection flags are safe even in source checkouts, so route them to
    // the top-level ccb binary without the subcommand prefix.
    let mut cmd = Command::new(&ccb_path);
    if args
        .iter()
        .any(|a| a == "--version" || a == "-h" || a == "--help")
    {
        cmd.args(&args);
    } else {
        cmd.arg(subcommand).args(&args);
    }

    match cmd.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("failed to run {}: {}", ccb_path.display(), err);
            ExitCode::from(1)
        }
    }
}

fn find_ccb_binary() -> Option<PathBuf> {
    let mut exe = std::env::current_exe().ok()?;
    if let Ok(resolved) = std::fs::canonicalize(&exe) {
        exe = resolved;
    }
    let dir = exe.parent()?;
    // The canonical Rust binary is named `ccbr`; fall back to legacy `ccb`.
    let ccbr = dir.join("ccbr");
    if ccbr.is_file() {
        return Some(ccbr);
    }
    let ccb = dir.join("ccb");
    if ccb.is_file() {
        return Some(ccb);
    }
    None
}
