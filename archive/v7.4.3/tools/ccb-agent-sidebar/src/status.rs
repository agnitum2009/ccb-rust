use ratatui::style::Color;

pub fn activity_symbol(state: &str) -> &'static str {
    match state {
        "active" => "●",
        "pending" => "◐",
        "idle" => "○",
        "failed" => "✕",
        "offline" => "·",
        _ => "·",
    }
}

pub fn activity_color(state: &str, explicit: Option<&str>) -> Color {
    if let Some(color) = explicit.and_then(parse_activity_color) {
        return color;
    }
    fallback_activity_color(state)
}

fn parse_activity_color(color: &str) -> Option<Color> {
    match color.trim().to_ascii_lowercase().as_str() {
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "red" => Some(Color::Red),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "dark_gray" | "dark-grey" | "darkgrey" => Some(Color::DarkGray),
        _ => None,
    }
}

fn fallback_activity_color(state: &str) -> Color {
    match state {
        "active" => Color::Green,
        "pending" => Color::Yellow,
        "idle" => Color::Blue,
        "failed" => Color::Red,
        "offline" => Color::DarkGray,
        _ => Color::Gray,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_phase1_states_to_fixed_symbols() {
        assert_eq!(activity_symbol("active"), "●");
        assert_eq!(activity_symbol("pending"), "◐");
        assert_eq!(activity_symbol("idle"), "○");
        assert_eq!(activity_symbol("failed"), "✕");
        assert_eq!(activity_symbol("offline"), "·");
    }

    #[test]
    fn uses_project_view_color_when_present() {
        assert_eq!(activity_color("idle", Some("green")), Color::Green);
        assert_eq!(activity_color("idle", Some(" DARK_GRAY ")), Color::DarkGray);
    }

    #[test]
    fn falls_back_to_state_color_for_unknown_or_missing_project_view_color() {
        assert_eq!(activity_color("active", None), Color::Green);
        assert_eq!(activity_color("failed", Some("unknown")), Color::Red);
    }
}
