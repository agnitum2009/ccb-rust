//! Codex provider launcher and runtime helpers.
//!
//! Mirrors Python `lib/provider_backends/codex/launcher.py` and its
//! `launcher_runtime` submodules. The execution adapter remains in
//! `crate::providers::codex`.

pub mod launcher;
pub mod launcher_runtime;

pub use launcher::{
    build_runtime_launcher, build_session_payload, build_start_cmd, post_launch,
    prepare_launch_context,
};
pub use launcher_runtime::command::{
    build_codex_shell_prefix, CodexLaunchContext, CodexStartCommand,
};
pub use launcher_runtime::home::CodexHomeLayout;
pub use launcher_runtime::home::{
    prepare_codex_home_overrides as prepare_codex_home_overrides_for_test,
    resolve_codex_home_layout, state_dir_for_runtime_dir,
};
pub use launcher_runtime::session_paths::{
    load_resume_session_id as load_resume_session_id_for_test, session_file_for_runtime_dir,
};
