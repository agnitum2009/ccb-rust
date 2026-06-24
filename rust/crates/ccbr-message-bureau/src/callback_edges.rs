//! Mirrors Python lib/message_bureau/callback_edges.py
//!
//! Callback edge tracking for parent-child job relationships.
//!
//! Re-exports from `ccbr_mailbox::models` and `ccbr_mailbox::stores`.

// Re-export the types and stores from ccbr_mailbox
pub use ccbr_mailbox::models::{CallbackEdgeRecord, CallbackEdgeState};
pub use ccbr_mailbox::stores::CallbackEdgeStore;

// TODO: translate any additional callback_edges.py functionality not in ccbr_mailbox
