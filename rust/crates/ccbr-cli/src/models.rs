//! Mirrors Python `lib/cli/models.py`.
//!
//! Unified `ParsedCommand` enum aggregating all CLI command types.
//! 1:1 alignment with Python Union type.

use serde::{Deserialize, Serialize};

pub use crate::models_faults::{
    ParsedFaultArmCommand, ParsedFaultClearCommand, ParsedFaultListCommand,
};
pub use crate::models_mailbox::{
    ParsedAckCommand, ParsedAskCommand, ParsedCancelCommand, ParsedInboxCommand, ParsedPendCommand,
    ParsedQueueCommand, ParsedResubmitCommand, ParsedRetryCommand, ParsedTraceCommand,
    ParsedWaitCommand, ParsedWatchCommand,
};
pub use crate::models_start::{
    ParsedCleanupCommand, ParsedClearCommand, ParsedConfigValidateCommand, ParsedDoctorCommand,
    ParsedKillCommand, ParsedLogsCommand, ParsedMaintenanceCommand, ParsedMobileCommand,
    ParsedPingCommand, ParsedPsCommand, ParsedReloadCommand, ParsedRestartCommand,
    ParsedStartCommand,
};

/// Union of all parsed CLI commands.
///
/// Mirrors Python `ParsedCommand = Union[...]` with serde tag dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ParsedCommand {
    #[serde(rename = "start")]
    Start(ParsedStartCommand),
    #[serde(rename = "kill")]
    Kill(ParsedKillCommand),
    #[serde(rename = "clear")]
    Clear(ParsedClearCommand),
    #[serde(rename = "restart")]
    Restart(ParsedRestartCommand),
    #[serde(rename = "maintenance")]
    Maintenance(ParsedMaintenanceCommand),
    #[serde(rename = "mobile")]
    Mobile(ParsedMobileCommand),
    #[serde(rename = "cleanup")]
    Cleanup(ParsedCleanupCommand),
    #[serde(rename = "ps")]
    Ps(ParsedPsCommand),
    #[serde(rename = "config-validate")]
    ConfigValidate(ParsedConfigValidateCommand),
    #[serde(rename = "reload")]
    Reload(ParsedReloadCommand),
    #[serde(rename = "doctor")]
    Doctor(ParsedDoctorCommand),
    #[serde(rename = "logs")]
    Logs(ParsedLogsCommand),
    #[serde(rename = "ping")]
    Ping(ParsedPingCommand),
    #[serde(rename = "ask")]
    Ask(ParsedAskCommand),
    #[serde(rename = "cancel")]
    Cancel(ParsedCancelCommand),
    #[serde(rename = "pend")]
    Pend(ParsedPendCommand),
    #[serde(rename = "queue")]
    Queue(ParsedQueueCommand),
    #[serde(rename = "trace")]
    Trace(ParsedTraceCommand),
    #[serde(rename = "resubmit")]
    Resubmit(ParsedResubmitCommand),
    #[serde(rename = "retry")]
    Retry(ParsedRetryCommand),
    #[serde(rename = "wait")]
    Wait(ParsedWaitCommand),
    #[serde(rename = "watch")]
    Watch(ParsedWatchCommand),
    #[serde(rename = "inbox")]
    Inbox(ParsedInboxCommand),
    #[serde(rename = "ack")]
    Ack(ParsedAckCommand),
    #[serde(rename = "fault-list")]
    FaultList(ParsedFaultListCommand),
    #[serde(rename = "fault-arm")]
    FaultArm(ParsedFaultArmCommand),
    #[serde(rename = "fault-clear")]
    FaultClear(ParsedFaultClearCommand),
}

impl ParsedCommand {
    /// Extract the project field common to all variants.
    pub fn project(&self) -> Option<&str> {
        match self {
            Self::Start(c) => c.project.as_deref(),
            Self::Kill(c) => c.project.as_deref(),
            Self::Clear(c) => c.project.as_deref(),
            Self::Restart(c) => c.project.as_deref(),
            Self::Maintenance(c) => c.project.as_deref(),
            Self::Cleanup(c) => c.project.as_deref(),
            Self::Ps(c) => c.project.as_deref(),
            Self::ConfigValidate(c) => c.project.as_deref(),
            Self::Reload(c) => c.project.as_deref(),
            Self::Doctor(c) => c.project.as_deref(),
            Self::Logs(c) => c.project.as_deref(),
            Self::Ping(c) => c.project.as_deref(),
            Self::Mobile(c) => c.project.as_deref(),
            Self::Ask(c) => c.project.as_deref(),
            Self::Cancel(c) => c.project.as_deref(),
            Self::Pend(c) => c.project.as_deref(),
            Self::Queue(c) => c.project.as_deref(),
            Self::Trace(c) => c.project.as_deref(),
            Self::Resubmit(c) => c.project.as_deref(),
            Self::Retry(c) => c.project.as_deref(),
            Self::Wait(c) => c.project.as_deref(),
            Self::Watch(c) => c.project.as_deref(),
            Self::Inbox(c) => c.project.as_deref(),
            Self::Ack(c) => c.project.as_deref(),
            Self::FaultList(c) => c.project.as_deref(),
            Self::FaultArm(c) => c.project.as_deref(),
            Self::FaultClear(c) => c.project.as_deref(),
        }
    }

    /// Return the command kind string.
    pub fn kind(&self) -> &str {
        match self {
            Self::Start(c) => &c.kind,
            Self::Kill(c) => &c.kind,
            Self::Clear(c) => &c.kind,
            Self::Restart(c) => &c.kind,
            Self::Maintenance(c) => &c.kind,
            Self::Cleanup(c) => &c.kind,
            Self::Ps(c) => &c.kind,
            Self::ConfigValidate(c) => &c.kind,
            Self::Reload(c) => &c.kind,
            Self::Doctor(c) => &c.kind,
            Self::Logs(c) => &c.kind,
            Self::Ping(c) => &c.kind,
            Self::Mobile(c) => &c.kind,
            Self::Ask(c) => &c.kind,
            Self::Cancel(c) => &c.kind,
            Self::Pend(c) => &c.kind,
            Self::Queue(c) => &c.kind,
            Self::Trace(c) => &c.kind,
            Self::Resubmit(c) => &c.kind,
            Self::Retry(c) => &c.kind,
            Self::Wait(c) => &c.kind,
            Self::Watch(c) => &c.kind,
            Self::Inbox(c) => &c.kind,
            Self::Ack(c) => &c.kind,
            Self::FaultList(c) => &c.kind,
            Self::FaultArm(c) => &c.kind,
            Self::FaultClear(c) => &c.kind,
        }
    }
}
