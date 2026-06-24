pub const COMPLETION_STATUS_COMPLETED: &str = "completed";
pub const COMPLETION_STATUS_CANCELLED: &str = "cancelled";
pub const COMPLETION_STATUS_FAILED: &str = "failed";
pub const COMPLETION_STATUS_INCOMPLETE: &str = "incomplete";

pub const VALID_COMPLETION_STATUSES: &[&str] = &[
    COMPLETION_STATUS_COMPLETED,
    COMPLETION_STATUS_CANCELLED,
    COMPLETION_STATUS_FAILED,
    COMPLETION_STATUS_INCOMPLETE,
];

const COMPLETION_STATUS_LABELS: &[(&str, &str)] = &[
    (COMPLETION_STATUS_COMPLETED, "Completed"),
    (COMPLETION_STATUS_CANCELLED, "Cancelled"),
    (COMPLETION_STATUS_FAILED, "Failed"),
    (COMPLETION_STATUS_INCOMPLETE, "Incomplete"),
];

const COMPLETION_STATUS_MARKERS: &[(&str, &str)] = &[
    (COMPLETION_STATUS_COMPLETED, "[CCB_TASK_COMPLETED]"),
    (COMPLETION_STATUS_CANCELLED, "[CCB_TASK_CANCELLED]"),
    (COMPLETION_STATUS_FAILED, "[CCB_TASK_FAILED]"),
    (COMPLETION_STATUS_INCOMPLETE, "[CCB_TASK_INCOMPLETE]"),
];

pub fn normalize_completion_status(status: Option<&str>, done_seen: bool) -> &'static str {
    let raw = status.unwrap_or("").trim().to_lowercase();
    if VALID_COMPLETION_STATUSES.contains(&raw.as_str()) {
        return match raw.as_str() {
            COMPLETION_STATUS_COMPLETED => COMPLETION_STATUS_COMPLETED,
            COMPLETION_STATUS_CANCELLED => COMPLETION_STATUS_CANCELLED,
            COMPLETION_STATUS_FAILED => COMPLETION_STATUS_FAILED,
            _ => COMPLETION_STATUS_INCOMPLETE,
        };
    }
    if done_seen {
        COMPLETION_STATUS_COMPLETED
    } else {
        COMPLETION_STATUS_INCOMPLETE
    }
}

pub fn completion_status_label(status: Option<&str>, done_seen: bool) -> &'static str {
    let normalized = normalize_completion_status(status, done_seen);
    COMPLETION_STATUS_LABELS
        .iter()
        .find(|(s, _)| *s == normalized)
        .map(|(_, label)| *label)
        .unwrap_or("Completed")
}

pub fn completion_status_marker(status: Option<&str>, done_seen: bool) -> &'static str {
    let normalized = normalize_completion_status(status, done_seen);
    COMPLETION_STATUS_MARKERS
        .iter()
        .find(|(s, _)| *s == normalized)
        .map(|(_, marker)| *marker)
        .unwrap_or("[CCB_TASK_COMPLETED]")
}

pub fn default_reply_for_status(status: Option<&str>, done_seen: bool) -> &'static str {
    let normalized = normalize_completion_status(status, done_seen);
    match normalized {
        COMPLETION_STATUS_CANCELLED => "Task cancelled or timed out before completion.",
        COMPLETION_STATUS_FAILED => "Task failed before producing a complete reply.",
        COMPLETION_STATUS_INCOMPLETE => "Task ended without a confirmed completion marker.",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_completion_status() {
        assert_eq!(
            normalize_completion_status(Some("completed"), true),
            COMPLETION_STATUS_COMPLETED
        );
        assert_eq!(
            normalize_completion_status(Some("failed"), true),
            COMPLETION_STATUS_FAILED
        );
        assert_eq!(
            normalize_completion_status(Some("cancelled"), true),
            COMPLETION_STATUS_CANCELLED
        );
        assert_eq!(
            normalize_completion_status(Some("incomplete"), true),
            COMPLETION_STATUS_INCOMPLETE
        );
        assert_eq!(
            normalize_completion_status(Some("unknown"), true),
            COMPLETION_STATUS_COMPLETED
        );
        assert_eq!(
            normalize_completion_status(Some("unknown"), false),
            COMPLETION_STATUS_INCOMPLETE
        );
        assert_eq!(
            normalize_completion_status(None, true),
            COMPLETION_STATUS_COMPLETED
        );
    }

    #[test]
    fn test_completion_status_label_and_marker() {
        assert_eq!(completion_status_label(Some("failed"), true), "Failed");
        assert_eq!(
            completion_status_marker(Some("failed"), true),
            "[CCB_TASK_FAILED]"
        );
    }

    #[test]
    fn test_default_reply_for_status() {
        assert_eq!(
            default_reply_for_status(Some("cancelled"), true),
            "Task cancelled or timed out before completion."
        );
        assert_eq!(default_reply_for_status(Some("completed"), true), "");
    }
}
