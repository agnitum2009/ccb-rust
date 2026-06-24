//! Mirrors Python `test/test_v2_kill_service.py` shutdown-intent subset.

use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::ParsedCommand;
use ccb_cli::models_start::ParsedDoctorCommand;
use ccb_cli::services::daemon::record_shutdown_intent;

fn make_context(project_root: &std::path::Path) -> CliContext {
    let ccb = project_root.join(".ccb");
    std::fs::create_dir_all(&ccb).unwrap();
    std::fs::write(ccb.join("ccb.config"), "demo:codex\n").unwrap();
    CliContextBuilder::new(ParsedCommand::Doctor(ParsedDoctorCommand {
        project: None,
        bundle: false,
        output_path: None,
        storage: false,
        json_output: false,
        kind: "doctor".into(),
    }))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

#[test]
fn test_record_shutdown_intent_persists_lifecycle_and_shutdown_intent() {
    let tmp = tempfile::tempdir().unwrap();
    let context = make_context(tmp.path());

    // Seed an existing lifecycle record so we can observe the phase transition.
    let lifecycle_path = context.paths.ccbd_lifecycle_path();
    std::fs::create_dir_all(lifecycle_path.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(
        lifecycle_path.as_std_path(),
        r#"{"phase":"mounted","desired_state":"running"}"#,
    )
    .unwrap();

    record_shutdown_intent(&context, "kill");

    let lifecycle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lifecycle_path.as_std_path()).unwrap())
            .unwrap();
    assert_eq!(lifecycle["phase"], "stopping");
    assert_eq!(lifecycle["desired_state"], "stopped");
    assert_eq!(lifecycle["shutdown_intent"], "kill");

    let shutdown_path = context.paths.ccbd_shutdown_intent_path();
    let shutdown: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(shutdown_path.as_std_path()).unwrap())
            .unwrap();
    assert_eq!(shutdown["project_id"], context.project.project_id);
    assert_eq!(shutdown["reason"], "kill");
    assert!(shutdown["requested_at"].is_number());
    assert!(shutdown["requested_by_pid"].is_number());
}

#[test]
fn test_record_shutdown_intent_keeps_unmounted_phase() {
    let tmp = tempfile::tempdir().unwrap();
    let context = make_context(tmp.path());

    let lifecycle_path = context.paths.ccbd_lifecycle_path();
    std::fs::create_dir_all(lifecycle_path.parent().unwrap().as_std_path()).unwrap();
    std::fs::write(
        lifecycle_path.as_std_path(),
        r#"{"phase":"unmounted","desired_state":"stopped"}"#,
    )
    .unwrap();

    record_shutdown_intent(&context, "kill");

    let lifecycle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lifecycle_path.as_std_path()).unwrap())
            .unwrap();
    assert_eq!(lifecycle["phase"], "unmounted");
    assert_eq!(lifecycle["shutdown_intent"], "kill");
}
