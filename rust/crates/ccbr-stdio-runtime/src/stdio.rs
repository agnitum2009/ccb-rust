//! Stdin/stdout helpers.
//!
//! Mirrors `stdio_runtime/stdio.py` from Python v7.5.2.

use std::io::{self, Read};

use crate::decoding::decode_stdin_bytes;

/// Configure UTF-8 encoding for the Windows console.
///
/// On Windows this sets the console input and output code pages to UTF-8.
/// On other platforms this is a no-op.
pub fn setup_windows_encoding() -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::raw::c_uint;

        const CP_UTF8: c_uint = 65001;

        extern "system" {
            fn SetConsoleCP(cp: c_uint) -> i32;
            fn SetConsoleOutputCP(cp: c_uint) -> i32;
        }

        unsafe {
            if SetConsoleCP(CP_UTF8) == 0 || SetConsoleOutputCP(CP_UTF8) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }

    Ok(())
}

/// Read all text from stdin using the shared decoding policy.
pub fn read_stdin_text() -> io::Result<String> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(decode_stdin_bytes(&buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_windows_encoding_does_not_panic() {
        // On non-Windows this is a no-op; on Windows it should succeed if
        // console APIs are available.
        setup_windows_encoding().unwrap();
    }
}
