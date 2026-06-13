use std::io::{self, Read};

/// Decode raw stdin bytes robustly without emitting surrogates.
pub fn decode_stdin_bytes(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    if let Some(decoded) = decode_with_bom(data) {
        return decoded;
    }
    if let Some(encoding) = forced_stdin_encoding() {
        return decode_forced(data, &encoding);
    }
    if let Some(decoded) = decode_utf8_strict(data) {
        return decoded;
    }
    if let Some(decoded) = decode_preferred_locale(data) {
        return decoded;
    }
    #[cfg(target_os = "windows")]
    if let Some(decoded) = decode_windows_mbcs(data) {
        return decoded;
    }
    String::from_utf8_lossy(data).to_string()
}

fn decode_with_bom(data: &[u8]) -> Option<String> {
    if data.starts_with(b"\xef\xbb\xbf") {
        return String::from_utf8(data[3..].to_vec()).ok();
    }
    if data.starts_with(b"\xff\xfe") {
        return decode_utf16_endian(&data[2..], false);
    }
    if data.starts_with(b"\xfe\xff") {
        return decode_utf16_endian(&data[2..], true);
    }
    None
}

fn decode_utf16_endian(data: &[u8], big_endian: bool) -> Option<String> {
    let u16s: Vec<u16> = data
        .chunks_exact(2)
        .map(|chunk| {
            if big_endian {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_le_bytes([chunk[0], chunk[1]])
            }
        })
        .collect();
    String::from_utf16(&u16s).ok()
}

fn forced_stdin_encoding() -> Option<String> {
    let value = std::env::var("CCB_STDIN_ENCODING").unwrap_or_default();
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn decode_forced(data: &[u8], encoding: &str) -> String {
    decode_with_encoding(data, encoding)
        .unwrap_or_else(|| String::from_utf8_lossy(data).to_string())
}

fn decode_utf8_strict(data: &[u8]) -> Option<String> {
    String::from_utf8(data.to_vec()).ok()
}

fn decode_preferred_locale(data: &[u8]) -> Option<String> {
    let preferred = std::env::var("LC_CTYPE")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let encoding = preferred.split('.').nth(1).unwrap_or(&preferred);
    if encoding.is_empty() {
        None
    } else {
        let decoded = decode_with_encoding(data, encoding);
        decoded.filter(|s| !s.contains(std::char::REPLACEMENT_CHARACTER))
    }
}

#[cfg(target_os = "windows")]
fn decode_windows_mbcs(data: &[u8]) -> Option<String> {
    let decoded = decode_with_encoding(data, "latin1");
    decoded.filter(|s| !s.contains(std::char::REPLACEMENT_CHARACTER))
}

fn decode_with_encoding(data: &[u8], encoding: &str) -> Option<String> {
    let encoding = encoding.to_lowercase();
    match encoding.as_str() {
        "utf-8" | "utf8" => String::from_utf8(data.to_vec()).ok(),
        "utf-16" | "utf16" => {
            let u16s: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            String::from_utf16(&u16s).ok()
        }
        "utf-16le" | "utf16le" => {
            let u16s: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            String::from_utf16(&u16s).ok()
        }
        "utf-16be" | "utf16be" => {
            let u16s: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect();
            String::from_utf16(&u16s).ok()
        }
        "latin1" | "iso-8859-1" => Some(data.iter().map(|&b| b as char).collect()),
        _ => String::from_utf8(data.to_vec()).ok(),
    }
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
    fn test_decode_utf8() {
        assert_eq!(decode_stdin_bytes(b"hello"), "hello");
    }

    #[test]
    fn test_decode_with_bom() {
        let data = b"\xef\xbb\xbfhello";
        assert_eq!(decode_stdin_bytes(data), "hello");
    }

    #[test]
    fn test_decode_forced_encoding() {
        std::env::set_var("CCB_STDIN_ENCODING", "latin1");
        assert_eq!(decode_stdin_bytes(&[0xe9]), "é");
        std::env::remove_var("CCB_STDIN_ENCODING");
    }
}
