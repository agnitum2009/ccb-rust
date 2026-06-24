//! Mirrors Python lib/message_bureau/facade.py
//!
//! Message Bureau facade: high-level message lifecycle management.
//!
//! Re-exports `MessageBureauFacade` from `ccbr_mailbox::bureau`.

pub use ccbr_mailbox::bureau::MessageBureauFacade;

// TODO: translate any additional facade.py functionality not in ccbr_mailbox
