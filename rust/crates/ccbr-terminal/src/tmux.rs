use std::path::Path;

const DEFAULT_CCBR_TMUX_CONFIG: &str = "/dev/null";

/// Build the base tmux command vector including config and socket args.
pub fn tmux_base(socket_name: Option<&str>, socket_path: Option<&str>) -> Vec<String> {
    let mut cmd = vec!["tmux".to_string()];
    cmd.extend(config_base_args());
    cmd.extend(socket_base_args(socket_name, socket_path));
    cmd
}

/// Build config arguments from `CCBR_TMUX_CONFIG` env.
pub fn config_base_args() -> Vec<String> {
    let config_path = std::env::var("CCBR_TMUX_CONFIG")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CCBR_TMUX_CONFIG.to_string());
    if config_path.is_empty() {
        Vec::new()
    } else {
        vec!["-f".to_string(), expanduser(&config_path)]
    }
}

/// Build socket arguments: socket_path takes precedence over socket_name.
pub fn socket_base_args(socket_name: Option<&str>, socket_path: Option<&str>) -> Vec<String> {
    if let Some(path) = socket_path {
        return vec!["-S".to_string(), expanduser(path)];
    }
    if let Some(name) = socket_name {
        return vec!["-L".to_string(), name.to_string()];
    }
    Vec::new()
}

/// Normalize a socket name, treating "default" as None.
pub fn normalize_socket_name(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() || text == "default" {
        None
    } else {
        Some(text.to_string())
    }
}

/// Extract socket name from `TMUX` environment value.
pub fn socket_name_from_tmux_env(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        return None;
    }
    let socket_path = text.split(',').next().unwrap_or("").trim();
    if socket_path.is_empty() {
        return None;
    }
    normalize_socket_name(Path::new(socket_path).file_name().and_then(|s| s.to_str()))
}

/// Extract socket reference (path or name) from `TMUX` env value.
pub fn socket_ref_from_tmux_env(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or("").trim();
    if text.is_empty() {
        return None;
    }
    let socket_path = text.split(',').next().unwrap_or("").trim();
    if socket_path.is_empty() {
        return None;
    }
    if socket_path.contains('/') || socket_path.contains('\\') {
        Some(expanduser(socket_path))
    } else {
        normalize_socket_name(Some(socket_path))
    }
}

/// Check if value looks like a tmux pane id (starts with `%`).
pub fn looks_like_pane_id(value: &str) -> bool {
    value.trim().starts_with('%')
}

/// Check if value looks like a tmux target (pane id, session, or session.window.pane).
pub fn looks_like_tmux_target(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty() && (v.starts_with('%') || v.contains(':') || v.contains('.'))
}

/// Normalize split direction string to tmux flag and canonical direction name.
pub fn normalize_split_direction(direction: &str) -> (&'static str, &'static str) {
    let direction_norm = direction.trim().to_lowercase();
    match direction_norm.as_str() {
        "right" | "h" | "horizontal" => ("-h", "right"),
        "bottom" | "v" | "vertical" => ("-v", "bottom"),
        _ => panic!("unsupported direction: {direction:?} (use 'right' or 'bottom')"),
    }
}

/// Find pane id by title marker from `list-panes` stdout.
pub fn pane_id_by_title_marker_output(stdout: &str, marker: &str) -> Option<String> {
    let marker = normalized_marker(marker);
    if marker.is_empty() {
        return None;
    }
    let (exact_matches, prefix_matches) = collect_pane_title_matches(stdout, &marker);
    select_marker_match(&exact_matches, &prefix_matches)
}

/// Collect exact and prefix pane-title matches from `list-panes` stdout.
pub fn collect_pane_title_matches(stdout: &str, marker: &str) -> (Vec<String>, Vec<String>) {
    let mut exact_matches: Vec<String> = Vec::new();
    let mut prefix_matches: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if let Some((pid, title)) = parse_pane_title_line(line) {
            record_pane_title_match(
                &pid,
                &title,
                marker,
                &mut exact_matches,
                &mut prefix_matches,
            );
        }
    }
    (exact_matches, prefix_matches)
}

/// Record a single pane title match into the exact/prefix buckets.
pub fn record_pane_title_match(
    pid: &str,
    title: &str,
    marker: &str,
    exact_matches: &mut Vec<String>,
    prefix_matches: &mut Vec<String>,
) {
    if title == marker {
        exact_matches.push(pid.to_string());
    } else if title.starts_with(marker) {
        prefix_matches.push(pid.to_string());
    }
}

