//! Mirrors Python `test/test_v2_ask_service.py` submit/guidance/sender subset.

use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::ParsedCommand;
use ccb_cli::models_mailbox::ParsedAskCommand;
use ccb_cli::services::ask::{message_with_reply_guidance, resolve_ask_sender, submit_ask_with};
use ccb_cli::services::ask_runtime::submission::SubmitClient;
use ccb_daemon::models::api_models::messages::MessageEnvelope;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Serialize tests that mutate process-global env vars.
static ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn make_context(tmp: &tempfile::TempDir) -> CliContext {
    make_context_with_config(tmp, "cmd; agent1:codex, agent2:claude\n")
}

fn make_context_with_config(tmp: &tempfile::TempDir, config_text: &str) -> CliContext {
    let project_root = tmp.path();
    std::fs::create_dir_all(project_root.join(".ccbr")).unwrap();
    std::fs::write(project_root.join(".ccbr/ccbr.config"), config_text).unwrap();
    CliContextBuilder::new(ParsedCommand::Ask(ParsedAskCommand::new(
        None,
        "agent1".into(),
        None,
        "hello".into(),
    )))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

fn make_command(target: &str, message: &str) -> ParsedAskCommand {
    ParsedAskCommand::new(None, target.into(), None, message.into())
}

#[derive(Clone)]
struct FakeClient {
    captured: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    response: serde_json::Value,
}

impl SubmitClient for FakeClient {
    fn submit(&self, envelope: &MessageEnvelope) -> anyhow::Result<serde_json::Value> {
        let mut cap = self.captured.lock().unwrap();
        cap.insert("project_id".into(), json!(envelope.project_id.clone()));
        cap.insert("to_agent".into(), json!(envelope.to_agent.clone()));
        cap.insert("from_actor".into(), json!(envelope.from_actor.clone()));
        cap.insert("body".into(), json!(envelope.body.clone()));
        cap.insert(
            "body_artifact".into(),
            envelope
                .body_artifact
                .clone()
                .unwrap_or(serde_json::Value::Null),
        );
        cap.insert("reply_to".into(), json!(envelope.reply_to.clone()));
        cap.insert("message_type".into(), json!(envelope.message_type.clone()));
        cap.insert(
            "delivery_scope".into(),
            json!(format!("{:?}", envelope.delivery_scope).to_lowercase()),
        );
        cap.insert(
            "silence_on_success".into(),
            json!(envelope.silence_on_success),
        );
        cap.insert("route_options".into(), envelope.route_options.clone());
        Ok(self.response.clone())
    }
}

#[test]
fn test_submit_ask_rejects_unknown_target() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let command = make_command("agent9", "hello");
    let err = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: Arc::new(Mutex::new(HashMap::new())),
                response: json!({}),
            };
            request_fn(&client)
        },
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "unknown agent: agent9");
}

#[test]
fn test_submit_ask_resolves_unique_role_id_alias() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context_with_config(
        &tmp,
        "version = 2\ndefault_agents = [\"agent1\", \"archi\"]\n[agents]\nagent1 = { provider = \"codex\" }\narchi = { provider = \"codex\", role = \"agentroles.archi\" }\n",
    );
    let command = make_command("agentroles.archi", "review");
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    let summary = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "archi",
                    "target_name": "archi",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    assert_eq!(cap.get("to_agent").unwrap().as_str().unwrap(), "archi");
    assert_eq!(summary.jobs[0]["agent_name"], "archi");
}

#[test]
fn test_submit_ask_maps_broadcast_payload_and_submission() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let mut command = make_command("all", "ship it");
    command.reply_to = Some("msg_1".into());
    command.mode = Some("notify".into());
    command.silence = true;
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    let summary = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "submission_id": "sub_1",
                    "jobs": [
                        {"job_id": "job_1", "agent_name": "agent1", "target_name": "agent1", "status": "accepted"},
                        {"job_id": "job_2", "agent_name": "agent2", "target_name": "agent2", "status": "accepted"},
                    ],
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    assert_eq!(summary.project_id, context.project.project_id);
    assert_eq!(summary.submission_id.as_deref(), Some("sub_1"));
    let job_ids: Vec<String> = summary
        .jobs
        .iter()
        .map(|j| j["job_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(job_ids, vec!["job_1", "job_2"]);

    let cap = captured.lock().unwrap();
    assert_eq!(
        cap.get("project_id").unwrap().as_str().unwrap(),
        context.project.project_id
    );
    assert_eq!(cap.get("to_agent").unwrap().as_str().unwrap(), "all");
    assert_eq!(cap.get("from_actor").unwrap().as_str().unwrap(), "agent1");
    assert_eq!(cap.get("body").unwrap().as_str().unwrap(), "ship it");
    assert_eq!(cap.get("reply_to").unwrap().as_str().unwrap(), "msg_1");
    assert_eq!(cap.get("message_type").unwrap().as_str().unwrap(), "notify");
    assert_eq!(
        cap.get("delivery_scope").unwrap().as_str().unwrap(),
        "broadcast"
    );
    assert!(cap.get("silence_on_success").unwrap().as_bool().unwrap());
    assert_eq!(cap.get("route_options").unwrap(), &json!({}));
}

#[test]
fn test_submit_ask_maps_callback_route_options() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let mut command = make_command("agent2", "collect evidence");
    command.callback = true;
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "agent2",
                    "target_name": "agent2",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    assert_eq!(
        cap.get("route_options").unwrap(),
        &json!({"mode": "callback"})
    );
}

