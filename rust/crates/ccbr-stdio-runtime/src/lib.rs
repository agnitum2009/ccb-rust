//! CCBR stdio setup and encoding helpers.
//!
//! Mirrors `lib/stdio_runtime/` from Python v7.5.2. This crate provides the
//! canonical stdin decoding implementation used by the terminal runtime.

pub mod decoding;
pub mod stdio;

/// Re-exports matching Python `stdio_runtime.__all__`.
pub use decoding::decode_stdin_bytes;
pub use stdio::{read_stdin_text, setup_windows_encoding};

/// Crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_smoke() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn public_api_exports() {
        assert_eq!(decode_stdin_bytes(b"hello"), "hello");
        // `setup_windows_encoding` and `read_stdin_text` are callable via the
        // re-exports above.
        let _ = setup_windows_encoding;
        let _ = read_stdin_text;
    }
}
