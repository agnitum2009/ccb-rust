use std::fmt;

const TMUX_TRANSIENT_SERVER_ERROR_MARKERS: &[&str] = &[
    "fork failed",
    "no server running",
    "server exited unexpectedly",
];

const TMUX_ABSENT_SERVER_ERROR_MARKERS: &[&str] = &["no server running"];

const TMUX_MISSING_SESSION_ERROR_MARKERS: &[&str] = &["can't find session", "session not found"];

/// tmux server/socket exists as authority, but is not ready for control-plane work yet.
#[derive(Debug)]
pub struct TmuxTransientServerUnavailable {
    message: String,
}

impl TmuxTransientServerUnavailable {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for TmuxTransientServerUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TmuxTransientServerUnavailable {}

/// A tmux command failed with command/socket context preserved for diagnostics.
#[derive(Debug)]
pub struct TmuxCommandError {
    message: String,
}

impl TmuxCommandError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for TmuxCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TmuxCommandError {}

/// Extract failure detail from a completed process-like object.
pub fn tmux_failure_detail(stderr: &str, stdout: &str, args: &[String]) -> String {
    let stderr = stderr.trim();
    let stdout = stdout.trim();
    if !stderr.is_empty() || !stdout.is_empty() {
        return if stderr.is_empty() { stdout } else { stderr }.to_string();
    }
    format!("tmux command failed: {}", args.join(" "))
}

/// Build a tmux command failure message with context.
pub fn tmux_command_failure_message(
    message: &str,
    args: Option<&[String]>,
    detail: Option<&str>,
    socket_path: Option<&str>,
    command: Option<&[String]>,
) -> String {
    let mut parts = vec![message.to_string()];
    if let Some(socket) = socket_path {
        let socket = socket.trim();
        if !socket.is_empty() {
            parts.push(format!("tmux_socket_path={socket}"));
            parts.push(format!("tmux_socket_path_bytes={}", socket.len()));
        }
    }
    let command_text = command
        .map(|c| c.join(" "))
        .or_else(|| args.map(|a| a.join(" ")))
        .unwrap_or_default();
    if !command_text.is_empty() {
        parts.push(format!("tmux_command={command_text:?}"));
    }
    if let Some(d) = detail {
        let single = d
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" | ");
        if !single.is_empty() && !message.contains(&single) {
            parts.push(format!("tmux_detail={single:?}"));
        }
    }
    parts.join("; ")
}

fn _single_line_detail(detail: &str) -> String {
    detail
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Check if error text indicates a transient tmux server error.
pub fn is_tmux_transient_server_error_text(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    TMUX_TRANSIENT_SERVER_ERROR_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

/// Check if an error is a transient server error.
pub fn is_tmux_transient_server_error<E: std::error::Error>(err: &E) -> bool {
    is_tmux_transient_server_error_text(&err.to_string())
}

/// Check if error text indicates absent tmux server.
pub fn is_tmux_absent_server_text(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    TMUX_ABSENT_SERVER_ERROR_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

/// Check if error text indicates missing tmux session.
pub fn is_tmux_missing_session_text(text: &str) -> bool {
    let normalized = text.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    TMUX_MISSING_SESSION_ERROR_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_error_detection() {
        assert!(is_tmux_transient_server_error_text(
            "fork failed: Device not configured"
        ));
        assert!(is_tmux_transient_server_error_text(
            "no server running on /tmp/ccbr-runtime/test.sock"
        ));
        assert!(is_tmux_transient_server_error_text(
            "server exited unexpectedly"
        ));
        assert!(!is_tmux_transient_server_error_text("pane not found"));
    }

    #[test]
    fn test_absent_and_missing_session_detection() {
        assert!(is_tmux_absent_server_text("no server running"));
        assert!(!is_tmux_absent_server_text("pane not found"));
        assert!(is_tmux_missing_session_text("can't find session"));
        assert!(is_tmux_missing_session_text("session not found"));
    }

    #[test]
    fn test_tmux_command_failure_message() {
        let msg = tmux_command_failure_message(
            "tmux command failed",
            Some(&["list-panes".to_string()]),
            Some("no server running"),
            Some("/tmp/ccb.sock"),
            None,
        );
        assert!(msg.contains("tmux_socket_path=/tmp/ccb.sock"));
        assert!(msg.contains("tmux_command="));
        assert!(msg.contains("no server running"));
    }
}
