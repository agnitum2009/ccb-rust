//! Mirrors Python `test/test_v2_diagnostics_bundle.py`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use ccbr_cli::context::{CliContext, CliContextBuilder};
use ccbr_cli::models::ParsedCommand;
use ccbr_cli::models_start::ParsedDoctorCommand;
use ccbr_cli::services::diagnostics_runtime::bundle::{
    export_diagnostic_bundle, export_diagnostic_bundle_with_storage,
};
use flate2::read::GzDecoder;
use serde_json::Value;

fn build_context(project_root: PathBuf) -> CliContext {
    CliContextBuilder::new(ParsedCommand::Doctor(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    }))
    .cwd(project_root)
    .build()
    .unwrap()
}

fn read_tar_json(bundle_path: &Path, member_name: impl AsRef<str>) -> Value {
    let member = member_name.as_ref();
    let file = fs::File::open(bundle_path).unwrap();
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().to_string();
        if path == member {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).unwrap();
            return serde_json::from_str(&buf).unwrap();
        }
    }
    panic!("member {} not found in {}", member, bundle_path.display());
}

fn archive_members(bundle_path: &Path) -> Vec<String> {
    let file = fs::File::open(bundle_path).unwrap();
    let dec = GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive
        .entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
        .collect()
}

#[test]
fn test_export_diagnostic_bundle_collects_reports_and_log_tails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();

    let context = build_context(project_root.clone());

    fs::create_dir_all(context.paths.ccbrd_dir().as_std_path()).unwrap();
    fs::write(
        context.paths.ccbrd_state_path().as_std_path(),
        "{\"record_type\":\"ccbrd_project_namespace_state\"}\n",
    )
    .unwrap();
    fs::write(
        context.paths.ccbrd_start_policy_path().as_std_path(),
        "{\"record_type\":\"ccbrd_start_policy\"}\n",
    )
    .unwrap();
    fs::write(
        context.paths.ccbrd_lifecycle_log_path().as_std_path(),
        "{\"record_type\":\"ccbrd_project_namespace_event\"}\n",
    )
    .unwrap();

    let heartbeat_path = context
        .paths
        .heartbeat_subject_path("job_progress", "job_1")
        .unwrap();
    fs::create_dir_all(heartbeat_path.parent().unwrap()).unwrap();
    fs::write(
        heartbeat_path.as_std_path(),
        "{\"record_type\":\"heartbeat_state\"}\n",
    )
    .unwrap();

    let maintenance_status_path = context.paths.ccbrd_maintenance_heartbeat_status_path();
    fs::create_dir_all(maintenance_status_path.parent().unwrap()).unwrap();
    fs::write(
        maintenance_status_path.as_std_path(),
        "{\"record_type\":\"maintenance_heartbeat_status\"}\n",
    )
    .unwrap();
    fs::write(
        context
            .paths
            .ccbrd_maintenance_heartbeat_activations_path()
            .as_std_path(),
        "{\"record_type\":\"maintenance_heartbeat_activation\"}\n",
    )
    .unwrap();

    let text_artifact_path = context
        .paths
        .ccbrd_text_artifacts_dir()
        .join("ask-request")
        .join("large.txt");
    fs::create_dir_all(text_artifact_path.parent().unwrap().as_std_path()).unwrap();
    fs::write(text_artifact_path.as_std_path(), "large ask body\n").unwrap();

    fs::write(
        context.paths.ccbrd_startup_report_path().as_std_path(),
        "{\"broken\":false}\n",
    )
    .unwrap();

    let log_lines: String = (0..400)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        context
            .paths
            .ccbrd_dir()
            .join("ccbrd.stdout.log")
            .as_std_path(),
        log_lines,
    )
    .unwrap();

    let runtime_path = context.paths.agent_runtime_path("demo");
    fs::create_dir_all(runtime_path.parent().unwrap().as_std_path()).unwrap();
    fs::write(
        runtime_path.as_std_path(),
        serde_json::to_string(&serde_json::json!({
            "schema_version": 2,
            "record_type": "agent_runtime",
            "agent_name": "demo",
            "state": "idle",
            "pid": 101,
            "started_at": "2026-04-03T00:00:00Z",
            "last_seen_at": "2026-04-03T00:00:01Z",
            "runtime_ref": "tmux:%1",
            "session_ref": None::<&str>,
            "workspace_path": context.paths.workspace_path("demo", None).as_str(),
            "project_id": context.project.project_id,
            "backend_type": "tmux",
            "queue_depth": 0,
            "socket_path": None::<&str>,
            "health": "healthy",
        }))
        .unwrap()
            + "\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let entries = manifest["entries"].as_array().unwrap();

    assert!(bundle_path.exists());
    assert!(summary.file_count >= 4);
    assert!(summary.truncated_count >= 1);
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/state.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/start-policy.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/lifecycle.jsonl"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/heartbeats/job_progress/job_1.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/maintenance-heartbeat/status.json"));
    assert!(entries.iter().any(
        |e| e["archive_path"] == "project/.ccbr/ccbrd/maintenance-heartbeat/activations.jsonl"
    ));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/artifacts/text/ask-request/large.txt"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/startup-report.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/ccbrd.stdout.log"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/agents/demo/runtime.json"));
}

