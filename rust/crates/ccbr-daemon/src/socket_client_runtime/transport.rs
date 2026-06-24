//! Mirrors Python `lib/ccbd/socket_client_runtime/transport.py`.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::api_models::{RpcRequest, RpcResponse};

use super::errors::CcbdClientError;

const CONNECT_RETRY_INTERVAL_S: f64 = 0.05;
const CONNECT_MAX_RETRIES: usize = 2;

/// Abstract socket used by `connect_socket` so tests can inject fakes.
pub trait Socket: Read + Write {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()>;
}

impl Socket for UnixStream {
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout(dur)
    }
}

/// Connect a socket to `path` with transient-error retry and deadline timeout.
///
/// The caller supplies a `factory` closure that creates and connects a socket
/// each attempt. This mirrors Python's `socket.socket` monkeypatching while
/// remaining testable without real I/O.
pub fn connect_socket<S, F, M, Sl>(
    _path: &Path,
    timeout_s: f64,
    mut factory: F,
    monotonic_fn: M,
    mut sleep_fn: Sl,
) -> Result<S, CcbdClientError>
where
    S: Socket,
    F: FnMut() -> io::Result<S>,
    M: Fn() -> f64,
    Sl: FnMut(f64),
{
    let deadline = monotonic_fn() + timeout_s.max(0.0);
    let mut last_error: Option<io::Error> = None;

    for attempt in 0..=CONNECT_MAX_RETRIES {
        let remaining = deadline - monotonic_fn();
        if remaining <= 0.0 {
            break;
        }

        match factory() {
            Ok(sock) => {
                sock.set_read_timeout(Some(Duration::from_secs_f64(remaining)))
                    .map_err(|e| CcbdClientError::new(e.to_string()))?;
                return Ok(sock);
            }
            Err(e) => {
                last_error = Some(e);
                if !is_transient_connect_error(last_error.as_ref().unwrap())
                    || attempt >= CONNECT_MAX_RETRIES
                {
                    break;
                }
                let sleep_for = CONNECT_RETRY_INTERVAL_S.min((deadline - monotonic_fn()).max(0.0));
                if sleep_for <= 0.0 {
                    break;
                }
                sleep_fn(sleep_for);
            }
        }
    }

    if let Some(e) = last_error {
        return Err(CcbdClientError::new(e.to_string()));
    }
    Err(CcbdClientError::new("timed out"))
}

fn is_transient_connect_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
    )
}

/// Convenience wrapper using real Unix sockets and wall-clock time.
pub fn connect_socket_unix(path: &Path, timeout_s: f64) -> Result<UnixStream, CcbdClientError> {
    connect_socket(
        path,
        timeout_s,
        || UnixStream::connect(path),
        monotonic_now,
        |dur| std::thread::sleep(Duration::from_secs_f64(dur)),
    )
}

fn monotonic_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Send a JSON-RPC request followed by a newline.
pub fn send_request<S: Write>(sock: &mut S, request: &RpcRequest) -> Result<(), CcbdClientError> {
    let payload = serde_json::to_string(request)? + "\n";
    sock.write_all(payload.as_bytes())?;
    Ok(())
}

/// Read a single newline-terminated response line.
pub fn recv_response_line<S: Read>(sock: &mut S) -> Result<String, CcbdClientError> {
    let mut reader = BufReader::new(sock);
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(CcbdClientError::new("empty response from ccbd"));
    }
    Ok(line)
}

/// Parse a raw response line into an `RpcResponse`.
pub fn decode_response(raw: &str) -> Result<RpcResponse, CcbdClientError> {
    Ok(serde_json::from_str(raw)?)
}
