use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutLeaf {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutNode {
    Leaf {
        leaf: LayoutLeaf,
    },
    Horizontal {
        left: Box<LayoutNode>,
        right: Box<LayoutNode>,
    },
    Vertical {
        left: Box<LayoutNode>,
        right: Box<LayoutNode>,
    },
}

impl LayoutNode {
    pub fn leaf(name: impl Into<String>) -> Self {
        LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: name.into(),
                ..LayoutLeaf::default()
            },
        }
    }

    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
                left.leaf_count() + right.leaf_count()
            }
        }
    }

    pub fn iter_leaves(&self) -> Vec<&LayoutLeaf> {
        match self {
            LayoutNode::Leaf { leaf } => vec![leaf],
            LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
                let mut leaves = left.iter_leaves();
                leaves.extend(right.iter_leaves());
                leaves
            }
        }
    }

    pub fn iter_names(&self) -> Vec<&str> {
        self.iter_leaves()
            .into_iter()
            .map(|leaf| leaf.name.as_str())
            .collect()
    }

    pub fn render(&self) -> String {
        match self {
            LayoutNode::Leaf { leaf } => render_leaf(leaf),
            LayoutNode::Horizontal { left, right } => {
                format!(
                    "{}; {}",
                    render_child(left, "horizontal"),
                    render_child(right, "horizontal")
                )
            }
            LayoutNode::Vertical { left, right } => {
                format!(
                    "{}, {}",
                    render_child(left, "vertical"),
                    render_child(right, "vertical")
                )
            }
        }
    }
}

fn render_leaf(leaf: &LayoutLeaf) -> String {
    let mut parts = Vec::new();
    if let Some(provider) = &leaf.provider {
        if leaf.workspace_mode.as_deref() == Some("worktree") {
            parts.push(format!("{}:{}(worktree)", leaf.name, provider));
        } else {
            parts.push(format!("{}:{}", leaf.name, provider));
        }
    } else {
        parts.push(leaf.name.clone());
    }
    if let Some(percent) = leaf.percent {
        parts.push(format!("@{percent}"));
    }
    parts.join("")
}

fn precedence(kind: &str) -> u8 {
    match kind {
        "horizontal" => 1,
        "vertical" => 2,
        _ => 3,
    }
}

fn render_child(node: &LayoutNode, parent_kind: &str) -> String {
    if matches!(node, LayoutNode::Leaf { .. }) {
        return node.render();
    }
    let child_kind = node_kind(node);
    let child_rank = precedence(child_kind);
    let parent_rank = precedence(parent_kind);
    let text = node.render();
    if child_rank < parent_rank {
        return format!("({text})");
    }
    if parent_kind == "vertical" && child_kind == "horizontal" {
        return format!("({text})");
    }
    text
}

fn node_kind(node: &LayoutNode) -> &'static str {
    match node {
        LayoutNode::Leaf { .. } => "leaf",
        LayoutNode::Horizontal { .. } => "horizontal",
        LayoutNode::Vertical { .. } => "vertical",
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct LayoutParseError(pub String);

fn leaf_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"^(?P<name>[A-Za-z][A-Za-z0-9_.-]{0,63})",
            r"(?:\s*:\s*(?P<provider>[A-Za-z0-9_-]+)",
            r"(?:\s*\(\s*(?P<workspace_mode>worktree)\s*\))?",
            r")?",
            r"(?:\s*@\s*(?P<percent>\d+))?$"
        ))
        .unwrap()
    })
}

struct LayoutParser {
    tokens: Vec<String>,
    index: usize,
}

impl LayoutParser {
    fn new(text: &str) -> Self {
        Self {
            tokens: tokenize(text),
            index: 0,
        }
    }

    fn parse(mut self) -> Result<LayoutNode, LayoutParseError> {
        if self.tokens.is_empty() {
            return Err(LayoutParseError("layout is empty".into()));
        }
        let node = self.parse_horizontal()?;
        if self.peek().is_some() {
            return Err(LayoutParseError(format!(
                "unexpected token {:?}",
                self.peek().unwrap()
            )));
        }
        Ok(node)
    }

