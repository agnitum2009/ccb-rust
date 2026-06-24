use regex::Regex;

pub fn markdown_section(body: &str, heading: &str) -> Option<String> {
    let escaped = regex::escape(heading);
    let pattern = format!(r"(?m)^##\s+{}[^\n]*$", escaped);
    let re = Regex::new(&pattern).ok()?;
    let start_match = re.find(body)?;
    let start = start_match.end();
    // Find the next ## heading after this one.
    let next_re = Regex::new(r"(?m)^##\s+").ok()?;
    let after_start = &body[start..];
    let end = next_re
        .find(after_start)
        .map(|m| start + m.start())
        .unwrap_or(body.len());
    Some(body[start..end].trim().to_string())
}

pub fn readme_release_block(body: &str, version: &str) -> Option<String> {
    let marker = format!("<summary><b>{}</b>", version);
    let start = body.find(&marker)?;
    let after_marker = start + marker.len();
    // Find the closing </summary> after the marker.
    let summary_end = body[after_marker..].find("</summary>")?;
    let content_start = after_marker + summary_end + "</summary>".len();
    // Find the closing </details> after content start.
    let details_end = body[content_start..].find("</details>")?;
    Some(
        body[content_start..content_start + details_end]
            .trim()
            .to_string(),
    )
}

pub fn has_substantive_release_text(text: Option<&str>) -> bool {
    let text = match text {
        Some(t) => t,
        None => return false,
    };
    let cleaned: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let stripped = line.trim();
            if stripped.is_empty() {
                return None;
            }
            if stripped.starts_with("<!--")
                || stripped.starts_with("-->")
                || stripped.starts_with("<details")
                || stripped.starts_with("</details>")
                || stripped.starts_with("<summary")
                || stripped.starts_with("</summary>")
            {
                return None;
            }
            Some(stripped.to_string())
        })
        .collect();
    let re = Regex::new(r"[A-Za-z0-9\u{4e00}-\u{9fff}]").unwrap();
    cleaned.iter().any(|line| re.is_match(line))
}

pub fn semver_tuple(version: &str) -> (i32, i32, i32) {
    let re = Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)$").unwrap();
    re.captures(version.trim())
        .map(|caps| {
            (
                caps[1].parse().unwrap_or(-1),
                caps[2].parse().unwrap_or(-1),
                caps[3].parse().unwrap_or(-1),
            )
        })
        .unwrap_or((-1, -1, -1))
}

pub fn release_note_versions(body: &str) -> Vec<String> {
    let re = Regex::new(r"<summary><b>(v\d+\.\d+\.\d+)</b>").unwrap();
    re.captures_iter(body)
        .map(|caps| caps[1].to_string())
        .collect()
}

pub fn install_section(body: &str, heading: &str) -> String {
    let escaped = regex::escape(heading);
    let pattern = format!(r"(?m)^##\s+{}\s*$", escaped);
    let Some(re) = Regex::new(&pattern).ok() else {
        return body.to_string();
    };
    let Some(start_match) = re.find(body) else {
        return body.to_string();
    };
    let start = start_match.end();
    let next_re = match Regex::new(r"(?m)^##\s+") {
        Ok(r) => r,
        Err(_) => return body.to_string(),
    };
    let after_start = &body[start..];
    let end = next_re
        .find(after_start)
        .map(|m| start + m.start())
        .unwrap_or(body.len());
    body[start..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_section() {
        let body = "# Title\n## v1.0.0 (2024-01-01)\nFixed bug.\n## v0.9.0\nOld.";
        assert_eq!(
            markdown_section(body, "v1.0.0 (2024-01-01)"),
            Some("Fixed bug.".to_string())
        );
        assert_eq!(markdown_section(body, "missing"), None);
    }

    #[test]
    fn test_readme_release_block() {
        let body = "<details>\n<summary><b>v1.0.0</b> - title</summary>\nFixed bug.\n</details>";
        assert_eq!(
            readme_release_block(body, "v1.0.0"),
            Some("Fixed bug.".to_string())
        );
    }

    #[test]
    fn test_has_substantive_release_text() {
        assert!(!has_substantive_release_text(None));
        assert!(!has_substantive_release_text(Some("")));
        assert!(!has_substantive_release_text(Some("<!-- comment -->")));
        assert!(has_substantive_release_text(Some("- Fixed bug")));
        assert!(has_substantive_release_text(Some("- 修复错误")));
    }

    #[test]
    fn test_semver_tuple() {
        assert_eq!(semver_tuple("v1.2.3"), (1, 2, 3));
        assert_eq!(semver_tuple("1.2.3"), (1, 2, 3));
        assert_eq!(semver_tuple("bad"), (-1, -1, -1));
    }

    #[test]
    fn test_release_note_versions() {
        let body = "<summary><b>v1.0.0</b></summary>\n<summary><b>v0.9.0</b></summary>";
        assert_eq!(release_note_versions(body), vec!["v1.0.0", "v0.9.0"]);
    }

    #[test]
    fn test_install_section() {
        let body = "# Title\n## How to Install\nRun this.\n## Next\nOther.";
        assert_eq!(install_section(body, "How to Install"), "\nRun this.\n");
        assert_eq!(install_section(body, "Missing"), body);
    }
}
