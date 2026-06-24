use crate::project_memory::policy::{FILTER_CCBR_INSTALL_BLOCKS, SOURCE_PROVIDER_USER_MEMORY};
use crate::types::ProjectMemorySource;
use regex::Regex;

const MARKER_PAIRS: &[(&str, &str)] = &[
    ("<!-- CCBR_CONFIG_START -->", "<!-- CCBR_CONFIG_END -->"),
    ("<!-- CCBR_ROLES_START -->", "<!-- CCBR_ROLES_END -->"),
    (
        "<!-- REVIEW_RUBRICS_START -->",
        "<!-- REVIEW_RUBRICS_END -->",
    ),
    ("<!-- CODEX_REVIEW_START -->", "<!-- CODEX_REVIEW_END -->"),
    (
        "<!-- GEMINI_INSPIRATION_START -->",
        "<!-- GEMINI_INSPIRATION_END -->",
    ),
];

const LEGACY_SECTION_STARTS: &[&str] = &[
    "## Codex Collaboration Rules",
    "## Codex 协作规则",
    "## Gemini Collaboration Rules",
    "## Gemini 协作规则",
    "## OpenCode Collaboration Rules",
    "## OpenCode 协作规则",
];

/// Apply configured filters to a memory source.
pub fn filter_memory_source(
    source: &ProjectMemorySource,
    filter_names: &[String],
) -> ProjectMemorySource {
    if source.kind != SOURCE_PROVIDER_USER_MEMORY
        || filter_names.is_empty()
        || source.content.is_empty()
    {
        return source.clone();
    }

    let mut content = source.content.clone();
    let mut applied: Vec<String> = Vec::new();

    if filter_names.contains(&FILTER_CCBR_INSTALL_BLOCKS.to_string()) {
        let (new_content, changed) = strip_ccbr_install_blocks(&content);
        if changed {
            content = new_content;
            applied.push(FILTER_CCBR_INSTALL_BLOCKS.to_string());
        }
    }

    if applied.is_empty() {
        return source.clone();
    }

    ProjectMemorySource {
        content: tidy_filtered_content(&content),
        filtered: true,
        filter_names: applied,
        ..source.clone()
    }
}

fn strip_ccbr_install_blocks(content: &str) -> (String, bool) {
    let mut result = content.to_string();
    let mut total = 0usize;

    for (start, end) in MARKER_PAIRS {
        let escaped_start = regex::escape(start);
        let escaped_end = regex::escape(end);

        let line_block_pattern = format!(
            r"^[^\S\n]*{}[^\S\n]*(?:\r?\n).*?^[^\S\n]*{}[^\S\n]*(?:\r?\n)?",
            escaped_start, escaped_end
        );
        let re = Regex::new(&format!("(?sm){}", line_block_pattern)).expect("marker pattern");
        let count = re.find_iter(&result).count();
        result = re.replace_all(&result, "").to_string();
        total += count;

        let inline_pattern = format!("{}.*?{}(?:\r?\n)?", escaped_start, escaped_end);
        let re = Regex::new(&format!("(?s){}", inline_pattern)).expect("inline marker pattern");
        let count = re.find_iter(&result).count();
        result = re.replace_all(&result, "").to_string();
        total += count;
    }

    let (result, legacy_count) = strip_legacy_sections(&result);
    total += legacy_count;

    (result, total > 0)
}

fn strip_legacy_sections(content: &str) -> (String, usize) {
    let mut result_lines: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    let mut in_legacy_section: Option<&str> = None;

    for line in content.lines() {
        if in_legacy_section.is_some() {
            if line.trim_start().starts_with("## ") {
                // End of legacy section; process this boundary header normally.
                in_legacy_section = None;
            } else {
                // Still inside legacy section; drop the line.
                continue;
            }
        }

        if let Some(start) = LEGACY_SECTION_STARTS
            .iter()
            .find(|s| line.trim_start().starts_with(**s))
        {
            in_legacy_section = Some(*start);
            skipped += 1;
            continue;
        }

        result_lines.push(line.to_string());
    }

    let result = if content.ends_with('\n') {
        result_lines.join("\n") + "\n"
    } else {
        result_lines.join("\n")
    };

    (result, skipped)
}

fn tidy_filtered_content(content: &str) -> String {
    let stripped = content.trim();
    if stripped.is_empty() {
        String::new()
    } else {
        format!("{stripped}\n")
    }
}
