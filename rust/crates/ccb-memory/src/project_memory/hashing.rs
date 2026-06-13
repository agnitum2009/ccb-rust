use sha2::{Digest, Sha256};

/// SHA-256 hex digest of a UTF-8 string.
pub fn sha256_text(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}
