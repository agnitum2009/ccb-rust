//! Mirrors Python lib/message_bureau/models.py
//!
//! Core model definitions for message bureau.
//!
//! Re-exports from `ccbr_mailbox::models`.

pub use ccbr_mailbox::models::{
    AttemptRecord, AttemptState, MessageRecord, MessageState, ReplyRecord, ReplyTerminalStatus,
    SCHEMA_VERSION,
};

// TODO: translate any additional models.py functionality not in ccbr_mailbox