/// Select the unique marker match, if any.
pub fn select_marker_match(exact_matches: &[String], prefix_matches: &[String]) -> Option<String> {
    if exact_matches.len() == 1 {
        return exact_matches.first().cloned();
    }
    if !exact_matches.is_empty() {
        return None;
    }
    if prefix_matches.len() == 1 {
        return prefix_matches.first().cloned();
    }
    None
}

/// Generate a default detached session name.
pub fn default_detached_session_name(cwd: &str, pid: u32, now_ts: f64) -> String {
    let dir_name = Path::new(cwd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("ccb");
    format!("ccbr-{dir_name}-{}-{pid}", (now_ts as i64) % 100000)
}

/// Normalize a title marker string.
pub fn normalized_marker(marker: &str) -> String {
    marker.trim().to_string()
}

/// Parse a `pane_id title` line from `list-panes -F` output.
pub fn parse_pane_title_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let (pid, title) = split_pane_title_line(line);
    let pid = pid.trim();
    if !looks_like_pane_id(pid) {
        return None;
    }
    Some((pid.to_string(), title.trim().to_string()))
}

/// Split a pane title line into (pane_id, title).
pub fn split_pane_title_line(line: &str) -> (&str, &str) {
    if let Some((pid, title)) = line.split_once('\t') {
        return (pid, title);
    }
    if let Some((pid, title)) = line.split_once(' ') {
        return (pid, title);
    }
    (line, "")
}

fn expanduser(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

/// Placeholder command used to keep detached panes alive.
pub fn pane_placeholder_cmd() -> &'static str {
    "while :; do sleep 3600; done"
}

/// Placeholder argv for tmux new-session.
pub fn pane_placeholder_argv() -> Vec<String> {
    vec![
        "sh".to_string(),
        "-lc".to_string(),
        pane_placeholder_cmd().to_string(),
    ]
}

/// Convert a tmux target into a normalized user option name prefixed with `@`.
pub fn normalize_user_option(name: &str) -> String {
    let opt = name.trim();
    if opt.is_empty() {
        return String::new();
    }
    if opt.starts_with('@') {
        opt.to_string()
    } else {
        format!("@{opt}")
    }
}

/// Interpret pane existence stdout.
pub fn pane_exists_output(stdout: &str) -> bool {
    stdout.trim().starts_with('%')
}

/// Interpret pane pipe stdout.
pub fn pane_pipe_enabled(stdout: &str) -> bool {
    stdout.trim() == "1"
}

/// Interpret pane dead stdout (`0` means alive).
pub fn pane_is_alive(stdout: &str) -> bool {
    stdout.trim() == "0"
}

/// Check if env value indicates copy mode is active.
pub fn copy_mode_is_active(value: &str) -> bool {
    matches!(value.trim(), "1" | "on" | "yes")
}

/// Parse pane size string like "80x24".
pub fn parse_pane_size(pane_size: &str) -> (u32, u32) {
    let text = pane_size.trim().to_lowercase();
    if let Some((w, h)) = text.split_once('x') {
        if let (Ok(width), Ok(height)) = (w.parse(), h.parse()) {
            return (width, height);
        }
    }
    (0, 0)
}

/// Compute split length for a given percentage and direction.
pub fn split_length_for_percent(pane_size: &str, direction_norm: &str, percent: u32) -> u32 {
    let (width, height) = parse_pane_size(pane_size);
    let mut basis = if matches!(direction_norm, "left" | "right" | "horizontal") {
        width
    } else {
        height
    };
    if basis == 0 {
        basis = 100;
    }
    let length = (basis as f64 * (percent as f64 / 100.0)).round() as u32;
    let max_len = basis.saturating_sub(1).max(1);
    length.max(1).min(max_len)
}

/// Build a split-window command argument list.
pub fn split_window_command(
    parent_pane_id: &str,
    flag: &str,
    split_length: u32,
    cmd: Option<&str>,
    cwd: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "split-window".to_string(),
        flag.to_string(),
        "-l".to_string(),
        split_length.to_string(),
        "-t".to_string(),
        parent_pane_id.to_string(),
    ];
    let start_dir = cwd.unwrap_or("").trim();
    if !start_dir.is_empty() {
        args.push("-c".to_string());
        args.push(start_dir.to_string());
    }
    args.push("-P".to_string());
    args.push("-F".to_string());
    args.push("#{pane_id}".to_string());
    let command = cmd.unwrap_or("").trim();
    if !command.is_empty() {
        args.push("sh".to_string());
        args.push("-lc".to_string());
        args.push(command.to_string());
    }
    args
}

