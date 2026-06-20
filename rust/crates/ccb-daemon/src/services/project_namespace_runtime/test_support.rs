//! Test-only fake tmux backend for namespace runtime tests.
//!
//! Mirrors the behavior of Python `test_v2_project_namespace_state._FakeTmuxBackend`
//! enough to exercise the controller and ensure logic without a real tmux server.

use std::collections::HashMap;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::sync::{Arc, Mutex};

use ccb_terminal::{TmuxBackend, TmuxOutput};

use super::backend::{Backend, BackendFactory};

fn exit_status(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

fn success(stdout: impl Into<String>) -> TmuxOutput {
    TmuxOutput {
        stdout: stdout.into(),
        stderr: String::new(),
        status: exit_status(0),
    }
}

fn failure(stderr: impl Into<String>) -> TmuxOutput {
    TmuxOutput {
        stdout: String::new(),
        stderr: stderr.into(),
        status: exit_status(1),
    }
}

fn strip_tmux_base(args: Vec<String>) -> Vec<String> {
    let mut iter = args.into_iter().peekable();
    if iter.peek().map(|s| s.as_str()) == Some("tmux") {
        iter.next();
    }
    let mut result = Vec::new();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-S" | "-L" | "-f" => {
                iter.next();
            }
            _ => {
                result.push(arg);
            }
        }
    }
    result
}

#[derive(Debug, Clone, Default)]
pub struct Window {
    #[allow(dead_code)]
    id: String,
    pub(crate) name: String,
    #[allow(dead_code)]
    width: i32,
    pub(crate) panes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Pane {
    #[allow(dead_code)]
    id: String,
    pub(crate) title: String,
    pub(crate) options: HashMap<String, String>,
    #[allow(dead_code)]
    width: i32,
    #[allow(dead_code)]
    session: String,
    #[allow(dead_code)]
    window: String,
}

#[derive(Debug, Default)]
pub struct FakeTmuxState {
    pub sessions: HashMap<String, Vec<Window>>,
    pub active_windows: HashMap<String, String>,
    pub panes: HashMap<String, Pane>,
    pub pane_widths: HashMap<String, i32>,
    pub pane_titles: HashMap<String, String>,
    pub pane_options: HashMap<String, HashMap<String, String>>,
    pub session_options: HashMap<String, HashMap<String, String>>,
    pub window_options: HashMap<String, HashMap<String, String>>,
    pub hooks: HashMap<String, HashMap<String, String>>,
    pub tmux_calls: Vec<(Vec<String>, bool)>,
    pub split_calls: Vec<(String, String, i32)>,
    pane_counter: usize,
    window_counter: usize,
    pub server_killed: bool,
}

impl FakeTmuxState {
    fn alloc_pane(&mut self) -> String {
        self.pane_counter += 1;
        let id = format!("%{}", self.pane_counter);
        self.panes.insert(
            id.clone(),
            Pane {
                id: id.clone(),
                width: 160,
                ..Default::default()
            },
        );
        self.pane_widths.insert(id.clone(), 160);
        id
    }

    fn alloc_window(&mut self) -> String {
        self.window_counter += 1;
        format!("@{}", self.window_counter)
    }

    fn session_windows(&mut self, session: &str) -> &mut Vec<Window> {
        self.sessions.entry(session.to_string()).or_default()
    }

    fn create_window(&mut self, session: &str, name: &str) -> Window {
        let pane_id = self.alloc_pane();
        let window = Window {
            id: self.alloc_window(),
            name: name.to_string(),
            width: 160,
            panes: vec![pane_id.clone()],
        };
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.session = session.to_string();
            pane.window = name.to_string();
        }
        self.session_windows(session).push(window.clone());
        self.active_windows
            .entry(session.to_string())
            .or_insert_with(|| name.to_string());
        window
    }

    fn find_window(&self, target: &str) -> Option<(String, Window)> {
        let (session, maybe_window) = target.split_once(':').unwrap_or((target, ""));
        let windows = self.sessions.get(session)?;
        let window = if maybe_window.is_empty() {
            let active = self.active_windows.get(session)?;
            windows.iter().find(|w| w.name == *active).cloned()
        } else {
            windows
                .iter()
                .find(|w| w.name == maybe_window || w.id == maybe_window)
                .cloned()
        };
        window.map(|w| (session.to_string(), w))
    }