    fn parse_horizontal(&mut self) -> Result<LayoutNode, LayoutParseError> {
        let mut node = self.parse_vertical()?;
        while self.peek() == Some(";") {
            self.consume(";")?;
            let rhs = self.parse_vertical()?;
            node = LayoutNode::Horizontal {
                left: Box::new(node),
                right: Box::new(rhs),
            };
        }
        Ok(node)
    }

    fn parse_vertical(&mut self) -> Result<LayoutNode, LayoutParseError> {
        let mut node = self.parse_primary()?;
        while self.peek() == Some(",") {
            self.consume(",")?;
            let rhs = self.parse_primary()?;
            node = LayoutNode::Vertical {
                left: Box::new(node),
                right: Box::new(rhs),
            };
        }
        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<LayoutNode, LayoutParseError> {
        let token = self
            .peek()
            .ok_or_else(|| LayoutParseError("unexpected end of layout".into()))?;
        if token == "(" {
            self.consume("(")?;
            let node = self.parse_horizontal()?;
            self.consume(")")?;
            return Ok(node);
        }
        if [")", ";", ","].contains(&token) {
            return Err(LayoutParseError(format!("unexpected token {token:?}")));
        }
        let token = self.consume_any()?;
        self.parse_leaf(&token)
    }

    fn parse_leaf(&self, token: &str) -> Result<LayoutNode, LayoutParseError> {
        let re = leaf_token_re();
        let captures = re.captures(token).ok_or_else(|| {
            LayoutParseError(format!(
                "invalid layout token {token:?}; expected 'cmd', 'agent', 'agent:provider', 'agent:provider(worktree)', or any of those forms with '@N'"
            ))
        })?;
        let percent = captures
            .name("percent")
            .and_then(|m| m.as_str().parse::<u32>().ok());
        Ok(LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: captures["name"].trim().into(),
                provider: captures.name("provider").map(|m| m.as_str().into()),
                workspace_mode: captures.name("workspace_mode").map(|m| m.as_str().into()),
                percent,
            },
        })
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.index).map(|s| s.as_str())
    }

    fn consume(&mut self, expected: &str) -> Result<(), LayoutParseError> {
        let token = self.peek();
        if token != Some(expected) {
            return Err(LayoutParseError(format!(
                "expected {expected:?}, found {token:?}"
            )));
        }
        self.index += 1;
        Ok(())
    }

    fn consume_any(&mut self) -> Result<String, LayoutParseError> {
        let token = self
            .tokens
            .get(self.index)
            .cloned()
            .ok_or_else(|| LayoutParseError("unexpected end of layout".into()))?;
        self.index += 1;
        Ok(token)
    }
}

pub fn parse_layout_spec(text: &str) -> Result<LayoutNode, LayoutParseError> {
    LayoutParser::new(text).parse()
}

/// Return the leaf names of a layout in left-to-right order.
///
/// Mirrors Python `iter_layout_names`.
pub fn iter_layout_names(node: &LayoutNode) -> Vec<&str> {
    node.iter_names()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut buf = String::new();
    for raw_line in text.lines() {
        let line = raw_line
            .split('#')
            .next()
            .unwrap_or("")
            .split("//")
            .next()
            .unwrap_or("");
        let mut index = 0;
        let _bytes = line.as_bytes();
        while index < line.len() {
            let char = line[index..].chars().next().unwrap();
            if char == '(' && !buf.trim().is_empty() {
                if let Some(close) = line[index + 1..].find(')') {
                    buf.push_str(&line[index..index + 1 + close + 1]);
                    index += 1 + close + 1;
                    continue;
                }
            }
            if ['(', ')', ';', ','].contains(&char) {
                append_leaf_token(&mut tokens, &mut buf);
                tokens.push(char.to_string());
                index += char.len_utf8();
                continue;
            }
            buf.push(char);
            index += char.len_utf8();
        }
        append_leaf_token(&mut tokens, &mut buf);
    }
    tokens.into_iter().filter(|s| !s.is_empty()).collect()
}

