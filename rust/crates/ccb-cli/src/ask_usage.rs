//! Mirrors Python `lib/cli/ask_usage.py`.
//!
//! Ask command usage/help text output.
//! 1:1 alignment with Python function.

use std::io::Write;

/// Write the `ask` command usage information to the given writer.
///
/// Mirrors Python `write_ask_usage`.
pub fn write_ask_usage<W: Write>(
    out: &mut W,
    command_name: &str,
    error: Option<&str>,
    alias_note: Option<&str>,
) -> std::io::Result<()> {
    if let Some(err) = error {
        writeln!(out, "error: {}", err)?;
        writeln!(out)?;
    }
    writeln!(out, "Usage:")?;
    writeln!(
        out,
        "  {} [--compact] [--silence] [--callback] [--artifact-request] [--artifact-reply] <target> [--] <message...>",
        command_name
    )?;
    writeln!(
        out,
        "      --compact request a distilled reply that preserves key information"
    )?;
    writeln!(
        out,
        "      --silence request silent-on-success delivery; failures/blockers still surface"
    )?;
    writeln!(
        out,
        "      --callback route the result back as a new task to the current agent"
    )?;
    writeln!(
        out,
        "      --artifact-request force the request body into a CCB text artifact"
    )?;
    writeln!(
        out,
        "      --artifact-reply force the final reply into a CCB text artifact"
    )?;
    writeln!(
        out,
        "      --artifact-io enable both --artifact-request and --artifact-reply"
    )?;
    writeln!(
        out,
        "      nested asks from active tasks must use --callback or --silence"
    )?;
    writeln!(
        out,
        "      sender is inferred from the current workspace agent and falls back to user"
    )?;
    writeln!(out, "      message text may be supplied on stdin")?;
    writeln!(out, "      examples:")?;
    writeln!(
        out,
        "        {} --compact agent1 review latest diff",
        command_name
    )?;
    writeln!(
        out,
        "        {} --silence agent1 run smoke check",
        command_name
    )?;
    writeln!(
        out,
        "        {} --callback agent2 collect evidence for this task",
        command_name
    )?;
    writeln!(
        out,
        "        {} --callback --artifact-reply agent2 collect long evidence",
        command_name
    )?;
    writeln!(
        out,
        "  {} get <job_id>    diagnostics-only: inspect one submitted job",
        command_name
    )?;
    writeln!(out, "  {} cancel <job_id>", command_name)?;
    if let Some(note) = alias_note {
        writeln!(out)?;
        writeln!(out, "{}", note)?;
    }
    Ok(())
}