    fn pane_window(&self, pane_id: &str) -> Option<(String, Window)> {
        for (session, windows) in &self.sessions {
            for window in windows {
                if window.panes.contains(&pane_id.to_string()) {
                    return Some((session.clone(), window.clone()));
                }
            }
        }
        None
    }

    fn kill_server(&mut self) {
        self.sessions.clear();
        self.active_windows.clear();
        self.server_killed = true;
    }

    fn drop_session(&mut self, session: &str) {
        self.sessions.remove(session);
        self.active_windows.remove(session);
    }

    fn render_format(
        &self,
        session_name: &str,
        window: &Window,
        pane_id: &str,
        fmt: &str,
    ) -> String {
        let pane = self.panes.get(pane_id).cloned().unwrap_or_default();
        let active_window = self.active_windows.get(session_name);
        let active_pane = active_window
            .and_then(|_name| window.panes.first())
            .map(|s| s.as_str())
            .unwrap_or("");
        let mut rendered = fmt.to_string();
        rendered = rendered.replace("#{session_name}", session_name);
        rendered = rendered.replace("#{window_name}", &window.name);
        rendered = rendered.replace("#{window_id}", &window.id);
        rendered = rendered.replace("#{window_width}", &window.width.to_string());
        rendered = rendered.replace("#{pane_id}", pane_id);
        rendered = rendered.replace("#{pane_title}", &pane.title);
        rendered = rendered.replace(
            "#{pane_width}",
            &self
                .pane_widths
                .get(pane_id)
                .map(|w| w.to_string())
                .unwrap_or_else(|| window.width.to_string()),
        );
        rendered = rendered.replace(
            "#{pane_active}",
            if pane_id == active_pane { "1" } else { "0" },
        );
        rendered = rendered.replace("#{pane_dead}", "0");
        rendered = rendered.replace(
            "#{window_active}",
            if active_window == Some(&window.name) {
                "1"
            } else {
                "0"
            },
        );
        rendered = rendered.replace(
            "#{window_active_flag}",
            if active_window == Some(&window.name) {
                "1"
            } else {
                "0"
            },
        );
        if let Some(opts) = self.pane_options.get(pane_id) {
            for (key, value) in opts {
                rendered = rendered.replace(&format!("#{{{key}}}"), value);
            }
        }
        rendered
    }

