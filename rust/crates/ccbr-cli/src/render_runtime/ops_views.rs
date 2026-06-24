//! Mirrors Python `lib/cli/render_runtime/ops_views.py`.

pub use crate::render_runtime::fault_views::{
    render_fault_arm, render_fault_clear, render_fault_list,
};
pub use crate::render_runtime::ops_views_basic::{
    render_cleanup, render_clear, render_config_validate, render_doctor_bundle, render_kill,
    render_logs, render_maintenance, render_ps, render_restart, render_start,
};
pub use crate::render_runtime::ops_views_doctor::{render_doctor, render_doctor_storage};
pub use crate::render_runtime::reload_view::render_reload;
