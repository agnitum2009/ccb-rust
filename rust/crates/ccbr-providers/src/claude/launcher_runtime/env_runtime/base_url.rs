//! Mirrors Python `lib/provider_backends/claude/launcher_runtime/env_runtime/base_url.py`.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Read `ANTHROPIC_BASE_URL` from the user's Claude settings file.
pub fn claude_user_base_url(user_settings_path: &camino::Utf8Path) -> String {
    super::overlay::claude_user_base_url(user_settings_path)
}

/// Decide whether a base URL points at a dead local TCP listener and should be
/// unset before launching Claude.
pub fn should_drop_claude_base_url<F>(value: &str, local_tcp_listener_available_fn: F) -> bool
where
    F: Fn(&str, u16) -> bool,
{
    let (host, port) = local_base_url_target(value);
    match (host, port) {
        (Some(host), Some(port)) => !local_tcp_listener_available_fn(&host, port),
        _ => false,
    }
}

/// Parse a local base URL target. Returns `(Some(host), Some(port))` only for
/// loopback hosts with an explicit port.
pub fn local_base_url_target(value: &str) -> (Option<String>, Option<u16>) {
    let value = value.trim();
    if value.is_empty() {
        return (None, None);
    }
    // Strip optional scheme and path, keeping host[:port].
    let re =
        regex::Regex::new(r"^(?:[a-zA-Z][a-zA-Z0-9+.-]*://)?([^/]*?)(?::(\d+))?(/.*)?$").unwrap();
    let caps = match re.captures(value) {
        Some(c) => c,
        None => return (None, None),
    };
    let host = match caps.get(1).map(|m| m.as_str().trim().to_lowercase()) {
        Some(h) => h,
        None => return (None, None),
    };
    let port: u16 = match caps.get(2).and_then(|m| m.as_str().parse().ok()) {
        Some(p) => p,
        None => return (None, None),
    };
    if !matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1") {
        return (None, None);
    }
    (Some(host), Some(port))
}

/// Check whether a TCP listener is available at `host:port`.
pub fn local_tcp_listener_available(host: &str, port: u16) -> bool {
    let addrs: Vec<SocketAddr> = match (host, port).to_socket_addrs() {
        Ok(iter) => iter.collect(),
        Err(_) => return false,
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_base_url_target_loopback_with_port() {
        assert_eq!(
            local_base_url_target("http://127.0.0.1:12345/path"),
            (Some("127.0.0.1".into()), Some(12345))
        );
        assert_eq!(
            local_base_url_target("https://localhost:8080"),
            (Some("localhost".into()), Some(8080))
        );
    }

    #[test]
    fn test_local_base_url_target_non_loopback_or_missing_port() {
        assert_eq!(
            local_base_url_target("https://api.example.test"),
            (None, None)
        );
        assert_eq!(local_base_url_target("http://127.0.0.1"), (None, None));
        assert_eq!(local_base_url_target("not-a-url"), (None, None));
    }
}
