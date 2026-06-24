#![allow(clippy::type_complexity)]

use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;

use crate::panes::TmuxRunner;
use crate::tmux;

/// Sanitize text before sending.
pub fn sanitize_text(text: &str) -> String {
    text.replace('\r', "").trim().to_string()
}

/// Decide whether to use inline legacy send.
pub fn should_use_inline_legacy_send(
    target_is_tmux: bool,
    text: &str,
    inline_limit: usize,
) -> bool {
    if target_is_tmux {
        return false;
    }
    !text.contains('\n') && text.len() <= inline_limit
}

/// Build a unique tmux buffer name.
pub fn build_buffer_name(pid: u32, now_ms: u64, rand_int: u32) -> String {
    format!("ccbr-tb-{pid}-{now_ms}-{rand_int}")
}

/// Sender for tmux text using buffers.
pub struct TmuxTextSender {
    tmux_run: Box<dyn TmuxRunner>,
    ensure_not_in_copy_mode: Box<dyn Fn(&str) + Send + Sync>,
    env_float: Box<dyn Fn(&str, f64) -> f64 + Send + Sync>,
}

impl TmuxTextSender {
    pub fn new<F, G>(
        tmux_run: Box<dyn TmuxRunner>,
        ensure_not_in_copy_mode: F,
        env_float: G,
    ) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
        G: Fn(&str, f64) -> f64 + Send + Sync + 'static,
    {
        Self {
            tmux_run,
            ensure_not_in_copy_mode: Box::new(ensure_not_in_copy_mode),
            env_float: Box::new(env_float),
        }
    }

    pub fn send_text(&self, pane_id: &str, text: &str) -> anyhow::Result<()> {
        let sanitized = sanitize_text(text);
        if sanitized.is_empty() {
            return Ok(());
        }

        let target_is_tmux = tmux::looks_like_tmux_target(pane_id);
        if !target_is_tmux {
            let session = pane_id;
            if should_use_inline_legacy_send(target_is_tmux, &sanitized, 200) {
                self.tmux_run
                    .run(&["send-keys", "-t", session, "-l", &sanitized], true, false)?;
                self.tmux_run
                    .run(&["send-keys", "-t", session, "Enter"], true, false)?;
                return Ok(());
            }
            self.paste_via_buffer(session, &sanitized, false)?;
            return Ok(());
        }

        (self.ensure_not_in_copy_mode)(pane_id);
        self.paste_via_buffer(pane_id, &sanitized, true)?;
        Ok(())
    }

