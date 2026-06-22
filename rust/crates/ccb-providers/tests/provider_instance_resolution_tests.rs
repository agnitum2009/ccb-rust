use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ccb_providers::claude::session::{load_session as load_claude_session, ClaudeProjectSession};
use ccb_providers::droid::session::{load_session as load_droid_session, DroidProjectSession};
use ccb_providers::opencode::session::{
    load_session as load_opencode_session, OpenCodeProjectSession,
};
use ccb_providers::providers::{codex, gemini};
use serde_json::Value;

#[test]
fn test_named_agent_load_session_does_not_fallback_to_primary() {
    let mut calls: Vec<Option<String>> = Vec::new();

    let mut loader = |work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(ClaudeProjectSession {
                session_file: work_dir.to_path_buf(),
                data: HashMap::from([(
                    "session".to_string(),
                    Value::String("primary".to_string()),
                )]),
            })
        } else {
            None
        }
    };

    let work_dir = PathBuf::from("/tmp/demo");
    assert!(load_claude_session(|w, i| loader(w, i), &work_dir, "agent3").is_none());
    assert_eq!(calls, vec![Some("agent3".to_string())]);

    calls.clear();
    let mut codex_loader = |work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(codex::CodexProjectSession {
                session_file: work_dir.to_path_buf(),
                data: HashMap::from([(
                    "session".to_string(),
                    Value::String("primary".to_string()),
                )]),
            })
        } else {
            None
        }
    };
    assert!(codex::load_session(|w, i| codex_loader(w, i), &work_dir, "agent1").is_none());
    assert_eq!(calls, vec![Some("agent1".to_string())]);

    calls.clear();
    let mut gemini_loader = |_work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(HashMap::from([(
                "session".to_string(),
                Value::String("primary".to_string()),
            )]))
        } else {
            None
        }
    };
    assert!(gemini::load_session(|w, i| gemini_loader(w, i), &work_dir, "reviewer").is_none());
    assert_eq!(calls, vec![Some("reviewer".to_string())]);

    calls.clear();
    let mut opencode_loader = |work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(OpenCodeProjectSession {
                session_file: work_dir.to_path_buf(),
                data: HashMap::from([(
                    "session".to_string(),
                    Value::String("primary".to_string()),
                )]),
            })
        } else {
            None
        }
    };
    assert!(
        load_opencode_session(&work_dir, "builder", "opencode", |w, i| opencode_loader(
            w, i
        ),)
        .is_none()
    );
    assert_eq!(calls, vec![Some("builder".to_string())]);

    calls.clear();
    let mut droid_loader = |work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(DroidProjectSession {
                session_file: work_dir.to_path_buf(),
                data: HashMap::from([(
                    "session".to_string(),
                    Value::String("primary".to_string()),
                )]),
            })
        } else {
            None
        }
    };
    assert!(load_droid_session(|w, i| droid_loader(w, i), &work_dir, "worker", "droid").is_none());
    assert_eq!(calls, vec![Some("worker".to_string())]);
}

#[test]
fn test_primary_agent_load_session_keeps_primary_fallback() {
    let mut calls: Vec<Option<String>> = Vec::new();

    let mut loader = |work_dir: &Path, instance: Option<&str>| {
        calls.push(instance.map(String::from));
        if instance.is_none() {
            Some(ClaudeProjectSession {
                session_file: work_dir.to_path_buf(),
                data: HashMap::from([(
                    "session".to_string(),
                    Value::String("primary".to_string()),
                )]),
            })
        } else {
            None
        }
    };

    let work_dir = PathBuf::from("/tmp/demo");
    assert!(load_claude_session(|w, i| loader(w, i), &work_dir, "claude").is_none());
    assert_eq!(calls, vec![Some("claude".to_string())]);
}
