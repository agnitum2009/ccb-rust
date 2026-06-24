//! Mirrors Python lib/message_bureau/facade_state.py
//!
//! Facade state management operations.
//!
//! Re-exports from `ccbr_mailbox::facade_state`.

pub use ccbr_mailbox::facade_state::{
    pending_callback_edges, refresh_message_state, set_message_state, FacadeState,
};

// TODO: translate any additional facade_state.py functionality not in ccbr_mailbox
