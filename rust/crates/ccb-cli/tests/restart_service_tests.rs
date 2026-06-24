//! Mirrors Python `test/test_ccb_restart.py` phase2/render subset.

use std::io::Write;
use std::sync::{Arc, Mutex};

use ccb_cli::context::{CliContext, CliContextBuilder};
use ccb_cli::models::ParsedCommand;
use ccb_cli::models_start::ParsedRestartCommand;
use ccb_cli::phase2_runtime::dispatch::dispatch;
use ccb_cli::phase2_runtime::handlers_ops::Phase2Services;
use serde_json::{json, Value};

fn make_context(tmp: &tempfile::TempDir) -> CliContext {
    let project_root = tmp.path();
    std::fs::create_dir_all(project_root.join(".ccb")).unwrap();
    std::fs::write(
        project_root.join(".ccb/ccb.config"),
        "cmd; agent1:codex, agent2:claude\n",
    )
    .unwrap();
    CliContextBuilder::new(ParsedCommand::Restart(ParsedRestartCommand::new(
        None,
        "agent1".into(),
    )))
    .cwd(project_root.to_path_buf())
    .build()
    .unwrap()
}

#[derive(Clone)]
struct FakeServices {
    captured: Arc<Mutex<Option<Value>>>,
    response: Value,
}

impl Phase2Services for FakeServices {
    fn write_lines<W: Write>(&self, out: &mut W, lines: &[String]) {
        for line in lines {
            writeln!(out, "{}", line).ok();
        }
    }

    fn restart_agent(&self, _context: &CliContext, command: &Value) -> Value {
        *self.captured.lock().unwrap() = Some(command.clone());
        self.response.clone()
    }

    fn kill_project(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn cleanup_project_storage(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn clear_agent_context(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn agent_logs(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn maintenance_status(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn ps_summary(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn export_diagnostic_bundle(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn doctor_storage_summary(&self, _context: &CliContext) -> Value {
        unimplemented!()
    }
    fn doctor_summary(&self, _context: &CliContext) -> Value {
        unimplemented!()
    }
    fn list_fault_rules(&self, _context: &CliContext) -> Value {
        unimplemented!()
    }
    fn arm_fault_rule(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn clear_fault_rule(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn reload_config(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn submit_ask(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn ping_target(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn pend_target(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn queue_target(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn trace_target(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn inbox_target(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn ack_reply(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn watch_target(&self, _context: &CliContext, _command: &Value) -> Vec<Value> {
        unimplemented!()
    }
    fn resubmit_message(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn retry_attempt(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn wait_for_replies(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn cancel_job(&self, _context: &CliContext, _command: &Value) -> Value {
        unimplemented!()
    }
    fn validate_config_context(&self, _context: &CliContext) -> Value {
        unimplemented!()
    }
    fn start_agents(
        &self,
        _context: &CliContext,
        _command: &Value,
        _terminal_size: Option<(u16, u16)>,
    ) -> Value {
        unimplemented!()
    }
}

#[test]
fn test_phase2_restart_sends_request_and_renders_summary() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = make_context(&tmp);
    let captured = Arc::new(Mutex::new(None));
    let services = FakeServices {
        captured: captured.clone(),
        response: json!({
            "restart_status": "ok",
            "agent_name": "agent1",
            "restartable_agents": ["agent1", "agent2"],
            "busy_gate": {
                "passed": true,
                "runtime_state": "idle",
                "runtime_queue_depth": 0,
                "queue_depth": 0,
                "pending_reply_count": 0,
                "active_job_id": null,
                "active_inbound_event_id": null,
                "pending_callback_count": 0,
            },
            "old_runtime": {"state": "idle", "health": "healthy", "pane_id": "%1", "active_pane_id": "%1"},
            "new_runtime": {"state": "idle", "health": "healthy", "pane_id": "%2", "active_pane_id": "%2"},
            "result": {"agent": "agent1", "status": "restarted", "pane_id": "%2"},
        }),
    };

    let mut out = Vec::new();
    let command = json!({"kind": "restart", "agent_name": "agent1"});
    let code = dispatch(&context, &command, &mut out, &services);

    let output = String::from_utf8(out).unwrap();
    assert_eq!(code, 0, "output: {}", output);
    assert_eq!(
        captured.lock().unwrap().as_ref().unwrap()["agent_name"],
        "agent1"
    );
    assert!(output.contains("restart_status: ok"), "output: {}", output);
    assert!(output.contains("agent_name: agent1"), "output: {}", output);
    assert!(
        output.contains("restart_busy_gate: passed=true"),
        "output: {}",
        output
    );
    assert!(
        output.contains(
            "old_runtime: state=idle health=healthy pane_id=%1 active_pane_id=%1 runtime_ref=None session_ref=None runtime_pid=None restart_count=None"
        ),
        "output: {}",
        output
    );
    assert!(
        output.contains(
            "new_runtime: state=idle health=healthy pane_id=%2 active_pane_id=%2 runtime_ref=None session_ref=None runtime_pid=None restart_count=None"
        ),
        "output: {}",
        output
    );
}
