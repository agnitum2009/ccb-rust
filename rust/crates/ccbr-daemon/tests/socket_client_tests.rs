//! Mirrors Python `test/test_ccbrd_socket_client.py`.

use std::cell::RefCell;
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use ccbr_daemon::api_models::MessageEnvelope;
use ccbr_daemon::socket_client::CcbdClient;
use ccbr_daemon::socket_client_runtime::errors::CcbdClientError;
use ccbr_daemon::socket_client_runtime::transport::{connect_socket, Socket};

fn temp_socket_path() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ccbrd.sock");
    // Leak the tempdir handle so the directory stays alive for the test.
    Box::leak(Box::new(dir));
    path
}

#[test]
fn test_ccbrd_client_uses_stable_default_timeout() {
    let client = CcbdClient::new(temp_socket_path());
    assert!((client.timeout_s() - 3.0).abs() < f64::EPSILON);
}

#[test]
fn test_ccbrd_client_reads_timeout_from_env() {
    std::env::set_var("CCBR_CCBRD_CLIENT_TIMEOUT_S", "4.5");
    let client = CcbdClient::new(temp_socket_path());
    assert!((client.timeout_s() - 4.5).abs() < f64::EPSILON);
    std::env::remove_var("CCBR_CCBRD_CLIENT_TIMEOUT_S");
}

#[test]
fn test_ccbrd_client_explicit_timeout_overrides_env() {
    std::env::set_var("CCBR_CCBRD_CLIENT_TIMEOUT_S", "4.5");
    let client = CcbdClient::new(temp_socket_path()).with_timeout(0.2);
    assert!((client.timeout_s() - 0.2).abs() < f64::EPSILON);
    std::env::remove_var("CCBR_CCBRD_CLIENT_TIMEOUT_S");
}

#[test]
fn test_ccbrd_client_with_timeout_preserves_socket_path() {
    std::env::set_var("CCBR_CCBRD_CLIENT_TIMEOUT_S", "4.5");
    let path = temp_socket_path();
    let client = CcbdClient::new(&path);
    let cloned = client.with_timeout(12.0);

    assert_ne!(&client as *const _, &cloned as *const _);
    assert_eq!(cloned.socket_path(), path);
    assert!((cloned.timeout_s() - 12.0).abs() < f64::EPSILON);
    assert!((client.timeout_s() - 4.5).abs() < f64::EPSILON);
    std::env::remove_var("CCBR_CCBRD_CLIENT_TIMEOUT_S");
}

struct TestEnvelope {
    to_agent: String,
    body: String,
}

impl MessageEnvelope for TestEnvelope {
    fn to_record(&self) -> Value {
        serde_json::json!({
            "to_agent": self.to_agent,
            "body": self.body,
        })
    }
}