    fn paste_via_buffer(&self, target: &str, text: &str, pane_target: bool) -> anyhow::Result<()> {
        let buffer_name = build_buffer_name(
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            rand::thread_rng().gen_range(1000..10000),
        );
        self.tmux_run.run_with_input(
            &["load-buffer", "-b", &buffer_name, "-"],
            true,
            false,
            Some(text.as_bytes()),
        )?;
        let result = if pane_target {
            self.tmux_run.run(
                &["paste-buffer", "-p", "-t", target, "-b", &buffer_name],
                true,
                false,
            )
        } else {
            self.tmux_run.run(
                &["paste-buffer", "-t", target, "-b", &buffer_name, "-p"],
                true,
                false,
            )
        };
        let enter_delay = (self.env_float)("CCB_TMUX_ENTER_DELAY", 0.5);
        if enter_delay > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(enter_delay));
        }
        let _ = self
            .tmux_run
            .run(&["send-keys", "-t", target, "Enter"], true, false);
        let _ = self
            .tmux_run
            .run(&["delete-buffer", "-b", &buffer_name], false, false);
        result?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::{TmuxRunOutput, TmuxRunner};

    fn ok() -> TmuxRunOutput {
        TmuxRunOutput {
            stdout: String::new(),
            stderr: String::new(),
            returncode: 0,
        }
    }

    #[test]
    fn test_sanitize_text_and_inline_legacy_send() {
        assert_eq!(sanitize_text(" hello\r\n"), "hello");
        assert!(should_use_inline_legacy_send(false, "hello", 200));
        assert!(!should_use_inline_legacy_send(false, "a\nb", 200));
        assert!(!should_use_inline_legacy_send(true, "hello", 200));
    }

    #[test]
    fn test_build_buffer_name_and_copy_mode() {
        assert_eq!(build_buffer_name(1, 2, 3), "ccbr-tb-1-2-3");
        assert!(tmux::copy_mode_is_active("1"));
        assert!(tmux::copy_mode_is_active("yes"));
        assert!(!tmux::copy_mode_is_active("0"));
    }

    #[test]
    fn test_tmux_text_sender_uses_inline_legacy_mode_for_session_targets() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<String>>::new()));
        let calls_clone = calls.clone();
        let runner: Box<dyn TmuxRunner> =
            Box::new(move |args: &[&str], _check: bool, _capture: bool| {
                calls_clone
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| s.to_string()).collect());
                Ok(ok())
            });
        let sender = TmuxTextSender::new(runner, |_pane_id| {}, |_name, default| default);

        sender.send_text("session-x", "hello").unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                vec!["send-keys", "-t", "session-x", "-l", "hello"],
                vec!["send-keys", "-t", "session-x", "Enter"],
            ]
        );
    }

    #[test]
    fn test_tmux_text_sender_deletes_buffer_after_paste_failure() {
        #[derive(Debug, Clone)]
        struct Capture {
            args: Vec<Vec<String>>,
        }
        struct CaptureRunner {
            capture: std::sync::Arc<std::sync::Mutex<Capture>>,
        }
        impl TmuxRunner for CaptureRunner {
            fn run(
                &self,
                args: &[&str],
                _check: bool,
                _capture: bool,
            ) -> anyhow::Result<TmuxRunOutput> {
                self.capture
                    .lock()
                    .unwrap()
                    .args
                    .push(args.iter().map(|s| s.to_string()).collect());
                if args[0] == "paste-buffer" {
                    return Err(anyhow::anyhow!("paste failed"));
                }
                Ok(ok())
            }
            fn run_with_input(
                &self,
                args: &[&str],
                _check: bool,
                _capture: bool,
                _input_bytes: Option<&[u8]>,
            ) -> anyhow::Result<TmuxRunOutput> {
                self.capture
                    .lock()
                    .unwrap()
                    .args
                    .push(args.iter().map(|s| s.to_string()).collect());
                Ok(ok())
            }
        }
        let capture = std::sync::Arc::new(std::sync::Mutex::new(Capture { args: Vec::new() }));
        let runner: Box<dyn TmuxRunner> = Box::new(CaptureRunner {
            capture: capture.clone(),
        });
        let sender = TmuxTextSender::new(runner, |_pane_id| {}, |_name, default| default);
        assert!(sender.send_text("%1", "hello").is_err());
        let cap = capture.lock().unwrap();
        assert_eq!(cap.args[0][0], "load-buffer");
        assert_eq!(cap.args[1][0], "paste-buffer");
        assert_eq!(cap.args[2], vec!["send-keys", "-t", "%1", "Enter"]);
        assert_eq!(cap.args[3][0], "delete-buffer");
    }

    #[test]
    fn test_tmux_text_sender_passes_text_to_load_buffer_for_pane_targets() {
        #[derive(Debug, Clone)]
        struct Capture {
            args: Vec<Vec<String>>,
            inputs: Vec<Option<Vec<u8>>>,
        }

        struct CaptureRunner {
            capture: std::sync::Arc<std::sync::Mutex<Capture>>,
        }

        impl TmuxRunner for CaptureRunner {
            fn run(
                &self,
                args: &[&str],
                _check: bool,
                _capture: bool,
            ) -> anyhow::Result<TmuxRunOutput> {
                self.capture
                    .lock()
                    .unwrap()
                    .args
                    .push(args.iter().map(|s| s.to_string()).collect());
                self.capture.lock().unwrap().inputs.push(None);
                Ok(ok())
            }

            fn run_with_input(
                &self,
                args: &[&str],
                _check: bool,
                _capture: bool,
                input_bytes: Option<&[u8]>,
            ) -> anyhow::Result<TmuxRunOutput> {
                self.capture
                    .lock()
                    .unwrap()
                    .args
                    .push(args.iter().map(|s| s.to_string()).collect());
                self.capture
                    .lock()
                    .unwrap()
                    .inputs
                    .push(input_bytes.map(|b| b.to_vec()));
                Ok(ok())
            }
        }

        let capture = std::sync::Arc::new(std::sync::Mutex::new(Capture {
            args: Vec::new(),
            inputs: Vec::new(),
        }));
        let runner: Box<dyn TmuxRunner> = Box::new(CaptureRunner {
            capture: capture.clone(),
        });
        let sender = TmuxTextSender::new(runner, |_pane_id| {}, |_name, default| default);

        sender.send_text("%1", "line one\nline two").unwrap();

        let cap = capture.lock().unwrap();
        assert_eq!(
            cap.args.len(),
            4,
            "expected load-buffer, paste-buffer, send-keys, delete-buffer"
        );
        assert_eq!(
            cap.args[0],
            vec!["load-buffer", "-b", cap.args[0][2].as_str(), "-"]
        );
        assert_eq!(
            cap.inputs[0]
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string()),
            Some("line one\nline two".to_string())
        );
        assert_eq!(cap.args[1][0], "paste-buffer");
        assert_eq!(cap.args[2], vec!["send-keys", "-t", "%1", "Enter"]);
        assert_eq!(
            cap.args[3],
            vec!["delete-buffer", "-b", cap.args[0][2].as_str()]
        );
    }
}
