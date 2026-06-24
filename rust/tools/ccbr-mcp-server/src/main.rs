use ccbr_mcp_server::{handle_request, McpRequest};
use std::io::{self, BufRead, Write};

fn main() {
    let caller = std::env::var("CCB_CALLER")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "droid".into());

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let raw = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let msg = match serde_json::from_str::<McpRequest>(raw) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match handle_request(msg, &caller) {
            ccbr_mcp_server::HandleOutcome::Ack => {}
            ccbr_mcp_server::HandleOutcome::Respond(response) => {
                send_response(&mut stdout, &response);
            }
            ccbr_mcp_server::HandleOutcome::Exit(response) => {
                send_response(&mut stdout, &response);
                break;
            }
        }
    }
}

fn send_response(stdout: &mut io::Stdout, response: &ccbr_mcp_server::McpResponse) {
    let json = serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"internal error"}}"#.into()
    });
    let _ = writeln!(stdout, "{json}");
    let _ = stdout.flush();
}
