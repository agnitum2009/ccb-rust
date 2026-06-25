use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::tmux;

/// Information about a tmux pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: String,
    pub pane_index: u32,
    pub pane_width: u32,
    pub pane_height: u32,
    pub pane_title: String,
    pub pane_pid: u32,
    pub window_name: String,
    pub session_name: String,
}

/// Parse output of `tmux list-panes -F ...` into pane info structs.
pub fn parse_list_panes(output: &str) -> Vec<PaneInfo> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 8 {
                Some(PaneInfo {
                    pane_id: parts[0].to_string(),
                    pane_index: parts[1].parse().unwrap_or(0),
                    pane_width: parts[2].parse().unwrap_or(0),
                    pane_height: parts[3].parse().unwrap_or(0),
                    pane_title: parts[4].to_string(),
                    pane_pid: parts[5].parse().unwrap_or(0),
                    window_name: parts[6].to_string(),
                    session_name: parts[7].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Output type returned by the tmux runner used by pane service.
#[derive(Debug, Clone)]
pub struct TmuxRunOutput {
    pub stdout: String,
    pub stderr: String,
    pub returncode: i32,
}

impl TmuxRunOutput {
    pub fn success(&self) -> bool {
        self.returncode == 0
    }
}

/// Trait for running tmux commands inside services.
pub trait TmuxRunner: Send + Sync {
    fn run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<TmuxRunOutput>;

    /// Run a tmux command with optional stdin bytes.
    ///
    /// The default implementation delegates to [`run`] and ignores input bytes;
    /// real backends should override this to pipe `input_bytes` into the child
    /// process (required for `load-buffer -` to function correctly).
    fn run_with_input(
        &self,
        args: &[&str],
        check: bool,
        capture: bool,
        _input_bytes: Option<&[u8]>,
    ) -> anyhow::Result<TmuxRunOutput> {
        self.run(args, check, capture)
    }
}

impl<F> TmuxRunner for F
where
    F: Fn(&[&str], bool, bool) -> anyhow::Result<TmuxRunOutput> + Send + Sync,
{
    fn run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<TmuxRunOutput> {
        (self)(args, check, capture)
    }
}

/// Service for querying and mutating tmux panes.
pub struct TmuxPaneService {
    tmux_run: Box<dyn TmuxRunner>,
}

impl TmuxPaneService {
    pub fn new<R>(tmux_run: R) -> Self
    where
        R: TmuxRunner + 'static,
    {
        Self {
            tmux_run: Box::new(tmux_run),
        }
    }

    pub fn pane_exists(&self, pane_id: &str) -> bool {
        if !tmux::looks_like_pane_id(pane_id) {
            return false;
        }
        let Ok(output) = self.tmux_run.run(
            &["display-message", "-p", "-t", pane_id, "#{pane_id}"],
            false,
            true,
        ) else {
            return false;
        };
        output.success() && tmux::pane_exists_output(&output.stdout)
    }

    pub fn get_current_pane_id(&self, env_pane: &str) -> anyhow::Result<String> {
        let env_pane = env_pane.trim();
        if tmux::looks_like_pane_id(env_pane) && self.pane_exists(env_pane) {
            return Ok(env_pane.to_string());
        }
        if let Some(pane_id) = self.current_pane_from_tmux() {
            return Ok(pane_id);
        }
        Err(anyhow::anyhow!("tmux current pane id not available"))
    }

    fn current_pane_from_tmux(&self) -> Option<String> {
        let Ok(output) = self
            .tmux_run
            .run(&["display-message", "-p", "#{pane_id}"], false, true)
        else {
            return None;
        };
        let out = output.stdout.trim();
        if tmux::looks_like_pane_id(out) && self.pane_exists(out) {
            Some(out.to_string())
        } else {
            None
        }
    }

    pub fn find_pane_by_title_marker(&self, marker: &str) -> Option<String> {
        let marker = marker.trim();
        if marker.is_empty() {
            return None;
        }
        let Ok(output) = self.tmux_run.run(
            &["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"],
            false,
            true,
        ) else {
            return None;
        };
        if !output.success() {
            return None;
        }
        tmux::pane_id_by_title_marker_output(&output.stdout, marker)
    }

    pub fn find_pane_by_user_options(&self, expected: &HashMap<String, String>) -> Option<String> {
        let matches = self.list_panes_by_user_options(expected);
        if matches.len() == 1 {
            matches.into_iter().next()
        } else {
            None
        }
    }

    pub fn list_panes_by_user_options(&self, expected: &HashMap<String, String>) -> Vec<String> {
        let normalized = normalize_expected_user_options(expected);
        if normalized.is_empty() {
            return Vec::new();
        }
        let Ok(output) = self.tmux_run.run(
            &["list-panes", "-a", "-F", &list_panes_format(&normalized)],
            false,
            true,
        ) else {
            return Vec::new();
        };
        if !output.success() {
            return Vec::new();
        }
        matching_pane_ids(&output.stdout, &normalized)
    }

    pub fn describe_pane(
        &self,
        pane_id: &str,
        user_options: &[String],
    ) -> Option<HashMap<String, String>> {
        if !tmux::looks_like_pane_id(pane_id) {
            return None;
        }
        let normalized = normalize_user_option_names(user_options);
        let format_parts = describe_pane_fields(&normalized);
        let Ok(output) = self.tmux_run.run(
            &[
                "display-message",
                "-p",
                "-t",
                pane_id,
                &format_parts.join("\t"),
            ],
            false,
            true,
        ) else {
            return None;
        };
        if !output.success() {
            return None;
        }
        describe_pane_output(&output.stdout, &normalized)
    }

    pub fn get_pane_content(&self, pane_id: &str, lines: usize) -> Option<String> {
        if pane_id.trim().is_empty() {
            return None;
        }
        let n = lines.max(1);
        let Ok(output) = self.tmux_run.run(
            &["capture-pane", "-t", pane_id, "-p", "-S", &format!("-{n}")],
            false,
            true,
        ) else {
            return None;
        };
        if !output.success() {
            return None;
        }
        Some(crate::backend::TmuxBackend::strip_ansi(&output.stdout))
    }

    pub fn is_pane_alive(&self, pane_id: &str) -> bool {
        if pane_id.trim().is_empty() {
            return false;
        }
        let Ok(output) = self.tmux_run.run(
            &["display-message", "-p", "-t", pane_id, "#{pane_dead}"],
            false,
            true,
        ) else {
            return false;
        };
        tmux::pane_is_alive(&output.stdout)
    }

    pub fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
        cmd: Option<&str>,
        cwd: Option<&str>,
    ) -> anyhow::Result<String> {
        if parent_pane_id.trim().is_empty() {
            return Err(anyhow::anyhow!("parent_pane_id is required"));
        }
        self.unzoom_parent_if_needed(parent_pane_id);
        if tmux::looks_like_pane_id(parent_pane_id) && !self.pane_exists(parent_pane_id) {
            return Err(anyhow::anyhow!(
                "Cannot split: pane {parent_pane_id} does not exist"
            ));
        }

        let pane_size = self.read_pane_size(parent_pane_id);
        let (flag, direction_norm) = tmux::normalize_split_direction(direction);
        let split_percent = percent.clamp(1, 99);
        let split_length =
            tmux::split_length_for_percent(&pane_size, direction_norm, split_percent);

        let args = tmux::split_window_command(parent_pane_id, flag, split_length, cmd, cwd);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.tmux_run.run(&args_ref, true, true).map_err(|e| {
            let msg = e.to_string();
            anyhow::anyhow!(tmux::split_window_error_text(
                &msg,
                "",
                -1,
                parent_pane_id,
                &pane_size,
                direction_norm,
            ))
        })?;

        if !output.success() {
            return Err(anyhow::anyhow!(tmux::split_window_error_text(
                &output.stderr,
                &output.stdout,
                output.returncode,
                parent_pane_id,
                &pane_size,
                direction_norm,
            )));
        }

        let pane_id = output.stdout.trim();
        if !tmux::looks_like_pane_id(pane_id) {
            return Err(anyhow::anyhow!(
                "tmux split-window did not return pane_id: {pane_id:?}"
            ));
        }
        Ok(pane_id.to_string())
    }

    pub fn set_pane_title(&self, pane_id: &str, title: &str) {
        if pane_id.trim().is_empty() {
            return;
        }
        let _ = self
            .tmux_run
            .run(&["select-pane", "-t", pane_id, "-T", title], false, true);
    }

    pub fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) {
        if pane_id.trim().is_empty() {
            return;
        }
        let opt = tmux::normalize_user_option(name);
        if opt.is_empty() {
            return;
        }
        let _ = self.tmux_run.run(
            &["set-option", "-p", "-t", pane_id, &opt, value],
            false,
            true,
        );
    }

    pub fn set_pane_style(
        &self,
        pane_id: &str,
        border_style: Option<&str>,
        active_border_style: Option<&str>,
    ) {
        if pane_id.trim().is_empty() {
            return;
        }
        self.set_pane_option(pane_id, "pane-border-style", border_style);
        self.set_pane_option(pane_id, "pane-active-border-style", active_border_style);
    }

    fn set_pane_option(&self, pane_id: &str, option: &str, value: Option<&str>) {
        let Some(value) = value else { return };
        if value.is_empty() {
            return;
        }
        let _ = self.tmux_run.run(
            &["set-option", "-p", "-t", pane_id, option, value],
            false,
            true,
        );
    }

    fn unzoom_parent_if_needed(&self, parent_pane_id: &str) {
        if !tmux::looks_like_pane_id(parent_pane_id) {
            return;
        }
        if self.pane_zoomed(parent_pane_id) {
            let _ = self
                .tmux_run
                .run(&["resize-pane", "-Z", "-t", parent_pane_id], false, false);
        }
    }

    fn read_pane_size(&self, parent_pane_id: &str) -> String {
        if let Ok(output) = self.tmux_run.run(
            &[
                "display-message",
                "-p",
                "-t",
                parent_pane_id,
                "#{pane_width}x#{pane_height}",
            ],
            false,
            true,
        ) {
            if output.success() {
                return output.stdout.trim().to_string();
            }
        }
        "unknown".to_string()
    }

    fn pane_zoomed(&self, parent_pane_id: &str) -> bool {
        let Ok(output) = self.tmux_run.run(
            &[
                "display-message",
                "-p",
                "-t",
                parent_pane_id,
                "#{window_zoomed_flag}",
            ],
            false,
            true,
        ) else {
            return false;
        };
        if !output.success() {
            return false;
        }
        matches!(output.stdout.trim(), "1" | "on" | "yes" | "true")
    }
}

fn normalize_expected_user_options(expected: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut normalized: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (name, value) in expected {
        let opt = tmux::normalize_user_option(name);
        let text = value.trim();
        if opt.is_empty() || text.is_empty() || seen.contains(&opt) {
            continue;
        }
        seen.insert(opt.clone());
        normalized.push((opt, text.to_string()));
    }
    normalized.sort_by(|a, b| a.0.cmp(&b.0));
    normalized
}

fn normalize_user_option_names(user_options: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for name in user_options {
        let opt = tmux::normalize_user_option(name);
        if opt.is_empty() || seen.contains(&opt) {
            continue;
        }
        seen.insert(opt.clone());
        normalized.push(opt);
    }
    normalized.sort();
    normalized
}

fn list_panes_format(normalized: &[(String, String)]) -> String {
    let mut parts = vec!["#{pane_id}".to_string()];
    parts.extend(normalized.iter().map(|(opt, _)| format!("#{{{opt}}}")));
    parts.join("\t")
}

fn matching_pane_ids(stdout: &str, normalized: &[(String, String)]) -> Vec<String> {
    let mut matches: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != normalized.len() + 1 {
            continue;
        }
        let pane_id = parts[0].trim();
        if !tmux::looks_like_pane_id(pane_id) {
            continue;
        }
        if pane_matches_expected(&parts, normalized) {
            matches.push(pane_id.to_string());
        }
    }
    matches
}

