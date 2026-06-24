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
        let _ = write_ctx_transfer_usage(&mut stdout);
        return ExitCode::from(0);
    }

    delegate_to_ccb("ctx-transfer")
}

fn write_ctx_transfer_usage<W: Write>(out: &mut W) -> std::io::Result<()> {
    writeln!(out, "Usage: ctx-transfer [OPTIONS]")?;
    writeln!(out)?;
    writeln!(out, "Transfer conversation context between CCB agents.")?;
    writeln!(out)?;
    writeln!(out, "Options:")?;
    writeln!(
        out,
        "  -n, --last <N>            Number of conversation pairs (default: 3)"
    )?;
    writeln!(
        out,
        "  --from, --source <PROVIDER>  Source provider (auto/claude/codex/gemini/opencode/droid)"
    )?;
    writeln!(
        out,
        "  --agent <NAME>            Target agent name (required with --send)"
    )?;
    writeln!(out, "  --send                    Send to agent via ask")?;
    writeln!(
        out,
        "  -d, --dry-run             Preview output without sending"
    )?;
    writeln!(out, "  -o, --output <PATH>       Write output to file")?;
    writeln!(
        out,
        "  --session-path <PATH>     Explicit session JSONL path"
    )?;
    writeln!(
        out,
        "  --max-tokens <N>          Maximum tokens to transfer (default: 8000)"
    )?;
    writeln!(
        out,
        "  -f, --format <FORMAT>     Output format: markdown/plain/json (default: markdown)"
    )?;
    writeln!(
        out,
        "  -q, --quiet               Suppress informational output"
    )?;
    writeln!(
        out,
        "  -s, --save                Save transfer to ./.ccbr/history/"
    )?;
    writeln!(
        out,
        "  --no-save                 Disable auto-save when sending"
    )?;
    writeln!(
        out,
        "  --detailed                Output detailed tool executions"
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