#[test]
fn test_submit_ask_spills_large_body_before_daemon_submit() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let large_message = format!("alpha-{}", "x".repeat(5000));
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    submit_ask_with(
        &context,
        &make_command("agent2", &large_message),
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "agent2",
                    "target_name": "agent2",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    let body = cap.get("body").unwrap().as_str().unwrap().to_string();
    let artifact = cap.get("body_artifact").unwrap();
    assert!(body.len() <= 4096);
    assert!(body.contains("larger than 4 KiB"));
    assert!(artifact.is_object());
    let artifact_path = std::path::PathBuf::from(artifact.get("path").unwrap().as_str().unwrap());
    assert!(artifact_path.exists());
    let artifact_text = std::fs::read_to_string(&artifact_path).unwrap();
    assert!(artifact_text.starts_with("alpha-"));
    assert!(artifact_text.contains("CCB reply guidance:"));
}

#[test]
fn test_submit_ask_forces_small_body_artifact() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let mut command = make_command("agent2", "short task");
    command.artifact_request = true;
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "agent2",
                    "target_name": "agent2",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    let body = cap.get("body").unwrap().as_str().unwrap();
    let artifact = cap.get("body_artifact").unwrap();
    assert!(body.contains("stored as an artifact by --artifact-request"));
    assert!(!body.contains("Preview:"));
    assert!(!body.contains("short task"));
    assert!(artifact.is_object());
    let artifact_path = std::path::PathBuf::from(artifact.get("path").unwrap().as_str().unwrap());
    assert!(artifact_path.exists());
    let artifact_text = std::fs::read_to_string(&artifact_path).unwrap();
    assert!(artifact_text.starts_with("short task"));
    assert!(artifact_text.contains("CCB reply guidance:"));
}

#[test]
fn test_message_with_reply_guidance_appends_compact_default() {
    let body = message_with_reply_guidance("review the diff", "ask", false, false);
    assert!(body.starts_with("review the diff\n\nCCB reply guidance:"));
    assert!(body.contains("Answer directly and concisely."));
}

#[test]
fn test_submit_ask_rejects_explicit_cmd_sender() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let mut command = make_command("agent1", "hello");
    command.sender = Some("cmd".into());
    let err = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, sender| sender.unwrap_or("agent1").into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: Arc::new(Mutex::new(HashMap::new())),
                response: json!({}),
            };
            request_fn(&client)
        },
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "unknown sender agent: cmd");
}

#[test]
fn test_resolve_ask_sender_defaults_to_user_for_project_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    for name in [
        "CCB_CALLER_ACTOR",
        "CCB_CALLER_RUNTIME_DIR",
        "CODEX_RUNTIME_DIR",
        "CCB_SESSION_ID",
    ] {
        std::env::remove_var(name);
    }
    assert_eq!(resolve_ask_sender(&context, None), "user");
}

#[test]
fn test_resolve_ask_sender_prefers_runtime_dir_actor() {
    let _guard = ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    for name in [
        "CCB_CALLER_ACTOR",
        "CCB_CALLER_RUNTIME_DIR",
        "CODEX_RUNTIME_DIR",
        "CCB_SESSION_ID",
    ] {
        std::env::remove_var(name);
    }
    let runtime_dir = tmp
        .path()
        .join(".ccbr/agents/agent1/provider-runtime/codex");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::env::set_var("CODEX_RUNTIME_DIR", runtime_dir.as_os_str());
    std::env::set_var("CCB_SESSION_ID", "legacy-session-without-actor");
    assert_eq!(resolve_ask_sender(&context, None), "agent1");
    std::env::remove_var("CODEX_RUNTIME_DIR");
    std::env::remove_var("CCB_SESSION_ID");
}

#[test]
fn test_resolve_ask_sender_prefers_relocated_runtime_dir_actor() {
    let _guard = ENV_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let before = make_context(&tmp);
    for name in [
        "CCB_CALLER_ACTOR",
        "CCB_CALLER_RUNTIME_DIR",
        "CODEX_RUNTIME_DIR",
        "CCB_SESSION_ID",
    ] {
        std::env::remove_var(name);
    }

    let relocated_root = tmp.path().join("state-root");
    std::fs::create_dir_all(&relocated_root).unwrap();
    let ref_value = serde_json::json!({
        "schema_version": 1,
        "record_type": "ccb_runtime_root_ref",
        "project_id": before.paths.project_id(),
        "runtime_state_root": relocated_root.to_string_lossy().to_string(),
        "created_at": "2026-05-07T00:00:00Z",
    });
    std::fs::write(
        before
            .paths
            .project_root
            .join(".ccbr/runtime-root-ref.json"),
        ref_value.to_string(),
    )
    .unwrap();

    let context = make_context(&tmp);
    assert_eq!(
        context.paths.runtime_state_root().as_str(),
        relocated_root.to_string_lossy().as_ref()
    );

    let runtime_dir = context
        .paths
        .agent_provider_runtime_dir("agent1", "codex")
        .into_std_path_buf();
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::env::set_var("CODEX_RUNTIME_DIR", runtime_dir.as_os_str());
    std::env::set_var("CCB_SESSION_ID", "legacy-session-without-actor");
    assert_eq!(resolve_ask_sender(&context, None), "agent1");
    std::env::remove_var("CODEX_RUNTIME_DIR");
    std::env::remove_var("CCB_SESSION_ID");
}

