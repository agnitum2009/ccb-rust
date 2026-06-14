use ccb_terminal::panes::{TmuxPaneService, TmuxRunOutput};
use ccb_terminal::TmuxBackend;

/// `TmuxLayoutBackend` adapter that delegates to `ccb_terminal::TmuxBackend`.
/// Used by the daemon start flow to create provider panes via the layout engine.
#[derive(Clone)]
pub struct DaemonLayoutBackend {
    backend: TmuxBackend,
}

impl DaemonLayoutBackend {
    pub fn new(socket_path: &str) -> Self {
        Self {
            backend: TmuxBackend::new(None, Some(socket_path.to_string())),
        }
    }

    fn run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<TmuxRunOutput> {
        let output = self
            .backend
            .tmux_run(args, check, capture, None, None)
            .map_err(|e| anyhow::anyhow!(e))?;
        if check && !output.success() {
            return Err(anyhow::anyhow!(
                "tmux command failed ({}): {}",
                output
                    .status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                output.stderr
            ));
        }
        Ok(TmuxRunOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            returncode: output.status.code().unwrap_or(-1),
        })
    }

    fn pane_service(&self) -> TmuxPaneService {
        let backend = self.backend.clone();
        TmuxPaneService::new(
            move |args: &[&str], check: bool, capture: bool| -> anyhow::Result<TmuxRunOutput> {
                let output = backend
                    .tmux_run(args, check, capture, None, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
                if check && !output.success() {
                    return Err(anyhow::anyhow!(
                        "tmux command failed ({}): {}",
                        output
                            .status
                            .code()
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        output.stderr
                    ));
                }
                Ok(TmuxRunOutput {
                    stdout: output.stdout,
                    stderr: output.stderr,
                    returncode: output.status.code().unwrap_or(-1),
                })
            },
        )
    }
}

impl ccb_terminal::layouts::TmuxLayoutBackend for DaemonLayoutBackend {
    fn get_current_pane_id(&self) -> anyhow::Result<String> {
        // The daemon runs outside tmux; always allocate a detached session.
        Err(anyhow::anyhow!("daemon runs outside tmux"))
    }

    fn is_alive(&self, target: &str) -> bool {
        let args: Vec<&str> = if target.starts_with('%') {
            vec!["display-message", "-p", "-t", target, "#{pane_id}"]
        } else {
            vec!["has-session", "-t", target]
        };
        self.run(&args, false, false)
            .map(|o| o.success())
            .unwrap_or(false)
    }

    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        direction: &str,
        percent: u32,
        parent_pane: Option<&str>,
    ) -> anyhow::Result<String> {
        self.pane_service().split_pane(
            parent_pane.unwrap_or(""),
            direction,
            percent,
            Some(cmd).filter(|c| !c.is_empty()),
            Some(cwd).filter(|c| !c.is_empty()),
        )
    }

    fn split_pane(
        &self,
        parent_pane_id: &str,
        direction: &str,
        percent: u32,
    ) -> anyhow::Result<String> {
        self.pane_service()
            .split_pane(parent_pane_id, direction, percent, None, None)
    }

    fn set_pane_title(&self, pane_id: &str, title: &str) {
        self.pane_service().set_pane_title(pane_id, title);
    }

    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) {
        self.pane_service()
            .set_pane_user_option(pane_id, name, value);
    }

    fn set_pane_style(
        &self,
        pane_id: &str,
        border_style: Option<&str>,
        active_border_style: Option<&str>,
    ) {
        self.pane_service()
            .set_pane_style(pane_id, border_style, active_border_style);
    }

    fn tmux_run(&self, args: &[&str], check: bool, capture: bool) -> anyhow::Result<String> {
        self.run(args, check, capture).map(|o| o.stdout)
    }
}
