use chrono::{DateTime, FixedOffset};

pub fn parse_timestamp(text: &str) -> Option<DateTime<FixedOffset>> {
    let text = text.trim();
    let text = if let Some(rest) = text.strip_suffix('Z') {
        format!("{}+00:00", rest)
    } else {
        text.to_string()
    };
    DateTime::parse_from_rfc3339(&text).ok()
}

pub fn seconds_between(start: &str, end: &str) -> Option<f64> {
    let start_dt = parse_timestamp(start)?;
    let end_dt = parse_timestamp(end)?;
    Some((end_dt - start_dt).num_milliseconds() as f64 / 1000.0)
}
