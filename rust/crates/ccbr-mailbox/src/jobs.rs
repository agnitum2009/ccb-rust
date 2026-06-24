//! CCBR job / submission / event stores.
//!
//! The canonical implementation now lives in the `ccbr-jobs` crate. This module
//! re-exports it so that existing `ccbr-mailbox` callers keep compiling.

pub use ccbr_jobs::store::{JobEventStore, JobStore, SubmissionStore};