#[test]
fn test_export_diagnostic_bundle_includes_relocated_runtime_state_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-relocated");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();

    let context = build_context(project_root.clone());

    let relocated_root = tmp.path().join("state-root");
    fs::write(
        context.paths.runtime_root_ref_path().as_std_path(),
        format!(
            "{{\"schema_version\":1,\"record_type\":\"ccbr_runtime_root_ref\",\"project_id\":\"{}\",\"runtime_state_root\":\"{}\",\"created_at\":\"2026-05-07T00:00:00Z\"}}\n",
            context.project.project_id,
            relocated_root.display()
        ),
    )
    .unwrap();

    context
        .paths
        .ensure_runtime_state_root(Some("2026-05-07T00:00:00Z"))
        .unwrap();
    fs::create_dir_all(
        context
            .paths
            .ccbrd_state_path()
            .parent()
            .unwrap()
            .as_std_path(),
    )
    .unwrap();
    fs::write(
        context.paths.ccbrd_state_path().as_std_path(),
        "{\"record_type\":\"ccbrd_project_namespace_state\"}\n",
    )
    .unwrap();
    fs::write(
        context.paths.ccbrd_start_policy_path().as_std_path(),
        "{\"record_type\":\"ccbrd_start_policy\"}\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let entries = manifest["entries"].as_array().unwrap();

    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/runtime-root-ref.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/runtime-root.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/state.json"));
    assert!(entries
        .iter()
        .any(|e| e["archive_path"] == "project/.ccbr/ccbrd/start-policy.json"));
    let marker_source = context.paths.runtime_root_marker_path().to_string();
    assert!(entries.iter().any(|e| e["source_path"] == marker_source));
}

#[test]
fn test_export_diagnostic_bundle_survives_corrupt_runtime_and_report_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-corrupt");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();

    let context = build_context(project_root.clone());

    fs::create_dir_all(
        context
            .paths
            .ccbrd_startup_report_path()
            .parent()
            .unwrap()
            .as_std_path(),
    )
    .unwrap();
    fs::write(
        context.paths.ccbrd_startup_report_path().as_std_path(),
        "{this is not json}\n",
    )
    .unwrap();
    fs::create_dir_all(
        context
            .paths
            .agent_runtime_path("demo")
            .parent()
            .unwrap()
            .as_std_path(),
    )
    .unwrap();
    fs::write(
        context.paths.agent_runtime_path("demo").as_std_path(),
        "{this is also not json}\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let entries = manifest["entries"].as_array().unwrap();

    assert!(bundle_path.exists());
    assert!(entries.iter().any(|e| {
        e["archive_path"] == "project/.ccbr/ccbrd/startup-report.json" && e["status"] == "included"
    }));
    assert!(entries.iter().any(|e| {
        e["archive_path"] == "project/.ccbr/agents/demo/runtime.json" && e["status"] == "included"
    }));
}

