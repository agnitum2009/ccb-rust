// TODO: remove these allowances once the stub modules are fully implemented.
// These suppress warnings in the current partial-Rust migration state.
#![allow(clippy::new_without_default)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]
#![allow(unused_variables)]

pub mod api;
pub mod api_selection;
pub mod backend;
pub mod backend_env;
pub mod backend_selection;
pub mod backend_types;
pub mod detect;
pub mod env;
pub mod identity;
pub mod input;
pub mod layouts;
pub mod layouts_models;
pub mod layouts_root;
pub mod layouts_split;
pub mod logs;
pub mod pane_logs;
pub mod pane_logs_runtime;
pub mod panes;
pub mod placeholders;
pub mod readiness;
pub mod registry;
pub mod respawn;
pub mod stdio;
pub mod theme;
pub mod tmux;
pub mod tmux_attach;
pub mod tmux_backend;
pub mod tmux_backend_control;
pub mod tmux_backend_logs;
pub mod tmux_backend_panes;
pub mod tmux_backend_runtime;
pub mod tmux_identity;
pub mod tmux_panes_runtime;
pub mod tmux_respawn_service;
pub mod tmux_send;
pub mod tmux_theme;

/// Re-export the most commonly used types.
pub use api::{
    create_auto_layout, detect_terminal, get_backend, get_backend_for_session,
    get_pane_id_from_session, get_shell_type,
};
pub use backend::{TerminalBackend, TerminalBackendSelection, TmuxBackend, TmuxOutput};
pub use backend_env::{apply_backend_env, get_backend_env};
pub use detect::{
    client_tty_matches, current_tty, inside_tmux, pane_id_matches, pane_tty_matches,
    tmux_env_present, tmux_value,
};
pub use env::{
    default_shell, env_float, env_int, is_windows, is_wsl, isolated_tmux_env, sanitize_filename,
    subprocess_kwargs, tmux_compatible_env,
};
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
pub use theme::{
    detect_terminal_family, pane_border_format, pane_visual as theme_pane_visual,
    render_tmux_session_theme, shell_exports, theme_profile_definition, tmux_status_interval,
    tmux_theme_profile, RenderedTmuxSessionTheme, TmuxThemeProfile,
};
pub use tmux::{
    collect_pane_title_matches, default_detached_session_name, looks_like_pane_id,
    looks_like_tmux_target, normalize_socket_name, normalize_split_direction,
    normalize_user_option, normalized_marker, pane_exists_output, pane_id_by_title_marker_output,
    pane_is_alive, pane_pipe_enabled, parse_pane_title_line, parse_session_name,
    record_pane_title_match, select_marker_match, socket_name_from_tmux_env,
    socket_ref_from_tmux_env, split_pane_title_line, tmux_base,
};
