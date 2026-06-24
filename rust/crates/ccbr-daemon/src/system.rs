//! Mirrors Python `lib/ccbd/system.py`.

/// Return the current UTC timestamp as an ISO-8601 string.
pub fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