#[test]
fn test_export_diagnostic_bundle_includes_provider_state_and_excludes_auth() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-provider-state");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:codex\n").unwrap();

    let context = build_context(project_root.clone());

    let provider_state_dir = context.paths.agent_provider_state_dir("demo", "codex");
    let session_log = provider_state_dir
        .join("home")
        .join("sessions")
        .join("2026")
        .join("04")
        .join("19")
        .join("rollout-demo-session.jsonl");
    fs::create_dir_all(session_log.parent().unwrap().as_std_path()).unwrap();
    fs::write(session_log.as_std_path(), "{\"type\":\"session_meta\"}\n").unwrap();

    let isolated_home = provider_state_dir.join("home");
    fs::create_dir_all(isolated_home.as_std_path()).unwrap();
    fs::write(
        isolated_home.join("config.toml").as_std_path(),
        "[model]\nname=\"gpt-5\"\n",
    )
    .unwrap();
    fs::write(
        isolated_home.join("auth.json").as_std_path(),
        "{\"OPENAI_API_KEY\":\"secret\"}\n",
    )
    .unwrap();

    let plugin_manifest = isolated_home
        .join(".tmp")
        .join("plugins")
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    fs::create_dir_all(plugin_manifest.parent().unwrap().as_std_path()).unwrap();
    fs::write(plugin_manifest.as_std_path(), "{\"name\":\"market\"}\n").unwrap();
    fs::write(
        isolated_home.join(".tmp").join("plugins.sha").as_std_path(),
        "plugin-sha\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let storage_summary = read_tar_json(
        &bundle_path,
        format!("{}/generated/storage-summary.json", summary.bundle_id),
    );
    let members = archive_members(&bundle_path);
    let entries = manifest["entries"].as_array().unwrap();

    assert!(entries.iter().any(|e| {
        e["archive_path"] == "project/.ccbr/agents/demo/provider-state/codex/home/sessions/2026/04/19/rollout-demo-session.jsonl"
            && e["status"] == "included"
    }));
    assert!(entries.iter().any(|e| {
        e["archive_path"] == "project/.ccbr/agents/demo/provider-state/codex/home/config.toml"
            && e["status"] == "included"
    }));
    assert!(entries
        .iter()
        .all(|e| e["archive_path"]
            != "project/.ccbr/agents/demo/provider-state/codex/home/auth.json"));
    assert!(entries.iter().all(|e| e["archive_path"]
        != "project/.ccbr/agents/demo/provider-state/codex/home/.tmp/plugins/.agents/plugins/marketplace.json"));

    let storage_entries = storage_summary["entries"].as_array().unwrap();
    assert!(storage_entries.iter().any(|e| {
        e["relative_path"]
            == "agents/demo/provider-state/codex/home/.tmp/plugins/.agents/plugins/marketplace.json"
            && e["storage_class"] == "startup_authority_bundle"
    }));

    assert!(members.contains(&format!(
        "{}/generated/storage-summary.json",
        summary.bundle_id
    )));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/codex/home/auth.json",
        summary.bundle_id
    )));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/codex/home/.tmp/plugins/.agents/plugins/marketplace.json",
        summary.bundle_id
    )));
}

#[test]
fn test_export_diagnostic_bundle_hard_excludes_provider_cache_when_storage_summary_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-provider-state-storage-error");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(
        ccbr_dir.join("ccbr.config"),
        "codexer:codex\nclauder:claude\ngem:gemini\n",
    )
    .unwrap();

    let context = build_context(project_root.clone());

    let codex_home = context
        .paths
        .agent_provider_state_dir("codexer", "codex")
        .join("home");
    fs::create_dir_all(codex_home.as_std_path()).unwrap();
    fs::write(
        codex_home.join("config.toml").as_std_path(),
        "model = \"gpt-5\"\n",
    )
    .unwrap();
    let plugin_manifest = codex_home
        .join(".tmp")
        .join("plugins")
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    fs::create_dir_all(plugin_manifest.parent().unwrap().as_std_path()).unwrap();
    fs::write(plugin_manifest.as_std_path(), "{\"name\":\"market\"}\n").unwrap();
    fs::write(
        codex_home.join(".tmp").join("plugins.sha").as_std_path(),
        "plugin-sha\n",
    )
    .unwrap();

    let outside = tmp.path().join("outside-provider-state");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("leaked.json"), "{\"secret\":\"outside\"}\n").unwrap();
    let _ = std::os::unix::fs::symlink(&outside, codex_home.join("linked-outside"));

    let claude_home = context
        .paths
        .agent_provider_state_dir("clauder", "claude")
        .join("home");
    let claude_version_manifest = claude_home
        .join(".local")
        .join("share")
        .join("claude")
        .join("versions")
        .join("2.1.137")
        .join("metadata.json");
    fs::create_dir_all(claude_version_manifest.parent().unwrap().as_std_path()).unwrap();
    fs::write(
        claude_version_manifest.as_std_path(),
        "{\"version\":\"2.1.137\"}\n",
    )
    .unwrap();

    let gemini_home = context
        .paths
        .agent_provider_state_dir("gem", "gemini")
        .join("home");
    let gemini_cache = gemini_home.join(".npm").join("_cacache").join("index.json");
    fs::create_dir_all(gemini_cache.parent().unwrap().as_std_path()).unwrap();
    fs::write(gemini_cache.as_std_path(), "{\"cache\":true}\n").unwrap();
    let gemini_node_gyp = gemini_home
        .join(".cache")
        .join("node-gyp")
        .join("config.json");
    fs::create_dir_all(gemini_node_gyp.parent().unwrap().as_std_path()).unwrap();
    fs::write(gemini_node_gyp.as_std_path(), "{\"cache\":true}\n").unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle_with_storage(&context, &command, |_paths| {
        Err("storage failed".into())
    })
    .unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let storage_summary = read_tar_json(
        &bundle_path,
        format!("{}/generated/storage-summary.json", summary.bundle_id),
    );
    let members = archive_members(&bundle_path);
    let entries = manifest["entries"].as_array().unwrap();

    assert_eq!(storage_summary["error"], "storage failed");
    assert!(entries.iter().any(|e| {
        e["archive_path"] == "project/.ccbr/agents/codexer/provider-state/codex/home/config.toml"
            && e["status"] == "included"
    }));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("/.tmp/plugins/")));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("/.local/share/claude/versions/")));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("/.npm/_cacache/")));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("/.cache/node-gyp/")));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("linked-outside")));
    assert!(members.iter().all(|m| !m.contains("/.tmp/plugins/")));
    assert!(members
        .iter()
        .all(|m| !m.contains("/.local/share/claude/versions/")));
    assert!(members.iter().all(|m| !m.contains("/.npm/_cacache/")));
    assert!(members.iter().all(|m| !m.contains("/.cache/node-gyp/")));
    assert!(members.iter().all(|m| !m.contains("linked-outside")));
}

