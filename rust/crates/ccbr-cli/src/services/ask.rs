//! Mirrors Python `lib/cli/services/ask.py`.

use crate::context::CliContext;
use crate::models_mailbox::ParsedAskCommand;
use crate::services::ask_runtime::submission::SubmitClient;
use crate::services::daemon::connect_mounted_daemon;
use crate::services::DaemonClient;

pub use crate::ask_sender::resolve_ask_sender;
pub use crate::services::ask_runtime::{
    exit_code_for_ask_status, load_persisted_terminal_watch_payload, message_with_reply_guidance,
    submit_ask_with, watch_ask_job, watch_ask_job_with, write_ask_output, AskSummary,
    SubmitClient as AskSubmitClient,
};

/// Default project `ask` submission.
pub fn submit_ask(context: &CliContext, command: &ParsedAskCommand) -> anyhow::Result<AskSummary> {
    submit_ask_with(
        context,
        command,
        ccbr_agents::config::load_project_config,
        resolve_ask_sender,
        invoke_mounted_daemon,
    )
}

/// Invoke a request against the mounted daemon, reconnecting on transient errors.
///
/// Mirrors Python `invoke_mounted_daemon`. This minimal Rust implementation
/// connects once and calls `request_fn`; retry translation is deferred.
pub fn invoke_mounted_daemon(
    context: &CliContext,
    _allow_restart_stale: bool,
    request_fn: &dyn Fn(&dyn SubmitClient) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<serde_json::Value> {
    let handle = connect_mounted_daemon(context, false)?;
    let client = DaemonSubmitClient {
        inner: handle.client,
    };
    request_fn(&client)
}

struct DaemonSubmitClient {
    inner: crate::services::UnixDaemonClient,
}

impl SubmitClient for DaemonSubmitClient {
    fn submit(
        &self,
        envelope: &ccbr_daemon::models::api_models::messages::MessageEnvelope,
    ) -> anyhow::Result<serde_json::Value> {
        self.inner
            .call("submit", envelope.to_record())
            .map_err(|e| anyhow::anyhow!(e))
    }
}
