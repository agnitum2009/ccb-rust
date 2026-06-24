use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ccbr_daemon::app::CcbdApp;
use ccbr_daemon::socket_server::SocketServer;

const HEARTBEAT_INTERVAL_MS: u64 = 1000;

fn print_usage() {
    eprintln!("Usage: ccbrd <project-root>");
    eprintln!("   or: ccbrd --project <project-root>");
}

fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let project_root = match resolve_project_root(&args) {
        Some(path) => path,
        None => {
            print_usage();
            std::process::exit(2);
        }
    };

    let mut app = CcbdApp::new(&project_root);
    if let Err(e) = app.start() {
        eprintln!("failed to start daemon: {}", e);
        std::process::exit(1);
    }

    let socket_path = app.socket_path();
    let app = Arc::new(Mutex::new(app));
    let server = SocketServer::new(&socket_path);

    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let heartbeat_shutdown = shutdown_requested.clone();
    let heartbeat_app = Arc::clone(&app);

    // Heartbeat thread.
    let heartbeat_handle = std::thread::spawn(move || {
        while !heartbeat_shutdown.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
            if let Ok(mut app) = heartbeat_app.lock() {
                app.heartbeat();
            }
        }
    });

    // Spawn server in background; the callback drives one heartbeat per request.
    let server_handle = server
        .listen(Arc::clone(&app), || {})
        .expect("failed to bind daemon socket");

    println!(
        "ccbrd started for project {} at {}",
        app.lock().unwrap().project_id(),
        socket_path
    );

    // Wait for SIGINT/SIGTERM.
    let shutdown_for_signal = shutdown_requested.clone();
    ctrlc::set_handler(move || {
        shutdown_for_signal.store(true, Ordering::SeqCst);
    })
    .expect("failed to set signal handler");

    while !shutdown_requested.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }

    println!("ccbrd shutting down...");
    server.shutdown();
    let _ = server_handle.join();
    let _ = heartbeat_handle.join();

    if let Ok(mut app) = app.lock() {
        if let Err(e) = app.shutdown() {
            eprintln!("shutdown error: {}", e);
        }
    }

    println!("ccbrd stopped.");
}

fn resolve_project_root(args: &[String]) -> Option<PathBuf> {
    if args.is_empty() {
        return std::env::current_dir().ok();
    }
    if args[0] == "--project" {
        return args.get(1).map(PathBuf::from);
    }
    if args[0].starts_with("--project=") {
        return args[0].strip_prefix("--project=").map(PathBuf::from);
    }
    Some(PathBuf::from(&args[0]))
}
