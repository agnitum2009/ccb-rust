//! Mirrors Python lib/message_bureau/control_trace.py
//!
//! Control trace operations for message and job tracking.
//!
//! Re-exports functions from `ccbr_mailbox::control_trace`.

pub use ccbr_mailbox::control_trace::trace;

// TODO: translate any additional control_trace.py functionality not in ccbr_mailbox
