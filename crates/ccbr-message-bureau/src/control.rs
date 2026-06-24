//! Mirrors Python lib/message_bureau/control.py
//!
//! Control service for queue inspection and management.
//!
//! Re-exports `MessageBureauControlService` from `ccbr_mailbox::bureau`.

// Re-export the control service from ccbr_mailbox
pub use ccbr_mailbox::bureau::MessageBureauControlService;

// TODO: translate any additional control.py functionality not in ccbr_mailbox