#[test]
fn test_submit_ask_resolves_legacy_role_id_alias() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context_with_config(
        &tmp,
        "version = 2\ndefault_agents = [\"agent1\", \"archi\"]\n[agents]\nagent1 = { provider = \"codex\" }\narchi = { provider = \"codex\", role = \"agentroles.archi\" }\n",
    );
    let command = make_command("ccb.archi", "review");
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    let summary = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "archi",
                    "target_name": "archi",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    assert_eq!(cap.get("to_agent").unwrap().as_str().unwrap(), "archi");
    assert_eq!(summary.jobs[0]["agent_name"], "archi");
}

#[test]
fn test_write_ask_output_appends_newline() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("reply.txt");
    ccb_cli::services::ask::write_ask_output(&path, "done").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "done\n");
}

#[test]
fn test_write_ask_output_preserves_existing_newline() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("reply.txt");
    ccb_cli::services::ask::write_ask_output(&path, "done\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "done\n");
}

#[test]
fn test_exit_code_for_ask_status() {
    use ccb_cli::services::ask::exit_code_for_ask_status;
    assert_eq!(exit_code_for_ask_status(Some("completed"), "done"), 0);
    assert_eq!(exit_code_for_ask_status(Some("incomplete"), "partial"), 2);
    assert_eq!(exit_code_for_ask_status(Some("failed"), ""), 1);
    assert_eq!(exit_code_for_ask_status(None, ""), 1);
}

#[test]
fn test_submit_ask_role_id_alias_requires_binding() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context_with_config(
        &tmp,
        "version = 2\ndefault_agents = [\"agent1\"]\n[agents]\nagent1 = { provider = \"codex\" }\n",
    );
    let command = make_command("agentroles.archi", "review");
    let err = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: Arc::new(Mutex::new(HashMap::new())),
                response: json!({}),
            };
            request_fn(&client)
        },
    )
    .unwrap_err();
    assert!(err
        .to_string()
        .contains("role agentroles.archi is not bound to any configured agent"));
}

#[test]
fn test_submit_ask_role_id_alias_rejects_multiple_bindings() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context_with_config(
        &tmp,
        "version = 2\ndefault_agents = [\"agent1\", \"agent2\"]\n[agents]\nagent1 = { provider = \"codex\", role = \"agentroles.archi\" }\nagent2 = { provider = \"codex\", role = \"agentroles.archi\" }\n",
    );
    let command = make_command("agentroles.archi", "review");
    let err = submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: Arc::new(Mutex::new(HashMap::new())),
                response: json!({}),
            };
            request_fn(&client)
        },
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("role agentroles.archi is bound to multiple agents"));
    assert!(msg.contains("agent1") && msg.contains("agent2"));
}

#[test]
fn test_submit_ask_maps_artifact_route_options() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let mut command = make_command("agent2", "collect evidence");
    command.artifact_request = true;
    command.artifact_reply = true;
    let captured = Arc::new(Mutex::new(HashMap::new()));
    let captured_clone = captured.clone();

    submit_ask_with(
        &context,
        &command,
        ccb_agents::config::load_project_config,
        |_ctx, _sender| "agent1".into(),
        |_ctx, _allow, request_fn| {
            let client = FakeClient {
                captured: captured_clone.clone(),
                response: json!({
                    "job_id": "job_1",
                    "agent_name": "agent2",
                    "target_name": "agent2",
                    "status": "accepted",
                }),
            };
            request_fn(&client)
        },
    )
    .unwrap();

    let cap = captured.lock().unwrap();
    assert_eq!(
        cap.get("route_options").unwrap(),
        &json!({"artifact_request": true, "artifact_reply": true})
    );
}

#[test]
fn test_message_with_reply_guidance_uses_silent_hint_for_silenced_asks() {
    let body = message_with_reply_guidance("run smoke test", "ask", false, true);
    assert!(body.contains("Silent-on-success requested."));
}

#[test]
fn test_ask_guidance_source_has_no_literal_chinese_characters() {
    let body = message_with_reply_guidance("review", "ask", false, false);
    assert!(!body.contains('\u{7b54}')); // 答
    assert!(!body.contains('\u{76f4}')); // 直
    assert!(!body.contains('\u{63a5}')); // 接
}
