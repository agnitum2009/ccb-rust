//! Mirrors Python lib/message_bureau/store.py
//!
//! Storage abstractions for message bureau data.
//!
//! Re-exports from `ccbr_mailbox::stores`.

pub use ccbr_mailbox::stores::{AttemptStore, MessageStore, ReplyStore};

// TODO: translate any additional store.py functionality not in ccbr_mailbox
