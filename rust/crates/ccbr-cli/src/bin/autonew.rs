use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        let mut stdout = std::io::stdout();
        let _ = write_autonew_usage(&mut stdout);
        return ExitCode::from(0);
    }

    delegate_to_ccb("autonew")
}

fn write_autonew_usage<W: Write>(out: &mut W) -> std::io::Result<()> {
    writeln!(out, "Usage: autonew <provider>")?;
    writeln!(out)?;
    writeln!(out, "Providers:")?;
    writeln!(out, "  gemini, codex, opencode, droid, claude")?;
    writeln!(out)?;
    writeln!(
        out,
        "Sends /new to the provider's pane to start a new session."
    )?;
    Ok(())
}

fn delegate_to_ccb(subcommand: &str) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ccbr_path = find_ccbr_binary().unwrap_or_else(|| PathBuf::from("ccb"));

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
