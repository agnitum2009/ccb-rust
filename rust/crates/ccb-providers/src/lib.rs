pub mod execution;
pub mod claude;
pub mod deepseek;
pub mod kimi;
pub mod mimo;
pub mod model_shortcuts;
pub mod native_cli_support;
pub mod opencode;
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

pub mod watchers;
pub mod watchdog_facade;
pub mod watchdog;
pub mod watch;
pub mod waiting;
pub mod user_events;
pub mod turns;
pub mod timeline;
pub mod terminal_events_runtime;
pub mod terminal_events;
pub mod terminal;
pub mod tail_runtime;
pub mod tail;
pub mod system_events;
pub mod submissions;
pub mod subagents;
pub mod structured;
pub mod stream;
pub mod storage_reader;
pub mod state_models;
pub mod state_machine;
pub mod state_capture;
pub mod state;
pub mod start_cmd;
pub mod start;
pub mod singleton;
pub mod setup;
pub mod settings;
pub mod sessions_index;
pub mod session_updates_runtime;
pub mod session_updates;
pub mod session_start;
pub mod session_selection;
pub mod session_runtime;
pub mod session_paths;
pub mod session_lookup;
pub mod session_index_runtime;
pub mod session_ids;
pub mod session_files;
pub mod session_content;
pub mod session_check;
pub mod session_authority;
pub mod service_state;
pub mod service_runtime_session;
pub mod service_runtime_flow;
pub mod serialization;
pub mod selection;
pub mod scripts;
pub mod script_modes;
pub mod scanning;
pub mod scan;
pub mod runtime_state;
pub mod runtime_restore;
pub mod runtime_io;
pub mod runtime_artifacts;
pub mod roots;
pub mod rewriting;
pub mod resume;
pub mod result;
pub mod restore_helpers;
pub mod resolver;
pub mod resolution;
pub mod reply_polling;
pub mod reply_logic;
pub mod reply;
pub mod readiness;
pub mod reader_support;
pub mod reader_state;
pub mod protocol_runtime;
pub mod project_scope;
pub mod project_logs;
pub mod project_id;
pub mod project_hash;
pub mod project_binding_service;
pub mod project_binding;
pub mod polling_runtime;
pub mod polling_loop;
pub mod polling_detection;
pub mod poll;
pub mod pending;
pub mod payloads;
pub mod payload_materialize;
pub mod payload_decision;
pub mod patterns;
pub mod pathing;
pub mod parsing;
pub mod overlay;
pub mod options;
pub mod normalization;
pub mod mutation;
pub mod monitoring;
pub mod meta;
pub mod messaging;
pub mod messages;
pub mod message_reader;
pub mod message_content;
pub mod message_cancel;
pub mod membership;
pub mod matching;
pub mod manifest;
pub mod loop_helpers;
pub mod lookup;
pub mod logging;
pub mod log_reader_facade;
pub mod log_meta;
pub mod log_entries;
pub mod log_cursor;
pub mod loading;
pub mod live_identity;
pub mod lifecycle_runtime;
pub mod lifecycle_recovery;
pub mod lifecycle_common;
pub mod lifecycle;
pub mod latest;
pub mod json_io;
pub mod items;
pub mod indexing;
pub mod incremental_io;
pub mod id_lookup;
pub mod hook_service;
pub mod hook_results_runtime;
pub mod hook_results;
pub mod hook_payload;
pub mod hook;
pub mod home_layout;
pub mod home;
pub mod history_transfer_service;
pub mod history_transfer;
pub mod history;
pub mod helpers;
pub mod global_logs;
pub mod git;
pub mod follow_policy;
pub mod finalization;
pub mod files;
pub mod fields_runtime;
pub mod fields;
pub mod facade_watchers;
pub mod facade_status;
pub mod facade_state;
pub mod facade_sessions;
pub mod facade_monitoring;
pub mod facade;
pub mod extraction;
pub mod extract;
pub mod exports;
pub mod events;
pub mod event_reading;
pub mod errors;
pub mod env;
pub mod entries_runtime;
pub mod entries;
pub mod encode;
pub mod discovery;
pub mod diagnostics;
pub mod decode;
pub mod debug;
pub mod db;
pub mod conversations;
pub mod conversation_views;
pub mod conversation;
pub mod context;
pub mod content;
pub mod communicator_state;
pub mod communicator_io;
pub mod communicator_health;
pub mod communicator_facade;
pub mod communicator;
pub mod committer;
pub mod command;
pub mod comm;
pub mod cli;
pub mod checking;
pub mod capabilities;
pub mod candidates;
pub mod cancel_tracking;
pub mod cancel;
pub mod cache;
pub mod bridge;
pub mod binding_update;
pub mod binding_runtime;
pub mod binding;
pub mod binary_cache;
pub mod base_url;
pub mod base;
pub mod auto_transfer;
pub mod assistant_events_runtime;
pub mod assistant_events;
pub mod assistant;
pub mod asking;
pub mod api_errors;
pub mod active_jobs;
pub mod active;

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
