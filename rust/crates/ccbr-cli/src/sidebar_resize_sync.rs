//! Mirrors Python `lib/cli/sidebar_resize_sync.py`.

use std::path::PathBuf;

/// Parsed arguments for the sidebar resize sync command.
pub struct SidebarResizeSync {
    pub tmux_socket_path: PathBuf,
    pub session_name: String,
    pub source_pane: String,
    pub source_window: String,
    pub project_id: String,
    pub from_stored_width: bool,
}

/// A single tmux pane record parsed from `list-panes`.
pub struct PaneRecord {
    pub session_name: String,
    pub window_id: String,
    pub window_name: String,
    pub pane_id: String,
    pub pane_width: i32,
    pub window_width: i32,
    pub project_id: String,
    pub role: String,
    pub sidebar_instance: String,
    pub managed_by: String,
}

struct SyncGuard<'a, F: Fn(&[String]) -> Result<String, String>> {
    sync: &'a SidebarResizeSync,
    tmux_run: &'a F,
}

impl<'a, F: Fn(&[String]) -> Result<String, String>> Drop for SyncGuard<'a, F> {
    fn drop(&mut self) {
        set_session_sync_guard(self.sync, self.tmux_run, false);
    }
}

/// Synchronize sidebar widths across windows in a tmux session.
///
/// Mirrors Python `sync_sidebar_resize`. The optional `run_fn` lets tests
/// inject a fake tmux runner; when `None`, `tmux` is invoked directly with
/// `-S <tmux_socket_path>`.
#[allow(clippy::type_complexity)]
pub fn sync_sidebar_resize(
    sync: &SidebarResizeSync,
    run_fn: Option<&dyn Fn(&[String]) -> Result<String, String>>,
) -> Option<usize> {
    let tmux_run = |args: &[String]| -> Result<String, String> {
        if let Some(run) = run_fn {
            run(args)
        } else {
            run_tmux_command(sync, args)
        }
    };

    let panes = list_panes(sync, &tmux_run);
    let source = panes
        .iter()
        .find(|p| !sync.source_pane.is_empty() && p.pane_id == sync.source_pane);

    let mut project_id = sync.project_id.clone();
    if project_id.is_empty() {
        if let Some(s) = source {
            project_id = s.project_id.clone();
        }
    }

    let source_sidebar = if let Some(s) = source {
        source_window_sidebar(&panes, s, &project_id)
    } else {
        source_window_sidebar_by_window(
            &panes,
            &sync.session_name,
            &sync.source_window,
            &project_id,
        )
    };

    if project_id.is_empty() {
        if let Some(sb) = source_sidebar {
            project_id = sb.project_id.clone();
        }
    }

    let target_width = if sync.from_stored_width {
        let stored = session_sidebar_width(sync, &tmux_run);
        if stored <= 0 {
            source_sidebar.map(|s| s.pane_width).unwrap_or(0)
        } else {
            stored
        }
    } else {
        source_sidebar.map(|s| s.pane_width)?
    };

    if target_width <= 0 {
        return None;
    }

    if !sync.from_stored_width {
        set_session_sidebar_width(sync, &tmux_run, target_width);
    }

    let mut resize_count = 0;
    set_session_sync_guard(sync, &tmux_run, true);
    let _sync_guard = SyncGuard {
        sync,
        tmux_run: &tmux_run,
    };
    for pane in sidebar_candidates(&panes, &sync.session_name, &project_id) {
        let clamped_width = clamp_sidebar_width(target_width, pane.window_width);
        if clamped_width <= 0 || pane.pane_width == clamped_width {
            continue;
        }
        if tmux_run(&[
            "resize-pane".to_string(),
            "-t".to_string(),
            pane.pane_id.clone(),
            "-x".to_string(),
            clamped_width.to_string(),
        ])
        .is_ok()
        {
            resize_count += 1;
        }
    }

    Some(resize_count)
}

