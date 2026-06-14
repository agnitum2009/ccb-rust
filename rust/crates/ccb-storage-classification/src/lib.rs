//! CCB provider-home storage classification.
//!
//! Mirrors `lib/storage_classification/` from Python v7.5.2.

pub mod classification;

pub use classification::{
    classify_provider_home, summarize_storage, StorageClass, StorageEntry, SCHEMA_VERSION,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
pub mod models;
pub mod provider_home;
pub mod service;
    use super::*;

    #[test]
    fn crate_smoke() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
