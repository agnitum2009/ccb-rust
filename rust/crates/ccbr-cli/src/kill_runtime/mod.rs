//! Mirrors Python `lib/cli/kill_runtime/`.

pub mod daemons;
pub mod processes;
pub mod sessions;
pub mod shutdown;
pub mod zombies;

pub use daemons::terminate_provider_daemon;
pub use processes::{
    is_pid_alive, is_pid_alive_at, kill_pid, kill_pid_tree_once, proc_pid_state, terminate_pid_tree,
};
