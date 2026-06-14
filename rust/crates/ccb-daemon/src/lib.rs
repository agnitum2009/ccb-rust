pub mod adapters;
pub mod app;
pub mod fault_injection;
pub mod handlers;
pub mod models;
pub mod project_focus;
pub mod project_view;
pub mod provider_launcher;
pub mod reload_additive_agents;
pub mod reload_patch_remove_agents;
pub mod reload_plan;
pub mod reload_transaction;
pub mod services;
pub mod socket_server;
pub mod start_flow;
pub mod stop_flow;
pub mod supervision;
pub mod terminal_adapter;

pub use app::CcbdApp;
pub use socket_server::SocketServer;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),
    #[error("config error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, DaemonError>;