fn pane_matches_expected(parts: &[&str], normalized: &[(String, String)]) -> bool {
    for (index, (_, expected_value)) in normalized.iter().enumerate() {
        if parts.get(index + 1).unwrap_or(&"").trim() != expected_value {
            return false;
        }
    }
    true
}

fn describe_pane_fields(normalized_options: &[String]) -> Vec<String> {
    let mut parts = vec![
        "#{pane_id}".to_string(),
        "#{pane_title}".to_string(),
        "#{pane_dead}".to_string(),
    ];
    parts.extend(normalized_options.iter().map(|opt| format!("#{{{opt}}}")));
    parts
}

fn describe_pane_output(
    stdout: &str,
    normalized_options: &[String],
) -> Option<HashMap<String, String>> {
    let format_size = normalized_options.len() + 3;
    let line = stdout.lines().next().unwrap_or("");
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != format_size {
        return None;
    }
    let mut described = HashMap::new();
    described.insert("pane_id".to_string(), parts[0].trim().to_string());
    described.insert("pane_title".to_string(), parts[1].to_string());
    described.insert("pane_dead".to_string(), parts[2].trim().to_string());
    for (index, opt) in normalized_options.iter().enumerate() {
        described.insert(
            opt.clone(),
            parts.get(index + 3).unwrap_or(&"").trim().to_string(),
        );
    }
    Some(described)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(stdout: &str, returncode: i32) -> TmuxRunOutput {
        TmuxRunOutput {
            stdout: stdout.to_string(),
            stderr: String::new(),
            returncode,
        }
    }

    #[test]
    fn test_parse_list_panes() {
        let output =
            "%0\t0\t80\t24\tbash\t1234\tmain\tsession\n%1\t1\t80\t24\tbash\t1235\tmain\tsession";
        let panes = parse_list_panes(output);
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].pane_id, "%0");
        assert_eq!(panes[1].pane_id, "%1");
    }

    #[test]
    fn test_parse_empty() {
        let panes = parse_list_panes("");
        assert!(panes.is_empty());
    }

    #[test]
    fn test_pane_service_gets_current_pane_and_finds_marker() {
        let calls = std::sync::Mutex::new(Vec::<Vec<String>>::new());
        let service = TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                calls
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                if args == ["display-message", "-p", "-t", "%1", "#{pane_id}"] {
                    return Ok(cp("%1\n", 0));
                }
                if args == ["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"] {
                    return Ok(cp("%1\tCCBR-one\n%2\tOTHER\n", 0));
                }
                Ok(cp("%1\n", 0))
            },
        );

        assert_eq!(service.get_current_pane_id("%1").unwrap(), "%1");
        assert_eq!(service.find_pane_by_title_marker("CCBR").unwrap(), "%1");
    }

    #[test]
    fn test_pane_service_sets_user_option_and_reads_content() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let calls_clone = calls.clone();
        let service = TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                calls_clone
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                if args.len() >= 2 && args[0] == "capture-pane" && args[1] == "-t" {
                    return Ok(cp("\x1b[31mhello\x1b[0m\n", 0));
                }
                if args.len() >= 2
                    && args[0] == "display-message"
                    && args[1] == "-p"
                    && args.contains(&"#{pane_dead}")
                {
                    return Ok(cp("0\n", 0));
                }
                Ok(cp("", 0))
            },
        );

        service.set_pane_user_option("%3", "ccbr_agent", "Gemini");
        let text = service.get_pane_content("%3", 20);
        let alive = service.is_pane_alive("%3");

        assert_eq!(
            calls.lock().unwrap()[0],
            vec!["set-option", "-p", "-t", "%3", "@ccb_agent", "Gemini"]
        );
        assert_eq!(text, Some("hello\n".to_string()));
        assert!(alive);
    }

    #[test]
    fn test_pane_service_describes_pane_with_user_options() {
        let service = TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args == [
                    "display-message",
                    "-p",
                    "-t",
                    "%3",
                    "#{pane_id}\t#{pane_title}\t#{pane_dead}\t#{@ccbr_agent}\t#{@ccbr_project_id}",
                ] {
                    return Ok(cp("%3\tagent2\t0\tagent2\tproj-1\n", 0));
                }
                Ok(cp("", 1))
            },
        );

        let described =
            service.describe_pane("%3", &["@ccb_agent".into(), "@ccb_project_id".into()]);

        assert_eq!(
            described,
            Some(HashMap::from_iter([
                ("pane_id".to_string(), "%3".to_string()),
                ("pane_title".to_string(), "agent2".to_string()),
                ("pane_dead".to_string(), "0".to_string()),
                ("@ccb_agent".to_string(), "agent2".to_string()),
                ("@ccb_project_id".to_string(), "proj-1".to_string()),
            ]))
        );
    }

    #[test]
    fn test_pane_service_finds_unique_pane_by_user_options() {
        let service = TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args
                    == [
                        "list-panes",
                        "-a",
                        "-F",
                        "#{pane_id}\t#{@ccbr_agent}\t#{@ccbr_project_id}",
                    ]
                {
                    return Ok(cp("%1\tagent1\tproj-1\n%2\tagent1\tproj-2\n", 0));
                }
                Ok(cp("", 0))
            },
        );

        let mut expected = HashMap::new();
        expected.insert("ccbr_agent".to_string(), "agent1".to_string());
        expected.insert("ccbr_project_id".to_string(), "proj-2".to_string());
        assert_eq!(
            service.find_pane_by_user_options(&expected),
            Some("%2".to_string())
        );
    }

    #[test]
    fn test_pane_service_lists_matching_panes_by_user_options() {
        let service = TmuxPaneService::new(
            move |args: &[&str], _check: bool, _capture: bool| -> anyhow::Result<TmuxRunOutput> {
                if args == ["list-panes", "-a", "-F", "#{pane_id}\t#{@ccbr_project_id}"] {
                    return Ok(cp("%1\tproj-1\n%2\tproj-2\n%3\tproj-2\n", 0));
                }
                Ok(cp("", 0))
            },
        );

        let mut expected = HashMap::new();
        expected.insert("ccbr_project_id".to_string(), "proj-2".to_string());
        assert_eq!(
            service.list_panes_by_user_options(&expected),
            vec!["%2".to_string(), "%3".to_string()]
        );
    }
}
