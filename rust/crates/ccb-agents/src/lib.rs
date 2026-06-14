use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("role error: {0}")]
    Role(String),
    #[error("workspace error: {0}")]
    Workspace(String),
    #[error("storage error: {0}")]
    Storage(#[from] ccb_storage::StorageError),
    #[error("store error: {0}")]
    Store(#[from] crate::store::StoreError),
    #[error("role manifest error: {0}")]
    RoleManifest(#[from] crate::rolepacks::RoleManifestError),
    #[error("provider profiles error: {0}")]
    ProviderProfiles(#[from] ccb_provider_profiles::ProfilesError),
    #[error("provider core error: {0}")]
    ProviderCore(#[from] ccb_provider_core::error::ProviderCoreError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("toml serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("layout parse error: {0}")]
    LayoutParse(#[from] crate::layout::LayoutParseError),
}

pub type Result<T> = std::result::Result<T, AgentError>;

pub mod agent_roles_manager;
pub mod config;
pub mod config_identity;
pub mod layout;
pub mod models;
pub mod policy;
pub mod role_aliases;
pub mod rolepacks;
pub mod roles;
pub mod runtime_binding;
pub mod store;
pub mod workspace;
pub mod agent_api;
pub mod agent_role_adapter;
pub mod agent_specs;
pub mod api;
pub mod bootstrap;
pub mod common;
pub mod compact;
pub mod config_loader;
pub mod defaults;
pub mod documents;
pub mod enums;
pub mod expectations;
pub mod helpers;
pub mod io;
pub mod layout_plan;
pub mod maintenance;
pub mod manifest;
pub mod names;
pub mod nodes;
pub mod ops;
pub mod parser;
pub mod parsing;
pub mod paths;
pub mod project;
pub mod projection;
pub mod provider_profiles;
pub mod rendering;
pub mod restore;
pub mod role_lookup;
pub mod runtime_lookup;
pub mod serialization;
pub mod service;
pub mod sources;
pub mod spec;
pub mod topology;
pub mod validation;
