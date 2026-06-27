use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(name = "ccbr", version, about = "CCBR multi-agent CLI workspace")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long)]
    pub project: Option<String>,

    #[arg(short, long)]
    pub safe: bool,

    #[arg(short = 'n', long)]
    pub reset: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Ask {
        target: String,
        #[arg(trailing_var_arg = true)]
        message: Vec<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long)]
        compact: bool,
        #[arg(long)]
        silence: bool,
    },
    Wait {
        target: String,
        #[arg(long)]
        quorum: Option<usize>,
        #[arg(long)]
        timeout: Option<f64>,
    },
    Watch {
        target: String,
    },
    Ps {
        #[arg(long)]
        alive: bool,
    },
    Ping {
        target: String,
    },
    Cancel {
        job_id: String,
    },
    Clear {
        #[arg(trailing_var_arg = true)]
        agent_names: Vec<String>,
    },
    Cleanup,
    Kill {
        #[arg(short, long)]
        force: bool,
    },
    Pend {
        target: String,
        count: Option<usize>,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        inbox: bool,
        #[arg(long)]
        queue: bool,
        #[arg(long)]
        detail: bool,
    },
    Queue {
        target: String,
        #[arg(long)]
        detail: bool,
    },
    Trace {
        target: String,
    },
    Resubmit {
        message_id: String,
    },
    Retry {
        target: String,
    },
    Inbox {
        agent_name: String,
        #[arg(long)]
        detail: bool,
    },
    Ack {
        agent_name: String,
        #[arg(long)]
        event_id: Option<String>,
    },
    Logs {
        agent_name: String,
    },
    Maintenance {
        #[command(subcommand)]
        action: MaintenanceAction,
    },
    Doctor {
        #[arg(long)]
        bundle: bool,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        storage: bool,
        #[arg(long)]
        json: bool,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Reload {
        #[arg(long)]
        dry_run: bool,
    },
    Restart {
        agent_name: String,
    },
    Version,
    Update,
    Uninstall,
    Reinstall,
    Roles {
        #[command(subcommand)]
        action: RolesAction,
    },
    Tools {
        #[command(subcommand)]
        action: ToolsAction,
    },
    Fault {
        #[command(subcommand)]
        action: FaultAction,
    },
    Repair {
        #[command(subcommand)]
        action: RepairAction,
    },
    Mobile {
        #[command(subcommand)]
        action: MobileAction,
    },
    Autonew {
        provider: String,
    },
    CtxTransfer {
        #[arg(short = 'n', long, default_value_t = 3)]
        last: usize,
        #[arg(long = "from", value_name = "SOURCE")]
        source_provider: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        send: bool,
        #[arg(short = 'd', long)]
        dry_run: bool,
        #[arg(short = 'o', long)]
        output: Option<String>,
        #[arg(long)]
        session_path: Option<String>,
        #[arg(long, default_value_t = 8000)]
        max_tokens: usize,
        #[arg(short = 'f', long, default_value = "markdown")]
        format: String,
        #[arg(short = 'q', long)]
        quiet: bool,
        #[arg(short = 's', long)]
        save: bool,
        #[arg(long)]
        no_save: bool,
        #[arg(long)]
        detailed: bool,
    },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MaintenanceAction {
    Status,
    Tick,
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum ConfigAction {
    Validate,
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum RolesAction {
    List,
    Install { path: String },
    Update { path: String },
    Add { spec: String },
    Sync { path: Option<String> },
    Doctor { path: String },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum ToolsAction {
    Doctor { tool: String },
    Install { tool: String },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum FaultAction {
    List,
    Arm {
        agent_name: String,
        task_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long)]
        error: Option<String>,
    },
    Clear {
        target: String,
    },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum RepairAction {
    Ack {
        target: String,
        event_id: Option<String>,
    },
    Retry {
        target: String,
    },
    Resubmit {
        target: String,
    },
}

#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MobileAction {
    Serve {
        #[arg(long, default_value = "127.0.0.1:8787")]
        listen: String,
        #[arg(long = "public-url")]
        public_url: Option<String>,
        #[arg(long = "route-provider", default_value = "lan")]
        route_provider: String,
    },
    Devices,
    Revoke {
        device_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ParsedCommand {
    Start(ParsedStart),
    Ask(ParsedAsk),
    Wait(ParsedWait),
    WaitReplies(crate::models::ParsedWaitCommand),
    Watch(ParsedWatch),
    Ps(ParsedPs),
    Ping(ParsedPing),
    Cancel(ParsedCancel),
    Clear(ParsedClear),
    Cleanup(ParsedCleanup),
    Kill(ParsedKill),
    Pend(ParsedPend),
    Queue(ParsedQueue),
    Trace(ParsedTrace),
    Resubmit(ParsedResubmit),
    Retry(ParsedRetry),
    Inbox(ParsedInbox),
    Ack(ParsedAck),
    Logs(ParsedLogs),
    Maintenance(ParsedMaintenance),
    Doctor(ParsedDoctor),
    ConfigValidate(ParsedConfigValidate),
    Tools(ParsedTools),
    Roles(ParsedRoles),
    Fault(ParsedFault),
    Repair(ParsedRepair),
    Mobile(ParsedMobile),
    Reload(ParsedReload),
    Restart(ParsedRestart),
    Status(ParsedStatus),
    Stop(ParsedStop),
    StopAll(ParsedStopAll),
    Attach(ParsedAttach),
    Shutdown(ParsedShutdown),
    ProjectView(ParsedProjectView),
    Autonew(ParsedAutonew),
    CtxTransfer(ParsedCtxTransfer),
    Version,
    Update(ParsedUpdate),
    Uninstall(ParsedUninstall),
    Reinstall(ParsedReinstall),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStart {
    pub project: Option<String>,
    pub agent_names: Vec<String>,
    pub restore: bool,
    pub auto_permission: bool,
    pub reset_context: bool,
}

impl Default for ParsedStart {
    fn default() -> Self {
        Self {
            project: None,
            agent_names: vec![],
            restore: true,
            auto_permission: true,
            reset_context: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAsk {
    pub project: Option<String>,
    pub target: String,
    pub sender: Option<String>,
    pub message: String,
    pub task_id: Option<String>,
    pub compact: bool,
    pub silence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedWait {
    pub project: Option<String>,
    pub target: String,
    pub quorum: Option<usize>,
    pub timeout_s: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedWatch {
    pub project: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPs {
    pub project: Option<String>,
    pub alive_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPing {
    pub project: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCancel {
    pub project: Option<String>,
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedClear {
    pub project: Option<String>,
    pub agent_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCleanup {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedKill {
    pub project: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPend {
    pub project: Option<String>,
    pub target: String,
    pub count: Option<usize>,
    pub watch: bool,
    pub inbox: bool,
    pub queue: bool,
    pub detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQueue {
    pub project: Option<String>,
    pub target: String,
    pub detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTrace {
    pub project: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedResubmit {
    pub project: Option<String>,
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRetry {
    pub project: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedInbox {
    pub project: Option<String>,
    pub agent_name: String,
    pub detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAck {
    pub project: Option<String>,
    pub agent_name: String,
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedLogs {
    pub project: Option<String>,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMaintenance {
    pub project: Option<String>,
    pub action: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDoctor {
    pub project: Option<String>,
    pub bundle: bool,
    pub output_path: Option<String>,
    pub storage: bool,
    pub json_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedConfigValidate {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTools {
    pub project: Option<String>,
    pub action: ToolsAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRoles {
    pub project: Option<String>,
    pub action: RolesAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFault {
    pub project: Option<String>,
    pub action: FaultAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRepair {
    pub project: Option<String>,
    pub action: RepairAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMobile {
    pub project: Option<String>,
    pub action: String,
    pub listen: Option<String>,
    pub public_url: Option<String>,
    pub route_provider: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReload {
    pub project: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRestart {
    pub project: Option<String>,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStatus {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStop {
    pub project: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStopAll {
    pub project: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAttach {
    pub project: Option<String>,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedShutdown {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedProjectView {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUpdate {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUninstall {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedReinstall {
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAutonew {
    pub project: Option<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCtxTransfer {
    pub project: Option<String>,
    pub last: usize,
    pub source_provider: String,
    pub agent_name: Option<String>,
    pub send: bool,
    pub dry_run: bool,
    pub output: Option<String>,
    pub session_path: Option<String>,
    pub max_tokens: usize,
    pub format: String,
    pub quiet: bool,
    pub save: bool,
    pub no_save: bool,
    pub detailed: bool,
}
