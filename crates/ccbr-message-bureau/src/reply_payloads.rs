//! Mirrors Python lib/message_bureau/reply_payloads.py
//!
//! Reply payload structures and operations.
//!
//! Re-exports functions from `ccbr_mailbox::reply_payloads`.

pub use ccbr_mailbox::reply_payloads::{
    compose_reply_payload, delivery_job_id_from_payload, reply_id_from_payload,
};

// TODO: translate any additional reply_payloads.py functionality not in ccbr_mailbox
