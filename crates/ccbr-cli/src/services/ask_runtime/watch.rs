//! Mirrors Python `lib/cli/services/ask_runtime/watch.py`.

use std::io::Write;

use crate::context::CliContext;
use crate::services::DaemonClient;

/// Number of seconds to wait for a watch before giving up.
pub fn watch_timeout_seconds() -> f64 {
    let raw = std::env::var("CCBR_WATCH_TIMEOUT_S").unwrap_or_else(|_| "3600".into());
    raw.trim().parse().unwrap_or(3600.0)
}

/// Number of seconds between watch polls.
pub fn watch_poll_interval_seconds() -> f64 {
    let raw = std::env::var("CCBR_WATCH_POLL_INTERVAL_S").unwrap_or_else(|_| "0.1".into());
    raw.trim().parse::<f64>().unwrap_or(0.1).max(0.0)
}

/// Watch an ask job until it reaches a terminal state.
///
/// Mirrors Python `watch_ask_job`. The Rust version currently uses the
/// daemon's `watch` endpoint to poll for job state.
pub fn watch_ask_job(
    context: &CliContext,
    job_id: &str,
    out: &mut dyn Write,
    timeout: Option<f64>,
    emit_output: bool,
) -> anyhow::Result<serde_json::Value> {
    watch_ask_job_with(
        context,
        job_id,
        out,
        timeout,
        emit_output,
        crate::services::daemon::connect_mounted_daemon,
        |_batch| (String::new(),),
        |out, lines| {
            for line in lines {
                writeln!(out, "{line}").ok();
            }
        },
        watch_timeout_seconds,
        watch_poll_interval_seconds,
        std::time::Instant::now,
        std::thread::sleep,
    )
}

/// Poll-based watch with fully injected dependencies.
#[allow(clippy::too_many_arguments)]
pub fn watch_ask_job_with<W, C, R, WL, TF, PF, IF, SF>(
    context: &CliContext,
    job_id: &str,
    out: &mut W,
    timeout: Option<f64>,
    emit_output: bool,
    connect_mounted_daemon_fn: C,
    render_watch_batch_fn: R,
    write_lines_fn: WL,
    timeout_seconds_fn: TF,
    poll_interval_seconds_fn: PF,
    monotonic_fn: IF,
    sleep_fn: SF,
) -> anyhow::Result<serde_json::Value>
where
    W: Write + ?Sized,
    C: Fn(&CliContext, bool) -> anyhow::Result<crate::services::daemon::DaemonHandle>,
    R: Fn(&serde_json::Value) -> (String,),
    WL: Fn(&mut W, &[String]),
    TF: Fn() -> f64,
    PF: Fn() -> f64,
    IF: Fn() -> std::time::Instant,
    SF: Fn(std::time::Duration),
{
    let deadline = {
        let now = monotonic_fn();
        let timeout_s = timeout.unwrap_or_else(timeout_seconds_fn);
        now + std::time::Duration::from_secs_f64(timeout_s)
    };
    let poll_interval = std::time::Duration::from_secs_f64(poll_interval_seconds_fn().max(0.0));

    let mut handle = connect_mounted_daemon_fn(context, false)?;
    let mut last_cursor = 0i64;

    loop {
        if monotonic_fn() >= deadline {
            return Err(anyhow::anyhow!("watch timed out for {job_id}"));
        }
        let result = _watch_get(&handle.client, job_id, last_cursor);
        match result {
            Ok(batch) => {
                let terminal = batch
                    .get("terminal")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let cursor = batch
                    .get("cursor")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(last_cursor);
                if emit_output {
                    let rendered = render_watch_batch_fn(&batch);
                    write_lines_fn(out, &[rendered.0]);
                }
                if terminal {
                    return Ok(batch);
                }
                last_cursor = cursor;
                sleep_fn(poll_interval);
            }
            Err(_e) => {
                // Reconnect once, matching Python's reconnect behavior.
                handle = connect_mounted_daemon_fn(context, false)?;
                if monotonic_fn() >= deadline {
                    return Err(anyhow::anyhow!("watch timed out for {job_id}"));
                }
            }
        }
    }
}

fn _watch_get(
    client: &crate::services::UnixDaemonClient,
    job_id: &str,
    cursor: i64,
) -> anyhow::Result<serde_json::Value> {
    client
        .call(
            "watch",
            serde_json::json!({ "job_id": job_id, "cursor": cursor }),
        )
        .map_err(|e| anyhow::anyhow!(e))
}

/// Load a persisted terminal watch payload for a completed callback job.
///
/// Mirrors Python `cli.services.watch_fallback.load_persisted_terminal_watch_payload`.
/// This Rust stub returns `None`; full implementation is deferred to the
/// completion/runtime integration phase.
pub fn load_persisted_terminal_watch_payload(
    _context: &CliContext,
    _job_id: &str,
) -> Option<serde_json::Value> {
    None
}