#[test]
fn test_export_diagnostic_bundle_excludes_gemini_auth_artifacts() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-gemini-provider-state");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:gemini\n").unwrap();

    let context = build_context(project_root.clone());

    let provider_state_dir = context.paths.agent_provider_state_dir("demo", "gemini");
    let managed_home = provider_state_dir.join("home").join(".gemini");
    fs::create_dir_all(managed_home.as_std_path()).unwrap();
    fs::write(
        managed_home.join("settings.json").as_std_path(),
        "{\"security\":{\"auth\":{\"selectedType\":\"oauth-personal\"}}}\n",
    )
    .unwrap();
    fs::write(
        managed_home.join(".env").as_std_path(),
        "GEMINI_API_KEY=secret\n",
    )
    .unwrap();
    fs::write(
        managed_home.join("google_accounts.json").as_std_path(),
        "{\"active\":\"user@example.test\"}\n",
    )
    .unwrap();
    fs::write(
        managed_home.join("oauth_creds.json").as_std_path(),
        "{\"refresh_token\":\"secret\"}\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let members = archive_members(&bundle_path);
    let entries = manifest["entries"].as_array().unwrap();

    assert!(entries.iter().any(|e| {
        e["archive_path"]
            == "project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/settings.json"
            && e["status"] == "included"
    }));
    assert!(entries.iter().all(|e| e["archive_path"]
        != "project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/oauth_creds.json"));
    assert!(entries.iter().all(|e| e["archive_path"]
        != "project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/.env"));
    assert!(entries.iter().all(|e| e["archive_path"]
        != "project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/google_accounts.json"));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/oauth_creds.json",
        summary.bundle_id
    )));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/.env",
        summary.bundle_id
    )));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/gemini/home/.gemini/google_accounts.json",
        summary.bundle_id
    )));
}

#[test]
fn test_export_diagnostic_bundle_excludes_claude_credentials() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-claude-provider-state");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:claude\n").unwrap();

    let context = build_context(project_root.clone());

    let provider_state_dir = context.paths.agent_provider_state_dir("demo", "claude");
    let managed_home = provider_state_dir.join("home").join(".claude");
    fs::create_dir_all(managed_home.as_std_path()).unwrap();
    fs::write(
        managed_home.join("settings.json").as_std_path(),
        "{\"theme\":\"dark\"}\n",
    )
    .unwrap();
    fs::write(
        managed_home.join(".credentials.json").as_std_path(),
        "{\"claudeAiOauth\":{\"refreshToken\":\"secret\"}}\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let members = archive_members(&bundle_path);
    let entries = manifest["entries"].as_array().unwrap();

    assert!(entries.iter().any(|e| {
        e["archive_path"]
            == "project/.ccbr/agents/demo/provider-state/claude/home/.claude/settings.json"
            && e["status"] == "included"
    }));
    assert!(entries.iter().all(|e| e["archive_path"]
        != "project/.ccbr/agents/demo/provider-state/claude/home/.claude/.credentials.json"));
    assert!(!members.contains(&format!(
        "{}/project/.ccbr/agents/demo/provider-state/claude/home/.claude/.credentials.json",
        summary.bundle_id
    )));
}

