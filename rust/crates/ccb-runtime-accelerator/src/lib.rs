use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceleratorRequest {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcceleratorResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AcceleratorResponse {
    pub fn ok(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessSample {
    pub pid: u32,
    pub ppid: u32,
    pub cpu_percent: f64,
    pub rss_kb: u64,
    pub command: String,
    pub args: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaselineSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub process_count: usize,
    pub processes: Vec<ProcessSample>,
}

pub fn handle_request_line(raw: &str) -> AcceleratorResponse {
    let request = match serde_json::from_str::<AcceleratorRequest>(raw.trim()) {
        Ok(request) => request,
        Err(err) => return AcceleratorResponse::err(format!("invalid request: {err}")),
    };
    handle_request(request)
}

pub fn handle_request(request: AcceleratorRequest) -> AcceleratorResponse {
    match request.method.as_str() {
        "ping" => AcceleratorResponse::ok(json!({
            "schema_version": SCHEMA_VERSION,
            "service": "ccb-runtime-accelerator",
            "status": "ok",
        })),
        "capabilities" => AcceleratorResponse::ok(json!({
            "schema_version": SCHEMA_VERSION,
            "capabilities": ["ping", "capabilities", "baseline_snapshot"],
            "hot_loop_replacement_active": false,
        })),
        "baseline_snapshot" => {
            let project_root = request
                .params
                .get("project_root")
                .and_then(Value::as_str)
                .unwrap_or("");
            match baseline_snapshot(project_root) {
                Ok(snapshot) => match serde_json::to_value(snapshot) {
                    Ok(value) => AcceleratorResponse::ok(value),
                    Err(err) => AcceleratorResponse::err(format!("serialize snapshot: {err}")),
                },
                Err(err) => AcceleratorResponse::err(err.to_string()),
            }
        }
        other => AcceleratorResponse::err(format!("unknown method: {other}")),
    }
}

pub fn response_line(response: &AcceleratorResponse) -> String {
    serde_json::to_string(response)
        .unwrap_or_else(|_| r#"{"ok":false,"error":"serialize response failed"}"#.to_string())
        + "\n"
}

pub fn serve(socket_path: &Path) -> anyhow::Result<()> {
    serve_with_shutdown(socket_path, || false)
}

#[cfg(unix)]
pub fn serve_with_shutdown<F>(socket_path: &Path, should_shutdown: F) -> anyhow::Result<()>
where
    F: Fn() -> bool,
{
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
    let listener = UnixListener::bind(socket_path)?;
    for stream in listener.incoming() {
        if should_shutdown() {
            break;
        }
        match stream {
            Ok(stream) => handle_stream(stream)?,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn serve_with_shutdown<F>(_socket_path: &Path, _should_shutdown: F) -> anyhow::Result<()>
where
    F: Fn() -> bool,
{
    anyhow::bail!("ccb-runtime-accelerator socket server requires Unix sockets")
}

#[cfg(unix)]
fn handle_stream(mut stream: UnixStream) -> anyhow::Result<()> {
    let mut line = String::new();
    {
        let mut reader = BufReader::new(&stream);
        reader.read_line(&mut line)?;
    }
    let response = handle_request_line(&line);
    stream.write_all(response_line(&response).as_bytes())?;
    Ok(())
}

pub fn baseline_snapshot(project_root: &str) -> anyhow::Result<BaselineSnapshot> {
    let rows = ps_rows()?;
    let project_root = project_root.trim().to_string();
    let mut processes: Vec<ProcessSample> = rows
        .into_iter()
        .filter(|sample| include_process(sample, &project_root))
        .collect();
    processes.sort_by_key(|sample| sample.pid);
    Ok(BaselineSnapshot {
        schema_version: SCHEMA_VERSION,
        project_root,
        process_count: processes.len(),
        processes,
    })
}

fn ps_rows() -> anyhow::Result<Vec<ProcessSample>> {
    let output = Command::new("ps")
        .args(["-eo", "pid=,ppid=,pcpu=,rss=,comm=,args="])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("ps failed with status {}", output.status);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().filter_map(parse_ps_line).collect())
}

fn parse_ps_line(line: &str) -> Option<ProcessSample> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let ppid = parts.next()?.parse().ok()?;
    let cpu_percent = parts.next()?.parse().ok()?;
    let rss_kb = parts.next()?.parse().ok()?;
    let command = parts.next()?.to_string();
    let args = parts.collect::<Vec<_>>().join(" ");
    let kind = classify_process(&command, &args);
    Some(ProcessSample {
        pid,
        ppid,
        cpu_percent,
        rss_kb,
        command,
        args,
        kind,
    })
}

fn include_process(sample: &ProcessSample, project_root: &str) -> bool {
    if sample.kind == "other" {
        return false;
    }
    if project_root.is_empty() {
        return true;
    }
    sample.args.contains(project_root)
}

pub fn classify_process(command: &str, args: &str) -> String {
    let text = format!("{command} {args}").to_ascii_lowercase();
    if text.contains("ccb-runtime-accelerator") {
        "accelerator".to_string()
    } else if text.contains("provider_backends.codex.bridge")
        || text.contains("codex.bridge")
        || text.contains("bridge_runtime")
    {
        "codex_bridge".to_string()
    } else if text.contains("ccbd") || text.contains("ccb-daemon") {
        "ccbd".to_string()
    } else if command.contains("codex") || text.contains(" codex ") {
        "codex_cli".to_string()
    } else {
        "other".to_string()
    }
}

pub fn default_socket_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".ccb")
        .join("runtime-accelerator")
        .join("accelerator.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_uses_daemon_like_response_shape() {
        let response = handle_request_line(r#"{"method":"ping","params":{}}"#);
        assert!(response.ok);
        let result = response.result.unwrap();
        assert_eq!(result["service"], "ccb-runtime-accelerator");
        assert_eq!(result["hot_loop_replacement_active"], Value::Null);
    }

    #[test]
    fn capabilities_report_no_hot_loop_replacement_in_slice_zero() {
        let response = handle_request_line(r#"{"method":"capabilities","params":{}}"#);
        assert!(response.ok);
        let result = response.result.unwrap();
        assert_eq!(result["hot_loop_replacement_active"], false);
    }

    #[test]
    fn unknown_method_fails_loudly() {
        let response = handle_request_line(r#"{"method":"replace_everything","params":{}}"#);
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("unknown method"));
    }

    #[test]
    fn classifies_hot_loop_processes() {
        assert_eq!(
            classify_process("python", "-m provider_backends.codex.bridge /repo"),
            "codex_bridge"
        );
        assert_eq!(classify_process("ccbd", "--project /repo"), "ccbd");
        assert_eq!(classify_process("codex", "--sandbox danger"), "codex_cli");
    }

    #[test]
    fn default_socket_lives_under_project_ccb() {
        assert_eq!(
            default_socket_path(Path::new("/repo")),
            PathBuf::from("/repo/.ccb/runtime-accelerator/accelerator.sock")
        );
    }
}
