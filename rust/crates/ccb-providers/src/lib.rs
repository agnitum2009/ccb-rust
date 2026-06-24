pub mod claude;
pub mod codex;
pub mod deepseek;
pub mod execution;
pub mod kimi;
pub mod mimo;
pub mod model_shortcuts;
pub mod native_cli_support;
pub mod opencode;
pub mod pane_log_support;
pub mod pane_quiet_support;
pub mod providers;
pub mod runtime;

pub const TEST_DOUBLE_PROVIDER_NAMES: &[&str] = providers::fake::TEST_DOUBLE_PROVIDER_NAMES;

/// Build a registry containing the default provider execution adapters.
pub fn build_default_execution_registry() -> execution::ProviderExecutionRegistry {
    let mut registry = execution::ProviderExecutionRegistry::new();
    registry.register(Box::new(providers::claude::ClaudeExecutionAdapter));
    registry.register(Box::new(providers::codex::CodexExecutionAdapter));
    registry.register(Box::new(providers::gemini::GeminiExecutionAdapter));
    registry.register(Box::new(providers::opencode::OpenCodeExecutionAdapter));
    registry.register(Box::new(providers::droid::DroidExecutionAdapter));
    registry.register(Box::new(providers::agy::AgyExecutionAdapter));
    registry.register(Box::new(providers::copilot::build_execution_adapter()));
    registry.register(Box::new(providers::codebuddy::build_execution_adapter()));
    registry.register(Box::new(providers::qwen::build_execution_adapter()));
    registry.register(Box::new(providers::kimi::KimiExecutionAdapter));
    registry.register(Box::new(providers::deepseek::DeepSeekExecutionAdapter));
    registry.register(Box::new(providers::mimo::MimoExecutionAdapter));
    registry.register(Box::new(providers::cursor::build_execution_adapter()));
    registry.register(Box::new(providers::crush::build_execution_adapter()));
    registry.register(Box::new(providers::kiro::build_execution_adapter()));
    registry.register(Box::new(providers::pi::build_execution_adapter()));
    for adapter in providers::fake::execution_adapters() {
        registry.register(Box::new(adapter));
    }
    registry
}

/// Build a registry of provider backends (manifests only or with minimal adapters).
pub fn build_default_backend_registry() -> ccb_provider_core::registry::ProviderBackendRegistry {
    let mut registry = ccb_provider_core::registry::ProviderBackendRegistry::new();
    registry.register(providers::claude::backend());
    registry.register(providers::codex::backend());
    registry.register(providers::gemini::backend());
    registry.register(providers::opencode::backend());
    registry.register(providers::droid::backend());
    registry.register(providers::agy::backend());
    registry.register(providers::copilot::backend());
    registry.register(providers::codebuddy::backend());
    registry.register(providers::qwen::backend());
    registry.register(providers::kimi::backend());
    registry.register(providers::deepseek::backend());
    registry.register(providers::mimo::backend());
    registry.register(providers::cursor::backend());
    registry.register(providers::crush::backend());
    registry.register(providers::kiro::backend());
    registry.register(providers::pi::backend());
    for backend in providers::fake::backends() {
        registry.register(backend);
    }
    registry
}

