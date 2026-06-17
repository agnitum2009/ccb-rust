//! Mirrors Python `lib/ccbd/reload_append_layout.py`.

use ccb_agents::layout::{parse_layout_spec, LayoutNode};

/// Plan entry describing how to append a new agent pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendAgentPlan {
    pub agent: String,
    pub direction: String,
}

impl AppendAgentPlan {
    pub fn new(agent: impl Into<String>, direction: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            direction: direction.into(),
        }
    }
}

/// Build an append plan for a window whose layout only grows at the rightmost leaf.
pub fn rightmost_leaf_append_plan(
    old_window: &dyn WindowLayoutAccess,
    new_window: &dyn WindowLayoutAccess,
) -> Option<Vec<AppendAgentPlan>> {
    let old_layout = parse_layout_spec(&old_window.user_layout()).ok()?;
    let new_layout = parse_layout_spec(&new_window.user_layout()).ok()?;
    if let Some(plan) = rightmost_leaf_append_plan_for_nodes(&old_layout, &new_layout) {
        return Some(plan);
    }
    trailing_sequence_append_plan(&old_layout, &new_layout)
}

fn node_kind(node: &LayoutNode) -> &'static str {
    match node {
        LayoutNode::Leaf { .. } => "leaf",
        LayoutNode::Horizontal { .. } => "horizontal",
        LayoutNode::Vertical { .. } => "vertical",
    }
}

fn node_children(node: &LayoutNode) -> Option<(&LayoutNode, &LayoutNode)> {
    match node {
        LayoutNode::Horizontal { left, right } | LayoutNode::Vertical { left, right } => {
            Some((left.as_ref(), right.as_ref()))
        }
        LayoutNode::Leaf { .. } => None,
    }
}

fn leaf_name(node: &LayoutNode) -> Option<&str> {
    match node {
        LayoutNode::Leaf { leaf } => Some(leaf.name.as_str()),
        _ => None,
    }
}

fn rightmost_leaf_append_plan_for_nodes(
    old_node: &LayoutNode,
    new_node: &LayoutNode,
) -> Option<Vec<AppendAgentPlan>> {
    match (old_node, new_node) {
        (LayoutNode::Leaf { .. }, _) => expanded_leaf_append_plan(old_node, new_node),
        _ => {
            if node_kind(old_node) != node_kind(new_node) {
                return None;
            }
            let (old_left, old_right) = node_children(old_node)?;
            let (new_left, new_right) = node_children(new_node)?;
            if old_left.render() != new_left.render() {
                return None;
            }
            rightmost_leaf_append_plan_for_nodes(old_right, new_right)
        }
    }
}

fn expanded_leaf_append_plan(
    anchor_node: &LayoutNode,
    new_node: &LayoutNode,
) -> Option<Vec<AppendAgentPlan>> {
    match new_node {
        LayoutNode::Leaf { .. } => {
            if new_node.render() == anchor_node.render() {
                Some(Vec::new())
            } else {
                None
            }
        }
        _ => {
            let (new_left, new_right) = node_children(new_node)?;
            let mut left_plan = expanded_leaf_append_plan(anchor_node, new_left)?;
            if !matches!(new_right, LayoutNode::Leaf { .. }) {
                return None;
            }
            let agent = leaf_name(new_right)?.to_string();
            let direction = if node_kind(new_node) == "horizontal" {
                "right"
            } else {
                "bottom"
            };
            left_plan.push(AppendAgentPlan::new(agent, direction));
            Some(left_plan)
        }
    }
}

fn trailing_sequence_append_plan(
    old_node: &LayoutNode,
    new_node: &LayoutNode,
) -> Option<Vec<AppendAgentPlan>> {
    let old_leaves: Vec<String> = old_node.iter_leaves().iter().map(|l| l.name.clone()).collect();
    let new_leaves: Vec<String> = new_node.iter_leaves().iter().map(|l| l.name.clone()).collect();
    if old_leaves.is_empty() || new_leaves[..old_leaves.len()] != old_leaves {
        return None;
    }
    let appended = &new_leaves[old_leaves.len()..];
    if appended.is_empty() {
        return Some(Vec::new());
    }
    let old_kind = sequence_kind(old_node)?;
    let new_kind = sequence_kind(new_node)?;
    if old_kind != new_kind {
        return None;
    }
    let direction = if old_kind == "horizontal" {
        "right"
    } else {
        "bottom"
    };
    Some(
        appended
            .iter()
            .map(|agent| AppendAgentPlan::new(agent.clone(), direction))
            .collect(),
    )
}

fn sequence_kind(node: &LayoutNode) -> Option<String> {
    if matches!(node, LayoutNode::Leaf { .. }) {
        return None;
    }
    let kind = node_kind(node).to_string();
    if all_branch_kinds(node, &kind) {
        Some(kind)
    } else {
        None
    }
}

fn all_branch_kinds(node: &LayoutNode, kind: &str) -> bool {
    match node {
        LayoutNode::Leaf { .. } => true,
        _ => {
            if node_kind(node) != kind {
                return false;
            }
            let (left, right) = match node_children(node) {
                Some((l, r)) => (l, r),
                None => return false,
            };
            all_branch_kinds(left, kind) && all_branch_kinds(right, kind)
        }
    }
}

/// Trait to access window layout fields.
pub trait WindowLayoutAccess {
    fn user_layout(&self) -> String;
    fn agent_names(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWindow {
        layout: String,
        agents: Vec<String>,
    }

    impl WindowLayoutAccess for TestWindow {
        fn user_layout(&self) -> String {
            self.layout.clone()
        }
        fn agent_names(&self) -> Vec<String> {
            self.agents.clone()
        }
    }

    fn win(layout: &str, agents: &[&str]) -> TestWindow {
        TestWindow {
            layout: layout.to_string(),
            agents: agents.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_rightmost_leaf_append_plan_no_change() {
        let old = win("claude", &["claude"]);
        let new = win("claude", &["claude"]);
        assert_eq!(rightmost_leaf_append_plan(&old, &new), Some(Vec::new()));
    }

    #[test]
    fn test_rightmost_leaf_append_plan_single_append() {
        let old = win("claude", &["claude"]);
        let new = win("claude;codex", &["claude", "codex"]);
        let plan = rightmost_leaf_append_plan(&old, &new).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].agent, "codex");
        assert_eq!(plan[0].direction, "right");
    }

    #[test]
    fn test_rightmost_leaf_append_plan_vertical() {
        let old = win("claude", &["claude"]);
        let new = win("claude,codex", &["claude", "codex"]);
        let plan = rightmost_leaf_append_plan(&old, &new).unwrap();
        assert_eq!(plan[0].agent, "codex");
        assert_eq!(plan[0].direction, "bottom");
    }

    #[test]
    fn test_rightmost_leaf_append_plan_blocked_when_old_agents_change() {
        let old = win("claude", &["claude"]);
        let new = win("codex", &["codex"]);
        assert!(rightmost_leaf_append_plan(&old, &new).is_none());
    }

    #[test]
    fn test_trailing_sequence_append_plan() {
        let old = win("claude;codex", &["claude", "codex"]);
        let new = win("claude;codex;gemini", &["claude", "codex", "gemini"]);
        let plan = rightmost_leaf_append_plan(&old, &new).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].agent, "gemini");
    }
}