#[test]
fn test_ccbrd_client_dynamic_submit_endpoint_uses_request() {
    let (path, _join, recorded) = echo_server(r#"{"ok": true, "payload": {"ok": true}}"#);
    let client = CcbdClient::new(&path);

    let envelope = TestEnvelope {
        to_agent: "agent1".into(),
        body: "hello".into(),
    };
    let payload = client.submit(&envelope).unwrap();

    assert_eq!(payload, serde_json::json!({"ok": true}));
    let recorded = recorded.lock().unwrap().take().unwrap();
    assert_eq!(recorded["op"], "submit");
    assert_eq!(recorded["request"]["to_agent"], "agent1");
}

#[test]
fn test_ccbrd_client_dynamic_attach_endpoint_builds_payload() {
    let (path, _join, recorded) = echo_server(r#"{"ok": true, "payload": {"ok": true}}"#);
    let client = CcbdClient::new(&path);

    let payload = client
        .attach(
            "agent3",
            "/tmp/work",
            "pane-backed",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("%9"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("external-attach"),
        )
        .unwrap();

    assert_eq!(payload, serde_json::json!({"ok": true}));
    let recorded = recorded.lock().unwrap().take().unwrap();
    assert_eq!(recorded["op"], "attach");
    let req = &recorded["request"];
    assert_eq!(req["agent_name"], "agent3");
    assert_eq!(req["workspace_path"], "/tmp/work");
    assert_eq!(req["backend_type"], "pane-backed");
    assert_eq!(req["pane_id"], "%9");
    assert_eq!(req["binding_source"], "external-attach");
    assert!(req["pid"].is_null());
    assert!(req["runtime_ref"].is_null());
}

#[test]
fn test_ccbrd_client_dynamic_shutdown_endpoint_uses_empty_payload() {
    let (path, _join, recorded) = echo_server(r#"{"ok": true, "payload": {"ok": true}}"#);
    let client = CcbdClient::new(&path);

    client.shutdown().unwrap();

    let recorded = recorded.lock().unwrap().take().unwrap();
    assert_eq!(recorded["op"], "shutdown");
    assert_eq!(recorded["request"], serde_json::json!({}));
}

#[test]
fn test_ccbrd_client_project_restart_panes_endpoint_uses_empty_payload() {
    let (path, _join, recorded) = echo_server(r#"{"ok": true, "payload": {"ok": true}}"#);
    let client = CcbdClient::new(&path);

    client.project_restart_panes().unwrap();

    let recorded = recorded.lock().unwrap().take().unwrap();
    assert_eq!(recorded["op"], "project_restart_panes");
    assert_eq!(recorded["request"], serde_json::json!({}));
}

#[test]
fn test_ccbrd_client_project_clear_context_endpoint_builds_payload() {
    let (path, _join, recorded) = echo_server(r#"{"ok": true, "payload": {"ok": true}}"#);
    let client = CcbdClient::new(&path);

    client
        .project_clear_context(&["agent1".into(), "agent2".into()])
        .unwrap();

    let recorded = recorded.lock().unwrap().take().unwrap();
    assert_eq!(recorded["op"], "project_clear_context");
    assert_eq!(
        recorded["request"]["agent_names"],
        serde_json::json!(["agent1", "agent2"])
    );
}

#[test]
fn test_ccbrd_client_request_wraps_socket_connect_errors() {
    // Create a listener, capture its path, then drop it so the next connect
    // receives ECONNREFUSED.
    let path = temp_socket_path();
    let listener = UnixListener::bind(&path).unwrap();
    drop(listener);

    let client = CcbdClient::new(&path);
    let err = client.request("ping", None).unwrap_err();
    assert!(err.to_string().contains("Connection refused"));
}

#[derive(Default, Debug)]
struct FakeSocket {
    timeouts: RefCell<Vec<Option<Duration>>>,
    closed: RefCell<bool>,
}

impl Socket for FakeSocket {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.timeouts.borrow_mut().push(dur);
        Ok(())
    }
}

impl Read for FakeSocket {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl Write for FakeSocket {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for FakeSocket {
    fn drop(&mut self) {
        *self.closed.borrow_mut() = true;
    }
}

#[test]
fn test_connect_socket_retries_transient_connect_errors_within_timeout() {
    let path = temp_socket_path();
    let attempts = Arc::new(AtomicUsize::new(0));
    let current = Rc::new(RefCell::new(0.0));
    let sleeps: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));

    let attempts_f = attempts.clone();
    let current_f = current.clone();
    let sleeps_f = sleeps.clone();

    let factory = move || -> io::Result<FakeSocket> {
        let n = attempts_f.fetch_add(1, Ordering::SeqCst) + 1;
        if n == 1 {
            return Err(io::Error::from(ErrorKind::WouldBlock));
        }
        Ok(FakeSocket::default())
    };

    let monotonic = move || *current.borrow();
    let sleep = move |seconds: f64| {
        sleeps_f.borrow_mut().push(seconds);
        *current_f.borrow_mut() += seconds;
    };

    let sock: FakeSocket = connect_socket(&path, 0.5, factory, monotonic, sleep).unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(*sleeps.borrow(), vec![0.05]);
    assert!(!sock.timeouts.borrow().is_empty());
}

#[test]
fn test_connect_socket_does_not_retry_non_transient_errors() {
    let path = temp_socket_path();
    let attempts = Arc::new(AtomicUsize::new(0));

    let factory = {
        let attempts = attempts.clone();
        move || -> io::Result<FakeSocket> {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::from_raw_os_error(13))
        }
    };

    let err: CcbdClientError = connect_socket(
        &path,
        0.5,
        factory,
        || 0.0,
        |_seconds| unreachable!("should not sleep"),
    )
    .unwrap_err();

    assert!(err.to_string().contains("Permission denied"));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[test]
fn test_connect_socket_caps_transient_connect_retries() {
    let path = temp_socket_path();
    let attempts = Arc::new(AtomicUsize::new(0));
    let current = Rc::new(RefCell::new(0.0));

    let attempts_f = attempts.clone();
    let current_f = current.clone();

    let factory = move || -> io::Result<FakeSocket> {
        attempts_f.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::from_raw_os_error(2))
    };

    let monotonic = move || *current.borrow();
    let sleep = move |seconds: f64| {
        *current_f.borrow_mut() += seconds;
    };

    let err: CcbdClientError = connect_socket(&path, 0.5, factory, monotonic, sleep).unwrap_err();

    assert!(err.to_string().contains("No such file"));
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Start a tiny Unix socket echo server that returns `response_json` once.
///
/// Returns the socket path, a handle for the server thread, and a shared
/// record that will contain the parsed JSON-RPC request after a client call.
fn echo_server(
    response_json: &'static str,
) -> (PathBuf, thread::JoinHandle<()>, Arc<Mutex<Option<Value>>>) {
    let path = temp_socket_path();
    let listener = UnixListener::bind(&path).unwrap();
    let path_clone = path.clone();
    let recorded: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let recorded_clone = recorded.clone();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() > 0 {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                *recorded_clone.lock().unwrap() = Some(value);
            }
        }
        let mut stream = reader.into_inner();
        stream.write_all(response_json.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();
    });
    (path_clone, handle, recorded)
}
