use crate::types::ProjectMemorySource;
use std::path::{Path, PathBuf};

const CCB_RUNTIME_COORDINATION_RULES: &str = r#"## CCB Runtime Coordination Rules

- CCB `ask` is submit-only: submit once, then stop. Do not wait, poll, or run `pend`/`watch`/`ping` unless diagnostics were requested.
- Prefer `/ask <agent> <message>` when available. Shell fallback:

```bash
command ask "$TARGET" <<'EOF'
$MESSAGE
EOF
```

- During an active CCB ask task, use `ask --callback` when a child result is needed to finish; use `ask --silence` only for independent no-result-needed work.
"#;

/// Render a memory bundle from sources.
pub fn render_memory_bundle(
    project_root: &Path,
    agent_name: &str,
    provider: &str,
    sources: &[ProjectMemorySource],
    workspace_path: Option<&Path>,
) -> String {
    let mut lines: Vec<String> = vec![
        "# CCB Managed Agent Memory".to_string(),
        String::new(),
        "<!-- ccbr-memory-bundle schema_version=1".to_string(),
        "generated_by: ccb".to_string(),
        "do_not_edit: true".to_string(),
        format!("agent: {agent_name}"),
        format!("provider: {provider}"),
        format!("project_root: {}", resolve_path(project_root).display()),
    ];

    if let Some(ws) = workspace_path {
        lines.push(format!("workspace_path: {}", resolve_path(ws).display()));
    }

    lines.extend([
        "-->".to_string(),
        String::new(),
        CCB_RUNTIME_COORDINATION_RULES.trim_end().to_string(),
        String::new(),
    ]);

    for source in sources {
        if !source.exists && source.warning.is_empty() {
            continue;
        }
        if source.content.trim().is_empty() && source.warning.is_empty() {
            continue;
        }
        lines.extend(render_source_section(source));
    }

    format!("{}\n", lines.join("\n").trim_end())
}

fn render_source_section(source: &ProjectMemorySource) -> Vec<String> {
    let content = source.content.trim_end();
    let mut lines = vec![
        format!("## {}", source.title),
        format!("source: {}", source.path.display()),
    ];
    if !source.warning.is_empty() {
        lines.push(format!("warning: {}", source.warning));
    }
    lines.extend([String::new(), content.to_string(), String::new()]);
    lines
}

fn resolve_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