fn append_leaf_token(tokens: &mut Vec<String>, buf: &mut String) {
    let leaf = buf.trim();
    if !leaf.is_empty() {
        tokens.push(leaf.into());
    }
    buf.clear();
}

pub fn build_balanced_layout(
    agent_names: impl IntoIterator<Item = impl AsRef<str>>,
    providers_by_agent: Option<&std::collections::HashMap<String, String>>,
    workspace_modes_by_agent: Option<&std::collections::HashMap<String, String>>,
    cmd_enabled: bool,
) -> LayoutNode {
    let ordered_agents: Vec<String> = agent_names
        .into_iter()
        .map(|s| s.as_ref().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ordered_agents.is_empty() {
        panic!("at least one agent is required for layout");
    }
    let empty_providers = std::collections::HashMap::new();
    let empty_modes = std::collections::HashMap::new();
    let providers = providers_by_agent.unwrap_or(&empty_providers);
    let modes = workspace_modes_by_agent.unwrap_or(&empty_modes);
    let mut leaves = Vec::new();
    if cmd_enabled {
        leaves.push(LayoutNode::leaf("cmd"));
    }
    for name in ordered_agents {
        leaves.push(LayoutNode::Leaf {
            leaf: LayoutLeaf {
                name: name.clone(),
                provider: providers.get(&name).cloned(),
                workspace_mode: modes.get(&name).cloned(),
                percent: None,
            },
        });
    }
    if leaves.len() == 1 {
        return leaves.into_iter().next().unwrap();
    }
    let mid = leaves.len().div_ceil(2);
    let left = stack_vertical(&leaves[..mid]);
    let right = stack_vertical(&leaves[mid..]);
    match (left, right) {
        (Some(left), Some(right)) => LayoutNode::Horizontal {
            left: Box::new(left),
            right: Box::new(right),
        },
        (Some(left), None) => left,
        (None, Some(right)) => right,
        (None, None) => unreachable!(),
    }
}

fn stack_vertical(leaves: &[LayoutNode]) -> Option<LayoutNode> {
    if leaves.is_empty() {
        return None;
    }
    let mut node = leaves[0].clone();
    for leaf in &leaves[1..] {
        node = LayoutNode::Vertical {
            left: Box::new(node),
            right: Box::new(leaf.clone()),
        };
    }
    Some(node)
}

pub fn prune_layout(node: &LayoutNode, include_names: &[String]) -> Option<LayoutNode> {
    let include: std::collections::HashSet<String> = include_names
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    match node {
        LayoutNode::Leaf { leaf } => {
            if include.contains(&leaf.name) {
                Some(node.clone())
            } else {
                None
            }
        }
        LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
            let left = prune_layout(left, include_names);
            let right = prune_layout(right, include_names);
            match (left, right) {
                (Some(left), Some(right)) => Some(match node {
                    LayoutNode::Horizontal { .. } => LayoutNode::Horizontal {
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    _ => LayoutNode::Vertical {
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                }),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_leaf() {
        let node = parse_layout_spec("agent1:codex(worktree)@50").unwrap();
        assert_eq!(node.leaf_count(), 1);
        let leaf = node.iter_leaves().pop().unwrap();
        assert_eq!(leaf.name, "agent1");
        assert_eq!(leaf.provider.as_deref(), Some("codex"));
        assert_eq!(leaf.workspace_mode.as_deref(), Some("worktree"));
        assert_eq!(leaf.percent, Some(50));
    }

    #[test]
    fn test_parse_horizontal_and_vertical() {
        let node = parse_layout_spec("a; b, c").unwrap();
        assert_eq!(node.leaf_count(), 3);
        assert!(matches!(node, LayoutNode::Horizontal { .. }));
    }

    #[test]
    fn test_build_balanced_layout() {
        let node =
            build_balanced_layout(["agent1", "agent2", "agent3", "agent4"], None, None, false);
        assert_eq!(node.leaf_count(), 4);
    }

    #[test]
    fn test_prune_layout() {
        let node = build_balanced_layout(["a", "b", "c"], None, None, false);
        let pruned = prune_layout(&node, &["a".into(), "c".into()]).unwrap();
        assert_eq!(pruned.leaf_count(), 2);
    }
}