/// Build an error text for a failed split-window command.
pub fn split_window_error_text(
    stderr: &str,
    stdout: &str,
    returncode: i32,
    parent_pane_id: &str,
    pane_size: &str,
    direction_norm: &str,
) -> String {
    let msg = stderr
        .trim()
        .is_empty()
        .then(|| stdout.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("no stdout/stderr");
    format!(
        "tmux split-window failed (exit {returncode}): {msg}\n\
         Pane: {parent_pane_id}, size: {pane_size}, direction: {direction_norm}\n\
         Hint: If the pane is zoomed, press Prefix+z to unzoom; also try enlarging terminal window."
    )
}

/// Parse a session name from stdout.
pub fn parse_session_name(stdout: &str) -> String {
    stdout.trim().to_string()
}

/// Check if selected pane should attach based on `TMUX` env.
pub fn should_attach_selected_pane(env_tmux: &str) -> bool {
    env_tmux.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_base_includes_socket_when_present() {
        std::env::remove_var("CCBR_TMUX_CONFIG");
        assert_eq!(tmux_base(None, None), vec!["tmux", "-f", "/dev/null"]);
        assert_eq!(
            tmux_base(Some("ccbr-demo"), None),
            vec!["tmux", "-f", "/dev/null", "-L", "ccbr-demo"]
        );
        let expanded = expanduser("~/.tmux/demo.sock");
        assert_eq!(
            tmux_base(Some("ccbr-demo"), Some("~/.tmux/demo.sock")),
            vec!["tmux", "-f", "/dev/null", "-S", expanded.as_str()]
        );
    }

    #[test]
    fn test_tmux_base_allows_managed_config_override() {
        std::env::set_var("CCBR_TMUX_CONFIG", "~/.config/ccb/tmux.conf");
        let expanded = expanduser("~/.config/ccb/tmux.conf");
        assert_eq!(
            tmux_base(Some("ccbr-demo"), None),
            vec!["tmux", "-f", expanded.as_str(), "-L", "ccbr-demo"]
        );
        std::env::remove_var("CCBR_TMUX_CONFIG");
    }

    #[test]
    fn test_tmux_target_helpers() {
        assert!(looks_like_pane_id("%1"));
        assert!(!looks_like_pane_id("sess"));
        assert!(looks_like_tmux_target("%1"));
        assert!(looks_like_tmux_target("sess:1.0"));
        assert!(!looks_like_tmux_target("sess"));
    }

    #[test]
    fn test_tmux_socket_name_helpers() {
        assert_eq!(normalize_socket_name(None), None);
        assert_eq!(normalize_socket_name(Some("")), None);
        assert_eq!(normalize_socket_name(Some("default")), None);
        assert_eq!(normalize_socket_name(Some("ccb")), Some("ccb".to_string()));
        assert_eq!(socket_name_from_tmux_env(None), None);
        assert_eq!(socket_name_from_tmux_env(Some("")), None);
        assert_eq!(
            socket_name_from_tmux_env(Some("/tmp/tmux-1000/default,123,0")),
            None
        );
        assert_eq!(
            socket_name_from_tmux_env(Some("/tmp/tmux-1000/ccb,123,0")),
            Some("ccb".to_string())
        );
    }

    #[test]
    fn test_normalize_split_direction() {
        assert_eq!(normalize_split_direction("right"), ("-h", "right"));
        assert_eq!(normalize_split_direction("vertical"), ("-v", "bottom"));
    }

    #[test]
    #[should_panic(expected = "unsupported direction")]
    fn test_normalize_split_direction_left_panics() {
        normalize_split_direction("left");
    }

    #[test]
    fn test_pane_id_by_title_marker_output_parses_list_panes() {
        let stdout = "%1\tCCB-a\n%2\tOTHER\n";
        assert_eq!(
            pane_id_by_title_marker_output(stdout, "CCB"),
            Some("%1".to_string())
        );
        assert_eq!(pane_id_by_title_marker_output(stdout, "missing"), None);
    }

    #[test]
    fn test_pane_id_by_title_marker_output_rejects_ambiguous_prefix_matches() {
        let stdout = "%1\tCCB-codex-a1b2c3d4\n%2\tCCB-codex-e5f6g7h8\n";
        assert_eq!(pane_id_by_title_marker_output(stdout, "CCB-codex"), None);
    }

    #[test]
    fn test_pane_id_by_title_marker_output_prefers_unique_exact_match() {
        let stdout = "%1\tCCB-codex\n%2\tCCB-codex-a1b2c3d4\n";
        assert_eq!(
            pane_id_by_title_marker_output(stdout, "CCB-codex"),
            Some("%1".to_string())
        );
    }

    #[test]
    fn test_default_detached_session_name_is_stable_format() {
        let name = default_detached_session_name("/tmp/demo", 123, 1700000000.0);
        assert_eq!(name, "ccbr-demo-0-123");
    }

    #[test]
    fn test_attach_helpers() {
        assert_eq!(normalize_user_option("ccbr_agent"), "@ccbr_agent");
        assert_eq!(normalize_user_option("@keep"), "@keep");
        assert_eq!(normalize_user_option(""), "");
        assert!(pane_exists_output("%12\n"));
        assert!(!pane_exists_output("12\n"));
        assert!(pane_pipe_enabled("1\n"));
        assert!(!pane_pipe_enabled("0\n"));
        assert!(pane_is_alive("0\n"));
        assert!(!pane_is_alive("1\n"));
        assert_eq!(parse_session_name(" demo \n"), "demo");
        assert!(should_attach_selected_pane(""));
        assert!(!should_attach_selected_pane("/tmp/tmux"));
    }

    #[test]
    fn test_split_length_for_percent() {
        assert_eq!(split_length_for_percent("80x24", "right", 50), 40);
        assert_eq!(split_length_for_percent("80x24", "bottom", 50), 12);
        assert_eq!(split_length_for_percent("", "right", 50), 50);
    }

    #[test]
    fn test_split_window_command() {
        let args = split_window_command("%0", "-h", 40, Some("vim"), Some("/tmp"));
        assert_eq!(
            args,
            vec![
                "split-window",
                "-h",
                "-l",
                "40",
                "-t",
                "%0",
                "-c",
                "/tmp",
                "-P",
                "-F",
                "#{pane_id}",
                "sh",
                "-lc",
                "vim"
            ]
        );
    }

    #[test]
    fn test_collect_pane_title_matches() {
        let stdout = "%1\tCCB-a\n%2\tCCB-b\n%3\tCCB\n";
        let (exact, prefix) = collect_pane_title_matches(stdout, "CCB");
        assert_eq!(exact, vec!["%3"]);
        assert_eq!(prefix, vec!["%1", "%2"]);
    }

    #[test]
    fn test_record_pane_title_match_buckets() {
        let mut exact = Vec::new();
        let mut prefix = Vec::new();
        record_pane_title_match("%1", "CCB", "CCB", &mut exact, &mut prefix);
        record_pane_title_match("%2", "CCB-codex", "CCB", &mut exact, &mut prefix);
        assert_eq!(exact, vec!["%1"]);
        assert_eq!(prefix, vec!["%2"]);
    }

    #[test]
    fn test_select_marker_match_prefers_unique_exact() {
        assert_eq!(
            select_marker_match(&["%1".to_string()], &[]),
            Some("%1".to_string())
        );
        assert_eq!(
            select_marker_match(&["%1".to_string(), "%2".to_string()], &[]),
            None
        );
        assert_eq!(
            select_marker_match(&[], &["%3".to_string()]),
            Some("%3".to_string())
        );
        assert_eq!(select_marker_match(&[] as &[String], &[]), None);
    }

    #[test]
    fn test_normalized_marker_trims() {
        assert_eq!(normalized_marker("  CCB  "), "CCB");
        assert_eq!(normalized_marker(""), "");
    }

    #[test]
    fn test_parse_pane_title_line() {
        assert_eq!(
            parse_pane_title_line("%1\tCCB-a"),
            Some(("%1".to_string(), "CCB-a".to_string()))
        );
        assert_eq!(
            parse_pane_title_line("%1 CCB-a"),
            Some(("%1".to_string(), "CCB-a".to_string()))
        );
        assert_eq!(parse_pane_title_line("notapane CCB-a"), None);
        assert_eq!(parse_pane_title_line(""), None);
    }

    #[test]
    fn test_split_pane_title_line() {
        assert_eq!(split_pane_title_line("%1\tCCB-a"), ("%1", "CCB-a"));
        assert_eq!(split_pane_title_line("%1 CCB-a"), ("%1", "CCB-a"));
        assert_eq!(split_pane_title_line("%1"), ("%1", ""));
    }
}
