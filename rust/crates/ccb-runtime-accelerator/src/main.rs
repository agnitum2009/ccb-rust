use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "ccb-runtime-accelerator")]
#[command(about = "CCB legacy runtime hot-loop accelerator sidecar")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve newline-delimited JSON RPC over a Unix socket.
    Serve {
        #[arg(long)]
        socket: PathBuf,
    },
    /// Probe a running accelerator.
    Ping {
        #[arg(long)]
        socket: PathBuf,
    },
    /// Emit a process CPU/RSS snapshot for CCB hot-loop processes.
    BaselineSnapshot {
        #[arg(long)]
        project_root: PathBuf,
    },
    /// Print the default accelerator socket path for a project.
    SocketPath {
        #[arg(long)]
        project_root: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { socket } => ccb_runtime_accelerator::serve(&socket),
        Command::Ping { socket } => {
            let response = call(&socket, "ping", json!({}))?;
            print!("{}", response);
            Ok(())
        }
        Command::BaselineSnapshot { project_root } => {
            let snapshot =
                ccb_runtime_accelerator::baseline_snapshot(&project_root.to_string_lossy())?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
            Ok(())
        }
        Command::SocketPath { project_root } => {
            println!(
                "{}",
                ccb_runtime_accelerator::default_socket_path(&project_root).display()
            );
            Ok(())
        }
    }
}

fn call(socket: &PathBuf, method: &str, params: serde_json::Value) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(socket)?;
    let request = serde_json::to_string(&json!({"method": method, "params": params}))?;
    stream.write_all(request.as_bytes())?;
    stream.write_all(b"\n")?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}
