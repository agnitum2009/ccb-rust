pub mod adapters;
pub mod api_models;
pub mod app;
pub mod artifact_maintenance;
pub mod client_runtime;
pub mod fault_injection;
pub mod handlers;
pub mod health_runtime;
pub mod mobile_gateway;
pub mod models;
pub mod patch_validation_targets;
pub mod polling;
pub mod project_focus;
pub mod project_view;
pub mod provider_launcher;
pub mod reload_additive_agents;
pub mod reload_append_layout;
pub mod reload_apply_graph;
pub mod reload_apply_models;
pub mod reload_apply_namespace;
pub mod reload_apply_plan;
pub mod reload_apply_results;
pub mod reload_apply_runtime;
pub mod reload_apply_service;
pub mod reload_apply_stages;
pub mod reload_drain;
pub mod reload_handoff;
pub mod reload_patch;
pub mod reload_patch_additive_agents;
pub mod reload_patch_remove_agents;
pub mod reload_plan;
pub mod reload_runtime_mount;
pub mod reload_runtime_mount_models;
pub mod reload_runtime_mount_service;
pub mod reload_runtime_mount_start;
pub mod reload_runtime_mount_state;
pub mod reload_runtime_mount_validation;
pub mod reload_runtime_unload;
pub mod reload_transaction;
pub mod reload_transaction_context;
pub mod reload_transaction_models;
pub mod reload_transaction_preflight;
pub mod reload_transaction_publish;
pub mod reload_transaction_records;
pub mod reload_transaction_results;
pub mod reload_transaction_service;
pub mod reload_transaction_signature;
pub mod reload_transaction_signature_rollback;
pub mod services;
pub mod socket_client;
pub mod socket_client_runtime;
pub mod socket_server;
pub mod start_flow;
pub mod start_flow_runtime;
pub mod start_preparation;
pub mod start_runtime;
#[path = "start_runtime/layout.rs"]
pub mod start_runtime_layout;
pub mod stop_flow;
pub mod supervision;
pub mod system;
pub mod terminal_adapter;
pub mod tick;
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
    Storage(#[from] ccbr_storage::StorageError),
    #[error("config error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, DaemonError>;