/// Parse and possibly handle a `__sidebar-resize-sync` internal command.
///
/// Returns `Ok(Some(0))` when the command was handled, `Ok(None)` when the
/// first token is not `__sidebar-resize-sync`, and `Err(...)` for parse
/// failures.
pub fn maybe_handle_sidebar_resize_sync_command(args: &[String]) -> Result<Option<i32>, String> {
    if args.is_empty() || args[0] != "__sidebar-resize-sync" {
        return Ok(None);
    }
    let sync = parse_sidebar_resize_sync(&args[1..])?;
    let run_fn =
        |tmux_args: &[String]| -> Result<String, String> { run_tmux_command(&sync, tmux_args) };
    sync_sidebar_resize(&sync, Some(&run_fn));
    Ok(Some(0))
}

fn sidebar_candidates<'a>(
    panes: &'a [PaneRecord],
    session_name: &str,
    project_id: &str,
) -> Vec<&'a PaneRecord> {
    panes
        .iter()
        .filter(|p| {
            p.session_name == session_name
                && p.role == "sidebar"
                && p.managed_by == "ccbd"
                && (project_id.is_empty() || p.project_id == project_id)
        })
        .collect()
}

fn source_window_sidebar<'a>(
    panes: &'a [PaneRecord],
    source: &'a PaneRecord,
    project_id: &str,
) -> Option<&'a PaneRecord> {
    let sidebars = sidebar_candidates(panes, &source.session_name, project_id);

    if let Some(pane) = sidebars
        .iter()
        .find(|p| !p.window_id.is_empty() && p.window_id == source.window_id)
    {
        return Some(pane);
    }
    if let Some(pane) = sidebars
        .iter()
        .find(|p| !p.sidebar_instance.is_empty() && p.sidebar_instance == source.window_name)
    {
        return Some(pane);
    }
    if source.role == "sidebar" && source.managed_by == "ccbd" {
        Some(source)
    } else {
        None
    }
}

fn source_window_sidebar_by_window<'a>(
    panes: &'a [PaneRecord],
    session_name: &str,
    source_window: &str,
    project_id: &str,
) -> Option<&'a PaneRecord> {
    let token = source_window.trim();
    if token.is_empty() {
        return None;
    }
    let sidebars = sidebar_candidates(panes, session_name, project_id);

    if let Some(pane) = sidebars
        .iter()
        .find(|p| !p.window_id.is_empty() && p.window_id == token)
    {
        return Some(pane);
    }
    if let Some(pane) = sidebars
        .iter()
        .find(|p| !p.window_name.is_empty() && p.window_name == token)
    {
        return Some(pane);
    }
    if let Some(pane) = sidebars
        .iter()
        .find(|p| !p.sidebar_instance.is_empty() && p.sidebar_instance == token)
    {
        return Some(pane);
    }
    None
}

fn list_panes<F>(sync: &SidebarResizeSync, tmux_run: &F) -> Vec<PaneRecord>
where
    F: Fn(&[String]) -> Result<String, String>,
{
    let fmt = [
        "#{session_name}",
        "#{window_id}",
        "#{window_name}",
        "#{pane_id}",
        "#{pane_width}",
        "#{window_width}",
        "#{@ccbr_project_id}",
        "#{@ccbr_role}",
        "#{@ccbr_sidebar_instance}",
        "#{@ccbr_managed_by}",
    ]
    .join("\t");

    let output = match tmux_run(&[
        "list-panes".to_string(),
        "-a".to_string(),
        "-F".to_string(),
        fmt,
    ]) {
        Ok(out) => out,
        Err(_) => return Vec::new(),
    };

    let mut records = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').map(|s| s.trim()).collect();
        if parts.len() != 10 {
            continue;
        }
        let session_name = parts[0].to_string();
        if session_name != sync.session_name {
            continue;
        }
        let pane_id = parts[3].to_string();
        if !pane_id.starts_with('%') {
            continue;
        }
        records.push(PaneRecord {
            session_name,
            window_id: parts[1].to_string(),
            window_name: parts[2].to_string(),
            pane_id,
            pane_width: positive_int(parts[4]),
            window_width: positive_int(parts[5]),
            project_id: parts[6].to_string(),
            role: parts[7].to_string(),
            sidebar_instance: parts[8].to_string(),
            managed_by: parts[9].to_string(),
        });
    }
    records
}

