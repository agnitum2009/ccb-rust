use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::app::CcbdApp;
use crate::socket_server::protocol;

/// Simple blocking Unix-domain-socket RPC server for the CCBR daemon.
pub struct SocketServer {
    socket_path: PathBuf,
    shutdown: Arc<AtomicBool>,
}

impl SocketServer {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Bind the socket and start serving requests in a background thread.
    /// The optional callback is invoked after every handled connection.
    pub fn listen<F>(
        &self,
        app: Arc<Mutex<CcbdApp>>,
        on_request: F,
    ) -> crate::Result<thread::JoinHandle<()>>
    where
        F: Fn() + Send + 'static,
    {
        // Ensure parent directory exists.
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Remove stale socket file.
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        listener.set_nonblocking(false)?;
        let shutdown = self.shutdown.clone();
        let timeout = Duration::from_millis(protocol::REQUEST_READ_TIMEOUT_MS);

        // Spawn a background heartbeat thread that drives execution polling.
        let heartbeat_shutdown = shutdown.clone();
        let heartbeat_app = app.clone();
        thread::spawn(move || {
            while !heartbeat_shutdown.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(500));
                if let Ok(mut app) = heartbeat_app.lock() {
                    app.heartbeat();
                }
            }
        });

        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                match stream {
                    Ok(stream) => {
                        Self::handle_one_connection(&app, stream, timeout);
                        on_request();
                    }
                    Err(e) => {
                        tracing::warn!("socket accept error: {}", e);
                    }
                }
            }
        });

        Ok(handle)
    }

    fn handle_one_connection(app: &Arc<Mutex<CcbdApp>>, mut stream: UnixStream, timeout: Duration) {
        let _ = stream.set_read_timeout(Some(timeout));
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let response = match app.lock() {
            Ok(mut app) => app.handle_rpc(&line),
            Err(e) => {
                tracing::error!("failed to lock daemon app: {}", e);
                r#"{"ok":false,"error":"daemon lock poisoned"}"#.to_string()
            }
        };
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(b"\n");
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Trigger a dummy connection to unblock `accept`.
        let _ = UnixStream::connect(&self.socket_path);
    }
}
