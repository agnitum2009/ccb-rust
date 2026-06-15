//! Mirrors Python `lib/cli/models_mailbox.py`.
//!
//! Mailbox-related CLI command models. 1:1 alignment with Python dataclasses.

use serde::{Deserialize, Serialize};

/// Parsed `ask` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedAskCommand {
    pub project: Option<String>,
    pub target: String,
    pub sender: Option<String>,
    pub message: String,
    pub task_id: Option<String>,
    pub reply_to: Option<String>,
    pub mode: Option<String>,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub silence: bool,
    #[serde(default)]
    pub callback: bool,
    #[serde(default)]
    pub artifact_request: bool,
    #[serde(default)]
    pub artifact_reply: bool,
    #[serde(default = "default_ask_kind")]
    pub kind: String,
}

fn default_ask_kind() -> String {
    "ask".into()
}

impl ParsedAskCommand {
    pub fn new(
        project: Option<String>,
        target: String,
        sender: Option<String>,
        message: String,
    ) -> Self {
        Self {
            project,
            target,
            sender,
            message,
            task_id: None,
            reply_to: None,
            mode: None,
            compact: false,
            silence: false,
            callback: false,
            artifact_request: false,
            artifact_reply: false,
            kind: "ask".into(),
        }
    }
}

/// Parsed `cancel` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedCancelCommand {
    pub project: Option<String>,
    pub job_id: String,
    #[serde(default = "default_cancel_kind")]
    pub kind: String,
}

fn default_cancel_kind() -> String {
    "cancel".into()
}

impl ParsedCancelCommand {
    pub fn new(project: Option<String>, job_id: String) -> Self {
        Self {
            project,
            job_id,
            kind: "cancel".into(),
        }
    }
}

/// Parsed `pend` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPendCommand {
    pub project: Option<String>,
    pub target: String,
    pub count: Option<i64>,
    #[serde(default = "default_observer_mode")]
    pub observer_mode: String,
    #[serde(default)]
    pub detail: bool,
    #[serde(default = "default_pend_kind")]
    pub kind: String,
}

fn default_observer_mode() -> String {
    "snapshot".into()
}
fn default_pend_kind() -> String {
    "pend".into()
}

impl ParsedPendCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            count: None,
            observer_mode: "snapshot".into(),
            detail: false,
            kind: "pend".into(),
        }
    }
}

/// Parsed `queue` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedQueueCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default)]
    pub detail: bool,
    #[serde(default = "default_queue_kind")]
    pub kind: String,
}

fn default_queue_kind() -> String {
    "queue".into()
}

impl ParsedQueueCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            detail: false,
            kind: "queue".into(),
        }
    }
}

/// Parsed `trace` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedTraceCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default = "default_trace_kind")]
    pub kind: String,
}

fn default_trace_kind() -> String {
    "trace".into()
}

impl ParsedTraceCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            kind: "trace".into(),
        }
    }
}

/// Parsed `resubmit` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedResubmitCommand {
    pub project: Option<String>,
    pub message_id: String,
    #[serde(default = "default_resubmit_kind")]
    pub kind: String,
}

fn default_resubmit_kind() -> String {
    "resubmit".into()
}

impl ParsedResubmitCommand {
    pub fn new(project: Option<String>, message_id: String) -> Self {
        Self {
            project,
            message_id,
            kind: "resubmit".into(),
        }
    }
}

/// Parsed `retry` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedRetryCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default = "default_retry_kind")]
    pub kind: String,
}

fn default_retry_kind() -> String {
    "retry".into()
}

impl ParsedRetryCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            kind: "retry".into(),
        }
    }
}

/// Parsed `wait` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedWaitCommand {
    pub project: Option<String>,
    pub mode: String,
    pub target: String,
    pub quorum: Option<i64>,
    pub timeout_s: Option<f64>,
    #[serde(default = "default_wait_kind")]
    pub kind: String,
}

fn default_wait_kind() -> String {
    "wait".into()
}

impl ParsedWaitCommand {
    pub fn new(project: Option<String>, mode: String, target: String) -> Self {
        Self {
            project,
            mode,
            target,
            quorum: None,
            timeout_s: None,
            kind: "wait".into(),
        }
    }
}

/// Parsed `watch` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedWatchCommand {
    pub project: Option<String>,
    pub target: String,
    #[serde(default = "default_watch_kind")]
    pub kind: String,
}

fn default_watch_kind() -> String {
    "watch".into()
}

impl ParsedWatchCommand {
    pub fn new(project: Option<String>, target: String) -> Self {
        Self {
            project,
            target,
            kind: "watch".into(),
        }
    }
}

/// Parsed `inbox` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedInboxCommand {
    pub project: Option<String>,
    pub agent_name: String,
    #[serde(default)]
    pub detail: bool,
    #[serde(default = "default_inbox_kind")]
    pub kind: String,
}

fn default_inbox_kind() -> String {
    "inbox".into()
}

impl ParsedInboxCommand {
    pub fn new(project: Option<String>, agent_name: String) -> Self {
        Self {
            project,
            agent_name,
            detail: false,
            kind: "inbox".into(),
        }
    }
}

/// Parsed `ack` command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedAckCommand {
    pub project: Option<String>,
    pub agent_name: String,
    pub inbound_event_id: Option<String>,
    #[serde(default = "default_ack_kind")]
    pub kind: String,
}

fn default_ack_kind() -> String {
    "ack".into()
}

impl ParsedAckCommand {
    pub fn new(project: Option<String>, agent_name: String) -> Self {
        Self {
            project,
            agent_name,
            inbound_event_id: None,
            kind: "ack".into(),
        }
    }
}