fn session_sidebar_width<F>(sync: &SidebarResizeSync, tmux_run: &F) -> i32
where
    F: Fn(&[String]) -> Result<String, String>,
{
    let output = tmux_run(&[
        "show-option".to_string(),
        "-qv".to_string(),
        "-t".to_string(),
        sync.session_name.clone(),
        "@ccb_sidebar_width_cells".to_string(),
    ]);
    match output {
        Ok(out) => out.lines().next().map(positive_int).unwrap_or(0),
        Err(_) => 0,
    }
}

fn set_session_sidebar_width<F>(sync: &SidebarResizeSync, tmux_run: &F, width: i32)
where
    F: Fn(&[String]) -> Result<String, String>,
{
    let _ = tmux_run(&[
        "set-option".to_string(),
        "-t".to_string(),
        sync.session_name.clone(),
        "@ccb_sidebar_width_cells".to_string(),
        width.max(1).to_string(),
    ]);
}

fn set_session_sync_guard<F>(sync: &SidebarResizeSync, tmux_run: &F, enabled: bool)
where
    F: Fn(&[String]) -> Result<String, String>,
{
    if enabled {
        let _ = tmux_run(&[
            "set-option".to_string(),
            "-t".to_string(),
            sync.session_name.clone(),
            "@ccb_sidebar_sync_guard".to_string(),
            "1".to_string(),
        ]);
    } else {
        let _ = tmux_run(&[
            "set-option".to_string(),
            "-u".to_string(),
            "-t".to_string(),
            sync.session_name.clone(),
            "@ccb_sidebar_sync_guard".to_string(),
        ]);
    }
}

fn clamp_sidebar_width(width: i32, window_width: i32) -> i32 {
    if window_width <= 0 {
        return width.max(1);
    }
    let min_user_width = if window_width > 20 { 10 } else { 1 };
    let upper = (window_width - min_user_width).max(1);
    width.min(upper).max(1)
}

fn run_tmux_command(sync: &SidebarResizeSync, args: &[String]) -> Result<String, String> {
    let output = std::process::Command::new("tmux")
        .arg("-S")
        .arg(sync.tmux_socket_path.as_os_str())
        .args(args)
        .output()
        .map_err(|e| format!("tmux failed: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn parse_sidebar_resize_sync(args: &[String]) -> Result<SidebarResizeSync, String> {
    let mut tmux_socket: Option<String> = None;
    let mut session: Option<String> = None;
    let mut source_pane = String::new();
    let mut source_window = String::new();
    let mut project_id = String::new();
    let mut from_stored_width = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--from-stored-width" {
            from_stored_width = true;
            continue;
        }

        let (key, value) = if let Some(eq) = arg.find('=') {
            (arg[..eq].to_string(), arg[eq + 1..].to_string())
        } else {
            let key = arg.to_string();
            let value = iter
                .next()
                .ok_or_else(|| format!("missing value for {}", key))?
                .to_string();
            (key, value)
        };

        match key.as_str() {
            "--tmux-socket" => tmux_socket = Some(value),
            "--session" => session = Some(value),
            "--source-pane" => source_pane = value,
            "--source-window" => source_window = value,
            "--project-id" => project_id = value,
            _ => return Err(format!("unknown argument: {}", key)),
        }
    }

    Ok(SidebarResizeSync {
        tmux_socket_path: PathBuf::from(tmux_socket.ok_or("missing --tmux-socket")?),
        session_name: session.ok_or("missing --session")?,
        source_pane,
        source_window,
        project_id,
        from_stored_width,
    })
}

fn positive_int(value: &str) -> i32 {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .map(|v| v.max(0))
        .unwrap_or(0)
}