pub mod active;
pub mod active_jobs;
pub mod active_runtime;
pub mod api_errors;
pub mod asking;
pub mod assistant;
pub mod assistant_events;
pub mod assistant_events_runtime;
pub mod auto_transfer;
pub mod base;
pub mod base_url;
pub mod binary_cache;
pub mod binding;
pub mod binding_runtime;
pub mod binding_update;
pub mod bridge;
pub mod cache;
pub mod cancel;
pub mod cancel_tracking;
pub mod candidates;
pub mod capabilities;
pub mod checking;
pub mod cli;
pub mod comm;
pub mod command;
pub mod committer;
pub mod communicator;
pub mod communicator_facade;
pub mod communicator_health;
pub mod communicator_io;
pub mod communicator_state;
pub mod content;
pub mod context;
pub mod conversation;
pub mod conversation_views;
pub mod conversations;
pub mod db;
pub mod debug;
pub mod decode;
pub mod diagnostics;
pub mod discovery;
pub mod encode;
pub mod entries;
pub mod entries_runtime;
pub mod env;
pub mod errors;
pub mod event_reading;
pub mod events;
pub mod exports;
pub mod extract;
pub mod extraction;
pub mod facade;
pub mod facade_monitoring;
pub mod facade_sessions;
pub mod facade_state;
pub mod facade_status;
pub mod facade_watchers;
pub mod fields;
pub mod fields_runtime;
pub mod files;
pub mod finalization;
pub mod follow_policy;
pub mod git;
pub mod global_logs;
pub mod helper_cleanup;
pub mod helper_manifest;
pub mod helpers;
pub mod history;
pub mod history_transfer;
pub mod history_transfer_service;
pub mod home;
pub mod home_layout;
pub mod hook;
pub mod hook_payload;
pub mod hook_results;
pub mod hook_results_runtime;
pub mod hook_service;
pub mod id_lookup;
pub mod incremental_io;
pub mod indexing;
pub mod items;
pub mod json_io;
pub mod latest;
pub mod lifecycle;
pub mod lifecycle_common;
pub mod lifecycle_recovery;
pub mod lifecycle_runtime;
pub mod live_identity;
pub mod loading;
pub mod log_cursor;
pub mod log_entries;
pub mod log_meta;
pub mod log_reader_facade;
pub mod logging;
pub mod lookup;
pub mod loop_helpers;
pub mod manifest;
pub mod matching;
pub mod membership;
pub mod message_cancel;
pub mod message_content;
pub mod message_reader;
pub mod messages;
pub mod messaging;
pub mod meta;
pub mod monitoring;
pub mod mutation;
pub mod normalization;
pub mod options;
pub mod overlay;
pub mod parsing;
pub mod pathing;
pub mod patterns;
pub mod payload_decision;
pub mod payload_materialize;
pub mod payloads;
pub mod pending;
pub mod poll;
pub mod polling_detection;
pub mod polling_loop;
pub mod polling_runtime;
pub mod project_binding;
pub mod project_binding_service;
pub mod project_hash;
pub mod project_id;
pub mod project_logs;
pub mod project_scope;
pub mod protocol_runtime;
pub mod reader_state;
pub mod reader_support;
pub mod readiness;
pub mod reply;
pub mod reply_logic;
pub mod reply_polling;
pub mod resolution;
pub mod resolver;
pub mod restore_helpers;
pub mod result;
pub mod resume;
pub mod rewriting;
pub mod roots;
pub mod runtime_artifacts;
pub mod runtime_io;
pub mod runtime_restore;
pub mod runtime_state;
pub mod scan;
pub mod scanning;
pub mod script_modes;
pub mod scripts;
pub mod selection;
pub mod serialization;
pub mod service_runtime_flow;
pub mod service_runtime_session;
pub mod service_state;
pub mod session_authority;
pub mod session_check;
pub mod session_content;
pub mod session_files;
pub mod session_ids;
pub mod session_index_runtime;
pub mod session_lookup;
pub mod session_paths;
pub mod session_runtime;
pub mod session_selection;
pub mod session_start;
pub mod session_updates;
pub mod session_updates_runtime;
pub mod sessions_index;
pub mod settings;
pub mod setup;
pub mod singleton;
pub mod start;
pub mod start_cmd;
pub mod state;
pub mod state_capture;
pub mod state_machine;
pub mod state_models;
pub mod storage_reader;
pub mod stream;
pub mod structured;
pub mod subagents;
pub mod submissions;
pub mod system_events;
pub mod tail;
pub mod tail_runtime;
pub mod terminal;
pub mod terminal_events;
pub mod terminal_events_runtime;
pub mod timeline;
pub mod turns;
pub mod user_events;
pub mod waiting;
pub mod watch;
pub mod watchdog;
pub mod watchdog_facade;
pub mod watchers;
pub mod workspace_preparation;

pub mod droid;
pub mod qwen;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_execution_registry() {
        let registry = build_default_execution_registry();
        assert!(registry.get("claude").is_some());
        assert!(registry.get("codex").is_some());
        assert!(registry.get("gemini").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("droid").is_some());
        assert!(registry.get("agy").is_some());
        assert!(registry.get("copilot").is_some());
        assert!(registry.get("codebuddy").is_some());
        assert!(registry.get("qwen").is_some());
        assert!(registry.get("kimi").is_some());
        assert!(registry.get("deepseek").is_some());
        assert!(registry.get("mimo").is_some());
        assert!(registry.get("cursor").is_some());
        assert!(registry.get("crush").is_some());
        assert!(registry.get("kiro").is_some());
        assert!(registry.get("pi").is_some());
        for name in providers::fake::TEST_DOUBLE_PROVIDER_NAMES {
            assert!(
                registry.get(name).is_some(),
                "execution registry missing {name}"
            );
        }
    }

    #[test]
    fn test_default_backend_registry() {
        let registry = build_default_backend_registry();
        assert!(registry.get("claude").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("qwen").is_some());
        assert!(registry.get("kimi").is_some());
        assert!(registry.get("deepseek").is_some());
        assert!(registry.get("mimo").is_some());
        assert!(registry.get("cursor").is_some());
        assert!(registry.get("crush").is_some());
        assert!(registry.get("kiro").is_some());
        assert!(registry.get("pi").is_some());
        for name in providers::fake::TEST_DOUBLE_PROVIDER_NAMES {
            assert!(
                registry.get(name).is_some(),
                "backend registry missing {name}"
            );
        }
    }
}
