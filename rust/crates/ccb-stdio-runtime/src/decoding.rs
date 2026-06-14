//! Robust stdin byte decoding.
//!
//! Mirrors `stdio_runtime/decoding.py` from Python v7.5.2.

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const UTF16_LE_BOM: &[u8] = b"\xff\xfe";
const UTF16_BE_BOM: &[u8] = b"\xfe\xff";

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
    if data.starts_with(UTF8_BOM) {
        return String::from_utf8(data[UTF8_BOM.len()..].to_vec()).ok();
    }
    if data.starts_with(UTF16_LE_BOM) {
        return decode_utf16(&data[UTF16_LE_BOM.len()..], false);
    }
    if data.starts_with(UTF16_BE_BOM) {
        return decode_utf16(&data[UTF16_BE_BOM.len()..], true);
    }
    None
}

fn decode_utf16(data: &[u8], big_endian: bool) -> Option<String> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
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
    decode_with_encoding(data, encoding, false)
        .unwrap_or_else(|| String::from_utf8_lossy(data).to_string())
}

fn decode_utf8_strict(data: &[u8]) -> Option<String> {
    String::from_utf8(data.to_vec()).ok()
}

fn decode_preferred_locale(data: &[u8]) -> Option<String> {
    let encoding = preferred_locale_encoding();
    if encoding.is_empty() {
        return None;
    }
    decode_with_encoding(data, &encoding, true)
}

fn preferred_locale_encoding() -> String {
    std::env::var("LC_CTYPE")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .split('.')
        .nth(1)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

#[cfg(target_os = "windows")]
fn decode_windows_mbcs(data: &[u8]) -> Option<String> {
    // Python's "mbcs" codec uses the current Windows ANSI code page. We
    // approximate it with windows-1252, which is correct for many Western
    // locales.
    decode_with_encoding(data, "windows-1252", true)
}

fn decode_with_encoding(data: &[u8], encoding: &str, strict: bool) -> Option<String> {
    let label = encoding.trim().to_lowercase();
    match label.as_str() {
        "utf-8" | "utf8" => String::from_utf8(data.to_vec()).ok(),
        "utf-16" | "utf16" => decode_utf16(data, false),
        "utf-16le" | "utf16le" => decode_utf16(data, false),
        "utf-16be" | "utf16be" => decode_utf16(data, true),
        "latin1" | "iso-8859-1" | "iso8859-1" | "latin-1" => {
            Some(data.iter().map(|&b| b as char).collect())
        }
        _ => {
            let enc = encoding_rs::Encoding::for_label(label.as_bytes())?;
            let (cow, _, had_errors) = enc.decode(data);
            if strict && had_errors {
                None
            } else {
                Some(cow.into_owned())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_input() {
        assert_eq!(decode_stdin_bytes(b""), "");
    }

    #[test]
    fn test_decode_utf8() {
        assert_eq!(decode_stdin_bytes(b"hello"), "hello");
    }

    #[test]
    fn test_decode_utf8_bom() {
        let data = b"\xef\xbb\xbfhello";
        assert_eq!(decode_stdin_bytes(data), "hello");
    }

    #[test]
    fn test_decode_utf16le_bom() {
        // "ab" in UTF-16LE with BOM.
        let data = b"\xff\xfe\x61\x00\x62\x00";
        assert_eq!(decode_stdin_bytes(data), "ab");
    }

    #[test]
    fn test_decode_utf16be_bom() {
        // "ab" in UTF-16BE with BOM.
        let data = b"\xfe\xff\x00\x61\x00\x62";
        assert_eq!(decode_stdin_bytes(data), "ab");
    }

    #[test]
    fn test_decode_forced_encoding_latin1() {
        std::env::set_var("CCB_STDIN_ENCODING", "latin1");
        assert_eq!(decode_stdin_bytes(&[0xe9]), "é");
        std::env::remove_var("CCB_STDIN_ENCODING");
    }

    #[test]
    fn test_decode_forced_encoding_unknown_falls_back_to_utf8_lossy() {
        std::env::set_var("CCB_STDIN_ENCODING", "not-a-real-encoding");
        // Invalid UTF-8 bytes should be replaced rather than returning an
        // empty string.
        let result = decode_stdin_bytes(&[0x80]);
        assert!(!result.is_empty());
        std::env::remove_var("CCB_STDIN_ENCODING");
    }

    #[test]
    fn test_decode_invalid_utf8_falls_back() {
        // 0x80 alone is invalid UTF-8. Without a forced encoding or locale
        // hint it will ultimately be replaced by the UTF-8 lossy fallback.
        let result = decode_stdin_bytes(&[0x80]);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_decode_preferred_locale_latin1() {
        std::env::set_var("LANG", "en_US.ISO-8859-1");
        std::env::remove_var("CCB_STDIN_ENCODING");
        assert_eq!(decode_stdin_bytes(&[0xe9]), "é");
        std::env::remove_var("LANG");
    }

    #[test]
    fn test_decode_prefers_utf8_when_valid_over_locale() {
        std::env::set_var("LANG", "zh_CN.gbk");
        std::env::remove_var("CCB_STDIN_ENCODING");
        let raw = "你好".as_bytes();
        assert_eq!(decode_stdin_bytes(raw), "你好");
        std::env::remove_var("LANG");
    }

    #[test]
    fn test_decode_falls_back_to_preferred_gbk() {
        std::env::set_var("LANG", "zh_CN.gbk");
        std::env::remove_var("CCB_STDIN_ENCODING");
        let text = "你好Codex！这是一条中文消息";
        let raw = encoding_rs::GBK.encode(text).0;
        assert_eq!(decode_stdin_bytes(&raw), text);
        std::env::remove_var("LANG");
    }
}
