//! Test support for `start_runtime` integration and unit tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ccbr_terminal::layouts::TmuxLayoutBackend;

use crate::provider_launcher::ProviderLauncher;
use crate::start_runtime::ensure_agent_runtime::EnsureAgentRuntimeImpl;

#[derive(Clone)]
pub struct FakeBackend {
    pub alive: Arc<Mutex<HashMap<String, bool>>>,
    pub calls: Arc<Mutex<Vec<String>>>,
    next_pane: Arc<Mutex<String>>,
    detached_pane: Arc<Mutex<String>>,
    pane_size: Arc<Mutex<(u32, u32)>>,
    create_pane_error: Arc<Mutex<Option<String>>>,
}

impl FakeBackend {
    pub fn new(next_pane: &str) -> Self {
        Self {
            alive: Arc::new(Mutex::new(HashMap::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
            next_pane: Arc::new(Mutex::new(next_pane.to_string())),
            detached_pane: Arc::new(Mutex::new(next_pane.to_string())),
            pane_size: Arc::new(Mutex::new((160, 48))),
            create_pane_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_detached_pane(self, pane: &str) -> Self {
        *self.detached_pane.lock().unwrap() = pane.to_string();
        self
    }

    pub fn with_pane_size(self, width: u32, height: u32) -> Self {
        *self.pane_size.lock().unwrap() = (width, height);
        self
    }

    pub fn with_create_pane_error(self, error: &str) -> Self {
        *self.create_pane_error.lock().unwrap() = Some(error.to_string());
        self
    }

    pub fn mark_alive(&self, pane_id: &str, alive: bool) {
        self.alive
            .lock()
            .unwrap()
            .insert(pane_id.to_string(), alive);
    }

    pub fn record(&self, call: &str) {
        self.calls.lock().unwrap().push(call.to_string());
    }

    pub fn has_call(&self, prefix: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with(prefix))
    }
}

impl TmuxLayoutBackend for FakeBackend {
    fn get_current_pane_id(&self) -> anyhow::Result<String> {
        Ok("%0".to_string())
    }

    fn is_alive(&self, pane_id: &str) -> bool {
        *self.alive.lock().unwrap().get(pane_id).unwrap_or(&false)
    }

    fn create_pane(
        &self,
        cmd: &str,
        cwd: &str,
        _direction: &str,
        _percent: u32,
        _parent_pane: Option<&str>,
    ) -> anyhow::Result<String> {
        self.record(&format!("create_pane:{cmd}:{cwd}"));
        if let Some(err) = self.create_pane_error.lock().unwrap().as_ref() {
            return Err(anyhow::anyhow!(err.clone()));
        }
        Ok(self.next_pane.lock().unwrap().clone())
    }

    fn split_pane(
        &self,
        _parent_pane_id: &str,
        _direction: &str,
        _percent: u32,
    ) -> anyhow::Result<String> {
        Ok(self.next_pane.lock().unwrap().clone())
    }

    fn set_pane_title(&self, pane_id: &str, title: &str) {
        self.record(&format!("set_pane_title:{pane_id}:{title}"));
    }

    fn set_pane_user_option(&self, pane_id: &str, name: &str, value: &str) {
        self.record(&format!("set_pane_user_option:{pane_id}:{name}:{value}"));
    }

    fn set_pane_style(
        &self,
        pane_id: &str,
        border_style: Option<&str>,
        active_border_style: Option<&str>,
    ) {
        self.record(&format!(
            "set_pane_style:{pane_id}:{border_style:?}:{active_border_style:?}"
        ));
    }

    fn tmux_run(&self, args: &[&str], _check: bool, _capture: bool) -> anyhow::Result<String> {
        self.record(&format!("tmux_run:{}", args.join(" ")));
        if args.len() == 5
            && args[0] == "list-panes"
            && args[1] == "-t"
            && args[3] == "-F"
            && args[4] == "#{pane_width} #{pane_height}"
        {
            let (w, h) = *self.pane_size.lock().unwrap();
            return Ok(format!("{w} {h}"));
        }
        if args.len() >= 2 && args[0] == "new-session" {
            return Ok(self.detached_pane.lock().unwrap().clone());
        }
        if args.len() == 5
            && args[0] == "list-panes"
            && args[1] == "-t"
            && args[3] == "-F"
            && args[4] == "#{pane_id}"
        {
            return Ok(self.detached_pane.lock().unwrap().clone());
        }
        Ok("".to_string())
    }
}

pub fn make_ensure_impl(backend: Arc<FakeBackend>) -> EnsureAgentRuntimeImpl {
    let launcher = ProviderLauncher::new();
    let backend_for_factory = backend.clone();
    EnsureAgentRuntimeImpl::new(launcher, move |_name, _path| {
        Box::new((*backend_for_factory).clone())
    })
}

pub fn make_ensure_impl_with_min_size(
    backend: Arc<FakeBackend>,
    width: u32,
    height: u32,
) -> EnsureAgentRuntimeImpl {
    make_ensure_impl(backend).with_min_pane_size(width, height)
}
