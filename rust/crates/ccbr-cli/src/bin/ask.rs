use ccbr_cli::ask_usage::write_ask_usage;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        let mut stdout = std::io::stdout();
        let _ = write_ask_usage(
            &mut stdout,
            "ask",
            None,
            Some("`ask` is a compatibility alias for `ccbr ask`."),
        );
        return ExitCode::from(0);
    }

    delegate_to_ccbr("ask")
}

fn delegate_to_ccbr(subcommand: &str) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ccbr_path = find_ccbr_binary().unwrap_or_else(|| PathBuf::from("ccbr"));

    // `--version` is treated as a top-level introspection flag so the helper
    // binaries report the same version as `ccbr` itself.
    let mut cmd = Command::new(&ccbr_path);
    if args.iter().any(|a| a == "--version") {
        cmd.args(&args);
    } else {
        cmd.arg(subcommand).args(&args);
    }

    match cmd.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("failed to run {}: {}", ccbr_path.display(), err);
            ExitCode::from(1)
        }
    }
}

fn find_ccbr_binary() -> Option<PathBuf> {
    let mut exe = std::env::current_exe().ok()?;
    if let Ok(resolved) = std::fs::canonicalize(&exe) {
        exe = resolved;
    }
    let dir = exe.parent()?;
    // The canonical Rust binary is named `ccbr`; fall back to legacy `ccbr`.
    let ccbr = dir.join("ccbr");
    if ccbr.is_file() {
        return Some(ccbr);
    }
    let ccbr = dir.join("ccbr");
    if ccbr.is_file() {
        return Some(ccbr);
    }
    None
}