    fn handle(&mut self, args: Vec<String>, check: bool, capture: bool) -> TmuxOutput {
        self.tmux_calls.push((args.clone(), capture));
        let args = strip_tmux_base(args);

        if args.is_empty() {
            return success("");
        }

        if args[0] == "start-server" {
            return success("");
        }

        if args.starts_with(&["set-option".to_string(), "-g".to_string()]) {
            return success("");
        }

        if args.starts_with(&["set-environment".to_string(), "-g".to_string()]) {
            return success("");
        }

        if args[0] == "bind-key" {
            return success("");
        }

        if args.starts_with(&["has-session".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let session = &args[2];
            return if self.sessions.contains_key(session) {
                success("")
            } else {
                failure("session not found")
            };
        }

        if args.starts_with(&["new-session".to_string(), "-d".to_string()]) {
            let mut session = String::new();
            let mut window = String::new();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-s" => {
                        session = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "-n" => {
                        window = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "-x" | "-y" | "-c" | "sh" | "-lc" => {
                        i += 1;
                    }
                    _ => {
                        if args[i].starts_with('-') && i + 1 < args.len() {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                }
            }
            if session.is_empty() {
                return failure("missing session name");
            }
            if window.is_empty() {
                window = session.clone();
            }
            self.drop_session(&session);
            self.create_window(&session, &window);
            return success("");
        }

        if args.starts_with(&["new-window".to_string(), "-d".to_string()]) {
            let mut session = String::new();
            let mut window = String::new();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-t" => {
                        session = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "-n" => {
                        window = args.get(i + 1).cloned().unwrap_or_default();
                        i += 2;
                    }
                    "-c" | "sh" | "-lc" => {
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            if session.contains(':') {
                session = session.split(':').next().unwrap_or(&session).to_string();
            }
            if session.is_empty() || window.is_empty() {
                return failure("missing session or window name");
            }
            self.create_window(&session, &window);
            return success("");
        }

        if args.starts_with(&["list-windows".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let session = &args[2];
            let fmt = if args.len() >= 5 && args[3] == "-F" {
                &args[4]
            } else {
                ""
            };
            let windows = self.sessions.get(session).cloned().unwrap_or_default();
            let mut rows = Vec::new();
            for window in windows {
                let active = self.active_windows.get(session) == Some(&window.name);
                if fmt == "#{window_name}" {
                    rows.push(window.name.clone());
                } else {
                    rows.push(format!(
                        "{}\t{}\t{}",
                        window.id,
                        window.name,
                        if active { "1" } else { "0" }
                    ));
                }
            }
            return success(rows.join("\n"));
        }

        if args.starts_with(&["list-panes".to_string(), "-a".to_string()]) {
            let fmt = if let Some(pos) = args.iter().position(|a| a == "-F") {
                args.get(pos + 1).cloned().unwrap_or_default()
            } else {
                "#{pane_id}".to_string()
            };
            let mut rows = Vec::new();
            for (session, windows) in &self.sessions {
                for window in windows {
                    for pane_id in &window.panes {
                        rows.push(self.render_format(session, window, pane_id, &fmt));
                    }
                }
            }
            return success(rows.join("\n"));
        }

        if args.starts_with(&["list-panes".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let target = &args[2];
            let _window = self.find_window(target).map(|(_, w)| w);
            let fmt = if let Some(pos) = args.iter().position(|a| a == "-F") {
                args.get(pos + 1).cloned().unwrap_or_default()
            } else {
                "#{pane_id}".to_string()
            };
            let rows: Vec<String> = if let Some((session, window)) = self.find_window(target) {
                window
                    .panes
                    .iter()
                    .map(|pane_id| self.render_format(&session, &window, pane_id, &fmt))
                    .collect()
            } else {
                Vec::new()
            };

            if fmt == "#{?pane_active,#{pane_id},}" {
                let active = rows.into_iter().find(|s| !s.is_empty());
                return success(active.unwrap_or_default() + "\n");
            }
            return success(rows.join("\n"));
        }

        if args.starts_with(&["select-window".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let target = &args[2];
            if let Some((session, window)) = self.find_window(target) {
                self.active_windows.insert(session, window.name);
                return success("");
            }
            return failure("window not found");
        }

        if args.starts_with(&["select-pane".to_string(), "-t".to_string()])
            && args.len() >= 3
            && !args.contains(&"-T".to_string())
        {
            let target = &args[2];
            if let Some((session, mut window)) = self
                .pane_window(target)
                .or_else(|| self.find_window(target))
            {
                if window.panes.contains(&target.to_string()) {
                    window.panes.retain(|p| p != target);
                    window.panes.insert(0, target.to_string());
                    if let Some(windows) = self.sessions.get_mut(&session) {
                        for w in windows.iter_mut() {
                            if w.id == window.id {
                                *w = window.clone();
                                break;
                            }
                        }
                    }
                }
                return success("");
            }
            return failure("pane not found");
        }

        if args.first().map(|s| s.as_str()) == Some("select-pane") {
            if let Some(title_pos) = args.iter().position(|a| a == "-T") {
                let title = args.get(title_pos + 1).cloned().unwrap_or_default();
                let pane_id = args
                    .iter()
                    .position(|a| a == "-t")
                    .and_then(|i| args.get(i + 1).cloned());
                if let Some(pane_id) = pane_id {
                    if let Some(pane) = self.panes.get_mut(&pane_id) {
                        pane.title = title.clone();
                    }
                    self.pane_titles.insert(pane_id.clone(), title);
                    return success("");
                }
                return failure("missing pane target");
            }
        }

        if args.starts_with(&["split-window".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let target = &args[2];
            let mut direction = "right".to_string();
            let mut percent = 50;
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "-h" => direction = "right".to_string(),
                    "-v" => direction = "down".to_string(),
                    "-hb" => direction = "left".to_string(),
                    "-vb" => direction = "up".to_string(),
                    "-p" => {
                        percent = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(50);
                        i += 1;
                    }
                    "-c" | "sh" | "-lc" => {}
                    _ => {}
                }
                i += 1;
            }
            if let Some((_session, mut window)) = self
                .find_window(target)
                .or_else(|| self.pane_window(target))
            {
                let parent = if window.panes.contains(&target.to_string()) {
                    target.clone()
                } else {
                    window.panes.last().cloned().unwrap_or_default()
                };
                let pane_id = self.alloc_pane();
                window.panes.push(pane_id.clone());
                if let Some(pane) = self.panes.get_mut(&pane_id) {
                    pane.session = _session.clone();
                    pane.window = window.name.clone();
                }
                let parent_width = *self.pane_widths.get(&parent).unwrap_or(&160);
                if direction == "right" || direction == "left" {
                    let new_width =
                        (parent_width as f64 * (percent as f64 / 100.0)).round() as i32;
                    let new_width = new_width.clamp(1, parent_width - 1);
                    self.pane_widths.insert(pane_id.clone(), new_width);
                    self.pane_widths.insert(parent.clone(), parent_width - new_width);
                } else {
                    self.pane_widths.insert(pane_id.clone(), parent_width);
                }
                self.split_calls.push((parent, direction, percent));
                if let Some(windows) = self.sessions.get_mut(&_session) {
                    for w in windows.iter_mut() {
                        if w.id == window.id {
                            *w = window;
                            break;
                        }
                    }
                }
                return success(pane_id);
            }
            return failure("split target not found");
        }

        if args.starts_with(&["respawn-pane".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let pane_id = &args[2];
            if args.contains(&"-k".to_string()) {
                let cmd = args
                    .iter()
                    .position(|a| a == "-k")
                    .and_then(|i| args.get(i + 1).cloned())
                    .unwrap_or_default();
                self.pane_options
                    .entry(pane_id.clone())
                    .or_default()
                    .insert("@respawn_cmd".to_string(), cmd);
            }
            return success("");
        }

        if args.starts_with(&["set-option".to_string(), "-t".to_string()]) && args.len() >= 5 {
            let target = &args[2];
            let option = &args[3];
            let value = &args[4];
            let (session, window) = target.split_once(':').unwrap_or((target, ""));
            if window.is_empty() {
                self.session_options
                    .entry(session.to_string())
                    .or_default()
                    .insert(option.clone(), value.clone());
            } else {
                let key = format!("{}:{}", session, window);
                self.window_options
                    .entry(key)
                    .or_default()
                    .insert(option.clone(), value.clone());
            }
            return success("");
        }

        if args.starts_with(&["set-window-option".to_string(), "-t".to_string()])
            && args.len() >= 5
        {
            let target = &args[2];
            let option = &args[3];
            let value = &args[4];
            let (session, window) = target.split_once(':').unwrap_or((target, ""));
            let key = if window.is_empty() {
                format!(
                    "{}:{}",
                    session,
                    self.active_windows
                        .get(session)
                        .cloned()
                        .unwrap_or_default()
                )
            } else {
                target.clone()
            };
            self.window_options
                .entry(key)
                .or_default()
                .insert(option.clone(), value.clone());
            return success("");
        }

        if args.starts_with(&["set-option".to_string(), "-p".to_string()]) && args.len() >= 5 {
            let mut pane_id: Option<String> = None;
            let mut option: Option<String> = None;
            let mut value: Option<String> = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-t" => {
                        pane_id = args.get(i + 1).cloned();
                        i += 2;
                    }
                    "-p" => {
                        i += 1;
                    }
                    _ => {
                        if option.is_none() {
                            option = Some(args[i].clone());
                        } else if value.is_none() {
                            value = Some(args[i].clone());
                        }
                        i += 1;
                    }
                }
            }
            if let (Some(pane_id), Some(option), Some(value)) = (pane_id, option, value) {
                self.pane_options
                    .entry(pane_id.clone())
                    .or_default()
                    .insert(option, value);
                if let Some(pane) = self.panes.get_mut(&pane_id) {
                    pane.options = self.pane_options[&pane_id].clone();
                }
            }
            return success("");
        }

        if args.starts_with(&["set-hook".to_string(), "-t".to_string()]) && args.len() >= 5 {
            let session = &args[2];
            let hook = &args[3];
            let command = args[4..].join(" ");
            self.hooks
                .entry(session.clone())
                .or_default()
                .insert(hook.clone(), command);
            return success("");
        }

        if args.starts_with(&["kill-server".to_string()]) {
            self.kill_server();
            return success("");
        }

        if args.starts_with(&["kill-session".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let session = &args[2];
            self.drop_session(session);
            return success("");
        }

        if args.starts_with(&["kill-pane".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let pane_id = &args[2];
            self.panes.remove(pane_id);
            self.pane_options.remove(pane_id);
            self.pane_titles.remove(pane_id);
            self.pane_widths.remove(pane_id);
            for windows in self.sessions.values_mut() {
                for window in windows.iter_mut() {
                    window.panes.retain(|p| p != pane_id);
                }
            }
            return success("");
        }

        if args.starts_with(&["kill-window".to_string(), "-t".to_string()]) && args.len() >= 3 {
            let target = &args[2];
            if let Some((_session, window)) = self.find_window(target) {
                if let Some(windows) = self.sessions.get_mut(&_session) {
                    windows.retain(|w| w.id != window.id);
                }
            }
            return success("");
        }

        if args.starts_with(&["display-message".to_string(), "-p".to_string()]) && args.len() >= 5
        {
            let fmt = &args[args.len() - 1];
            let target = args
                .iter()
                .position(|a| a == "-t")
                .and_then(|i| args.get(i + 1).cloned())
                .unwrap_or_default();
            if fmt == "#{pane_id}" {
                if let Some((_session, window)) = self
                    .find_window(&target)
                    .or_else(|| self.pane_window(&target))
                {
                    let pane_id = window.panes.first().cloned().unwrap_or_default();
                    return success(pane_id);
                }
                if self.panes.contains_key(&target) {
                    return success(target);
                }
            }
            return success("");
        }

        if args.starts_with(&["show-option".to_string(), "-qv".to_string()]) && args.len() >= 5 {
            let target = &args[3];
            let option = &args[4];
            let (session, window) = target.split_once(':').unwrap_or((target, ""));
            let value = if window.is_empty() {
                self.session_options
                    .get(session)
                    .and_then(|m| m.get(option).cloned())
            } else {
                self.window_options
                    .get(target)
                    .and_then(|m| m.get(option).cloned())
            };
            return success(value.unwrap_or_default());
        }

        if args[0] == "send-keys" || args[0] == "select-layout" || args[0] == "break-pane" {
            return success("");
        }

        if check {
            return failure(format!("unhandled tmux command: {}", args.join(" ")));
        }
        success("")
    }
}

#[derive(Clone)]
pub struct FakeTmuxBackend {
    state: Arc<Mutex<FakeTmuxState>>,
}

impl FakeTmuxBackend {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeTmuxState::default())),
        }
    }

    pub fn state(&self) -> Arc<Mutex<FakeTmuxState>> {
        self.state.clone()
    }

    pub fn backend_factory(&self) -> BackendFactory {
        let state = self.state.clone();
        BackendFactory::new(move |socket_path| {
            let tmux = TmuxBackend::new(None, Some(socket_path.to_string())).with_runner({
                let state = state.clone();
                move |args, check, capture, _input, _timeout, _env| {
                    let mut guard = state.lock().unwrap();
                    Ok(guard.handle(args, check, capture))
                }
            });
            Ok(Backend::new(socket_path.to_string(), String::new(), tmux))
        })
    }
}

impl Default for FakeTmuxBackend {
    fn default() -> Self {
        Self::new()
    }
}
