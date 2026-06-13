pub mod backend;
pub mod env;
pub mod identity;
pub mod input;
pub mod layouts;
pub mod logs;
pub mod panes;
pub mod readiness;
pub mod registry;
pub mod respawn;
pub mod stdio;
pub mod tmux;

/// Re-export the most commonly used types.
pub use backend::{TerminalBackend, TerminalBackendSelection, TmuxBackend, TmuxOutput};
pub use identity::{apply_ccb_pane_identity, pane_visual, TmuxPaneVisual};
pub use input::{sanitize_text, should_use_inline_legacy_send, TmuxTextSender};
pub use layouts::{
    create_tmux_auto_layout, LayoutNode, LayoutResult, SplitDirection, TmuxLayoutBackend,
};
pub use logs::TmuxPaneLogManager;
pub use panes::{parse_list_panes, PaneInfo, TmuxPaneService};
pub use readiness::{
    is_tmux_absent_server_text, is_tmux_missing_session_text, is_tmux_transient_server_error,
    is_tmux_transient_server_error_text, TmuxCommandError, TmuxTransientServerUnavailable,
};
pub use registry::{PaneEntry, PaneRegistry, UserSession};
pub use respawn::TmuxRespawnService;
pub use stdio::{decode_stdin_bytes, read_stdin_text};
pub use tmux::{
    default_detached_session_name, looks_like_pane_id, looks_like_tmux_target,
    normalize_socket_name, normalize_split_direction, normalize_user_option, pane_exists_output,
    pane_id_by_title_marker_output, pane_is_alive, pane_pipe_enabled, parse_session_name,
    socket_name_from_tmux_env, socket_ref_from_tmux_env, tmux_base,
};