#[test]
fn test_export_diagnostic_bundle_excludes_claude_home_hook_assets() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-claude-hook-assets");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:claude\n").unwrap();

    let context = build_context(project_root.clone());

    let provider_state_dir = context.paths.agent_provider_state_dir("demo", "claude");
    let managed_home = provider_state_dir.join("home");
    fs::create_dir_all(managed_home.join(".claude").as_std_path()).unwrap();
    fs::write(
        managed_home
            .join(".claude")
            .join("settings.json")
            .as_std_path(),
        "{\"theme\":\"dark\"}\n",
    )
    .unwrap();
    fs::create_dir_all(managed_home.join(".codeisland").as_std_path()).unwrap();
    fs::write(
        managed_home
            .join(".codeisland")
            .join("state.json")
            .as_std_path(),
        "{\"secret\":\"token\"}\n",
    )
    .unwrap();
    fs::write(
        managed_home
            .join(".codeisland")
            .join("codeisland-hook.sh")
            .as_std_path(),
        "#!/bin/sh\nexit 0\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let manifest = read_tar_json(&bundle_path, format!("{}/manifest.json", summary.bundle_id));
    let members = archive_members(&bundle_path);
    let entries = manifest["entries"].as_array().unwrap();

    assert!(entries.iter().any(|e| {
        e["archive_path"]
            == "project/.ccbr/agents/demo/provider-state/claude/home/.claude/settings.json"
            && e["status"] == "included"
    }));
    assert!(entries.iter().all(|e| !e["archive_path"]
        .as_str()
        .unwrap()
        .contains("/.codeisland/")));
    assert!(members.iter().all(|m| !m.contains("/.codeisland/")));
}

#[test]
fn test_export_diagnostic_bundle_excludes_all_provider_credentials_and_caches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path().join("repo-bundle-sensitive");
    let ccbr_dir = project_root.join(".ccbr");
    fs::create_dir_all(&ccbr_dir).unwrap();
    fs::write(ccbr_dir.join("ccbr.config"), "demo:claude\n").unwrap();

    let context = build_context(project_root.clone());

    let provider_state_dir = context.paths.agent_provider_state_dir("demo", "claude");
    let managed_home = provider_state_dir.join("home");
    fs::create_dir_all(managed_home.join(".claude").as_std_path()).unwrap();

    // Secret filenames
    for name in &[
        ".credentials.json",
        ".env",
        "auth.json",
        "google_accounts.json",
        "oauth_creds.json",
    ] {
        fs::write(
            managed_home.join(".claude").join(name).as_std_path(),
            "secret\n",
        )
        .unwrap();
    }

    // Hard-excluded directory segments
    let excluded_dirs = [
        managed_home.join(".tmp/plugins/agents/plugins/state.json"),
        managed_home.join(".local/share/claude/versions/0.9.0/bin"),
        managed_home.join(".npm/_cacache/content-v2"),
        managed_home.join(".cache/node-gyp/headers"),
        managed_home.join(".cache/vscode-ripgrep/binary"),
    ];
    for path in &excluded_dirs {
        fs::create_dir_all(path.parent().unwrap().as_std_path()).unwrap();
        fs::write(path.as_std_path(), "x\n").unwrap();
    }

    // A safe file that *should* be included
    fs::create_dir_all(managed_home.join(".claude").as_std_path()).unwrap();
    fs::write(
        managed_home.join(".claude/settings.json").as_std_path(),
        "{}\n",
    )
    .unwrap();

    let command = serde_json::to_value(ParsedDoctorCommand {
        project: None,
        bundle: true,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    })
    .unwrap();
    let summary = export_diagnostic_bundle(&context, &command).unwrap();
    let bundle_path = PathBuf::from(&summary.bundle_path);
    let members = archive_members(&bundle_path);

    let forbidden: Vec<&str> = vec![
        ".credentials.json",
        "/.env",
        "/auth.json",
        "/google_accounts.json",
        "/oauth_creds.json",
        "/.tmp/plugins/",
        "/.local/share/claude/versions/",
        "/.npm/_cacache/",
        "/.cache/node-gyp/",
        "/.cache/vscode-ripgrep/",
    ];
    for f in &forbidden {
        assert!(
            members.iter().all(|m| !m.contains(f)),
            "member contains forbidden path {}: {:?}",
            f,
            members
        );
    }
    assert!(members.iter().any(|m| m.contains("/.claude/settings.json")));
}
