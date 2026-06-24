//! CCB stdio setup and encoding helpers.
//!
//! The canonical implementation now lives in `ccbr-stdio-runtime`. This module
//! re-exports it for backward compatibility with existing terminal callers.

pub use ccbr_stdio_runtime::*;
